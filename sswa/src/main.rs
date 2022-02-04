use crate::args::Args;
use clap::Parser;
use anni_clap_handler::Handler;

pub mod config;
mod args;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Args::parse().run().await
}

// 加载配置
// pub async fn load_config(&mut self, config: &Config) -> anyhow::Result<()> {
//     // 读取用户帐号信息
//     let ref account = config.config.account_path;
//     let mut account = File::open(account).await?;
//     let mut str = String::new();
//     account.read_to_string(&mut str).await?;
//     self.load_login_info(&toml::from_str(&str)?);
//
//     Ok(())
// }
