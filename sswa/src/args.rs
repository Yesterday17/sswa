use std::path::PathBuf;
use clap::Parser;
use anni_clap_handler::{Context, Handler, handler};
use anyhow::bail;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use ssup::{Client, Credential, UploadLine};
use crate::config::Config;
use crate::template::VideoTemplate;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// 配置文件所在的目录，留空时默认通过 directories-next 获取
    #[clap(short, long)]
    config_root: Option<PathBuf>,

    /// 执行的子命令
    #[clap(subcommand)]
    command: SsCommand,
}

#[anni_clap_handler::async_trait]
impl Handler for Args {
    async fn handle_command(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        // 初始化配置文件目录
        let config_root = self.config_root.as_deref().and_then(|path| {
            if path.is_absolute() {
                Some(path.to_path_buf())
            } else {
                path.canonicalize().ok()
            }
        }).unwrap_or_else(|| directories_next::ProjectDirs::from("moe.mmf", "Yesterday17", "sswa")
            .unwrap()
            .config_dir()
            .to_path_buf()
        );

        // 初始化读取配置文件
        let config: Config = match File::open(config_root.join("config.toml")).await {
            Ok(mut config) => {
                let mut config_str = String::new();
                config.read_to_string(&mut config_str).await?;
                toml::from_str(&config_str)?
            }
            Err(_) => Config::new(),
        };

        ctx.insert(config_root);
        ctx.insert(config);
        Ok(())
    }

    async fn handle_subcommand(&mut self, ctx: Context) -> anyhow::Result<()> {
        self.command.execute(ctx).await
    }
}

#[derive(Parser, Handler, Debug, Clone)]
pub enum SsCommand {
    /// 输出配置文件所在路径
    Config(SsConfigCommand),
    /// 用户帐号相关功能
    Account(SsAccountCommand),
    /// 上传视频相关功能
    Upload(SsUploadCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct SsConfigCommand;

#[handler(SsConfigCommand)]
async fn handle_config(config_root: &PathBuf) -> anyhow::Result<()> {
    print!("{}", config_root.display());
    Ok(())
}

#[derive(Parser, Debug, Clone)]
pub struct SsAccountCommand;

#[handler(SsAccountCommand)]
async fn handle_account() -> anyhow::Result<()> {
    Ok(())
}

#[derive(Parser, Debug, Clone)]
pub struct SsUploadCommand {
    /// 投稿使用的模板
    #[clap(short, long)]
    template: String,

    /// 投稿模板对应的变量文件
    /// 未指定该字段时会要求用户输入对应的变量
    #[clap(short, long = "var")]
    variables: Option<PathBuf>,

    /// 投稿帐号
    #[clap(long)]
    account: String,

    /// 待投稿的视频
    videos: Vec<PathBuf>,
}

impl SsUploadCommand {
    /// 尝试导入用户凭据，失败时则以该名称创建新的凭据
    async fn credential(&self, root: &PathBuf) -> anyhow::Result<Credential> {
        let account = root.join("accounts").join(format!("{}.bin", self.account));
        if account.exists() {
            // 凭据存在，读取并返回
            let mut file = File::open(account).await?;
            let mut account_str = String::new();
            file.read_to_string(&mut account_str).await?;
            let account = toml::from_str(&account_str)?;

            // TODO: 验证登录是否有效

            Ok(account)
        } else {
            // 凭据不存在，新登录
            todo!()
        }
    }

    /// 尝试导入视频模板
    async fn template(&self, root: &PathBuf) -> anyhow::Result<VideoTemplate> {
        let template = root.join("templates").join(format!("{}.toml", self.account));
        if !template.exists() {
            bail!("Template not found!");
        }

        let mut file = File::open(template).await?;
        let mut template_str = String::new();
        file.read_to_string(&mut template_str).await?;
        Ok(toml::from_str(&template_str)?)
    }
}

#[handler(SsUploadCommand)]
async fn handle_upload(this: &SsUploadCommand, config_root: &PathBuf) -> anyhow::Result<()> {
    let client = Client::new(UploadLine::auto().await?, this.credential(config_root).await?);
    let parts = client.upload(&this.videos).await?;
    client.submit(this.template(&config_root).await?
        .into_submit_form(parts).await?).await?;
    Ok(())
}
