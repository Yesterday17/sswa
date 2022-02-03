use ssup::{Client, Credential};
use ssup::UploadLine;

pub mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = Client::new(UploadLine::kodo());
    let qrcode = Credential::get_qrcode().await?;
    let credential = Credential::from_qrcode(qrcode).await?;
    client.load_credential(&credential);

    Ok(())
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
