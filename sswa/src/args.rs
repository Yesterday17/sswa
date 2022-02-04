use std::path::PathBuf;
use clap::Parser;
use anni_clap_handler::{Handler, handler};

#[derive(Parser, Handler, Debug, Clone)]
pub struct Args {
    /// 配置文件所在的目录，留空时默认通过 directories-next 获取
    #[clap(short, long)]
    config_root: Option<PathBuf>,

    /// 执行的子命令
    #[clap(subcommand)]
    command: SsCommand,
}

#[derive(Parser, Handler, Debug, Clone)]
pub enum SsCommand {
    /// 输出配置文件所在路径
    Config(SsConfigCommand),
    /// 用户帐号相关功能
    Credential(SsCredentialCommand),
    /// 上传视频相关功能
    Upload(SsUploadCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct SsConfigCommand;

#[handler(SsConfigCommand)]
async fn handle_config() -> anyhow::Result<()> {
    Ok(())
}

#[derive(Parser, Debug, Clone)]
pub struct SsCredentialCommand;

#[handler(SsCredentialCommand)]
async fn handle_credential() -> anyhow::Result<()> {
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
