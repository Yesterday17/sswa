use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest_cookie_store::CookieStoreMutex;
use crate::config::Config;
use serde::{Deserialize, Serialize};
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

    pub fn load_config(&mut self, config: &Config) {
        //
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

#[derive(Serialize, Deserialize)]
struct LoginInfo {
    pub cookie_info: serde_json::Value,
    pub sso: Vec<String>,
    pub token_info: TokenInfo,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TokenInfo {
    pub access_token: String,
    expires_in: u32,
    mid: u32,
    refresh_token: String,
}
