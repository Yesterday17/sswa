use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use cookie::Cookie;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Url;
use reqwest_cookie_store::CookieStoreMutex;
use crate::config::Config;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use crate::constants::USER_AGENT;
use crate::line::UploadLine;
use crate::video::VideoPart;

pub struct Client {
    pub client: reqwest::Client,
    cookie_store: Arc<CookieStoreMutex>,

    line: UploadLine,
}

impl Client {
    pub fn new(upload_line: UploadLine) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("Referer", HeaderValue::from_static("https://www.bilibili.com/"));
        headers.insert("Connection", HeaderValue::from_static("keep-alive"));

        let cookie_store = cookie_store::CookieStore::default();
        let cookie_store = CookieStoreMutex::new(cookie_store);
        let cookie_store = Arc::new(cookie_store);
        Self {
            client: reqwest::Client::builder()
                .cookie_provider(cookie_store.clone())
                .user_agent(USER_AGENT.read().as_str())
                .default_headers(headers)
                .timeout(Duration::new(60, 0))
                .build()
                .unwrap(),
            cookie_store,
            line: upload_line,
        }
    }

    pub async fn auto() -> anyhow::Result<Self> {
        Ok(Self::new(UploadLine::auto().await?))
    }

    /// 加载配置
    pub async fn load_config(&mut self, config: &Config) -> anyhow::Result<()> {
        // 读取用户帐号信息
        // TODO： 解密
        let ref account = config.config.account_path;
        let mut account = File::open(account).await?;
        let mut str = String::new();
        account.read_to_string(&mut str).await?;
        self.load_login_info(&toml::from_str(&str)?);

        Ok(())
    }

    /// 加载 LoginInfo 进入 Client
    fn load_login_info(&mut self, info: &LoginInfo) {
        let mut store = self.cookie_store.lock().unwrap();
        let link = Url::parse("https://bilibili.com").unwrap();
        for cookie in &info.cookie_info.cookies {
            store.insert_raw(&cookie.to_cookie(), &link).unwrap();
        }
    }

    /// 上传多个分P，返回分P列表
    pub async fn upload(&self, videos: &[PathBuf]) -> anyhow::Result<Vec<VideoPart>> {
        let mut parts = Vec::with_capacity(videos.len());

        for video in videos {
            let (sx, mut rx) = tokio::sync::mpsc::channel(1);
            let metadata = tokio::fs::metadata(&video).await?;
            let total_size = metadata.len() as usize;

            let upload = self.line.upload(self, video, total_size, sx);
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
        Ok(parts)
    }
}

/// 存储用户的登录信息
#[derive(Serialize, Deserialize)]
struct LoginInfo {
    pub cookie_info: CookieInfo,
    pub sso: Vec<String>,
    pub token_info: TokenInfo,
}

/// 存储 Cookie 信息
#[derive(Serialize, Deserialize)]
struct CookieInfo {
    cookies: Vec<CookieEntry>,
}

/// Cookie 项
#[derive(Serialize, Deserialize)]
struct CookieEntry {
    name: String,
    value: String,
}

impl CookieEntry {
    fn to_cookie(&self) -> Cookie {
        Cookie::build(self.name.clone(), self.value.clone())
            .domain("bilibili.com")
            .finish()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TokenInfo {
    pub access_token: String,
    expires_in: u32,
    mid: u32,
    refresh_token: String,
}
