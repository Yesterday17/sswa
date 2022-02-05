use std::path::PathBuf;
use clap::Parser;
use anni_clap_handler::{Context, Handler, handler};
use anyhow::bail;
use tokio::fs;
use ssup::{Client, Credential};
use ssup::constants::set_useragent;
use crate::config::Config;
use crate::template::VideoTemplate;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// 配置文件所在的目录，留空时默认通过 directories-next 获取
    #[clap(short, long)]
    config_root: Option<PathBuf>,

    /// 手动指定投稿时的 User-Agent
    #[clap(long = "ua")]
    user_agent: Option<String>,

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
        let config: Config = match fs::read_to_string(config_root.join("config.toml")).await {
            Ok(config) => toml::from_str(&config)?,
            Err(_) => Config::new(),
        };

        // 设置 User-Agent
        if let Some(ref user_agent) = self.user_agent {
            set_useragent(user_agent.to_string());
        }

        // 创建目录
        let _ = fs::create_dir(config_root.join("templates")).await;
        let _ = fs::create_dir(config_root.join("accounts")).await;

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
    #[clap(short = 'u', long = "user")]
    account: String,

    /// 待投稿的视频
    videos: Vec<PathBuf>,
}

impl SsUploadCommand {
    /// 尝试导入用户凭据，失败时则以该名称创建新的凭据
    async fn credential(&self, root: &PathBuf) -> anyhow::Result<Credential> {
        let account = root.join("accounts").join(format!("{}.json", self.account));
        if account.exists() {
            // 凭据存在，读取并返回
            let account = fs::read_to_string(account).await?;
            let account = serde_json::from_str(&account)?;

            // TODO: 验证登录是否有效

            Ok(account)
        } else {
            // 凭据不存在，新登录
            let qrcode = Credential::get_qrcode().await?;
            eprintln!("qrcode = {}", qrcode);
            let credential = Credential::from_qrcode(qrcode).await?;
            fs::write(account, serde_json::to_string(&credential)?).await?;
            Ok(credential)
        }
    }

    /// 尝试导入视频模板
    async fn template(&self, root: &PathBuf) -> anyhow::Result<VideoTemplate> {
        let template = root.join("templates").join(format!("{}.toml", self.template));
        if !template.exists() {
            bail!("Template not found!");
        }

        let template = fs::read_to_string(template).await?;
        Ok(toml::from_str(&template)?)
    }
}

#[handler(SsUploadCommand)]
async fn handle_upload(this: &SsUploadCommand, config_root: &PathBuf) -> anyhow::Result<()> {
    let client = Client::auto(this.credential(config_root).await?).await?;
    let mut parts = Vec::with_capacity(this.videos.len());

    // 上传分P
    for video in &this.videos {
        let (sx, mut rx) = tokio::sync::mpsc::channel(1);
        let metadata = tokio::fs::metadata(&video).await?;
        let total_size = metadata.len() as usize;

        let upload = client.upload_video_part(video, total_size, sx);
        tokio::pin!(upload);

        let mut uploaded_size = 0;

        loop {
            tokio::select! {
                Some(size) = rx.recv() => {
                    // 上传进度
                    uploaded_size += size;
                }
                video = &mut upload => {
                    // 上传完成
                    parts.push(video?);
                    break;
                }
            }
        }
    }

    // 提交投稿
    let template = this.template(&config_root).await?;
    let cover = client.upload_cover(&template.cover).await?;
    client.submit(template.into_video(parts, cover).await?).await
}
