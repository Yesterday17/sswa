use std::collections::HashMap;
use std::path::PathBuf;
use clap::Parser;
use anni_clap_handler::{Context, Handler, handler};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use ssup::Credential;
use crate::config::Config;

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

        // 读取用户帐号信息
        let account: HashMap<String, Credential> = match File::open(&config.account_path).await {
            Ok(mut account) => {
                // TODO: password
                let mut account_str = String::new();
                account.read_to_string(&mut account_str).await?;
                toml::from_str(&account_str)?
            }
            Err(_) => HashMap::new(),
        };

        ctx.insert(config_root);
        ctx.insert(config);
        ctx.insert(account);
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

    /// 可选的投稿帐号
    #[clap(long)]
    account: Option<String>,

    /// 待投稿的视频
    videos: Vec<PathBuf>,
}

#[handler(SsUploadCommand)]
async fn handle_upload() -> anyhow::Result<()> {
    Ok(())
}
