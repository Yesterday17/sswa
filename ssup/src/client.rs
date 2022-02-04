use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use anyhow::bail;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Url;
use reqwest_cookie_store::CookieStoreMutex;
use crate::constants::USER_AGENT;
use crate::credential::Credential;
use crate::line::UploadLine;
use crate::video::{VideoPart, Video};

/// 上传使用的客户端
pub struct Client {
    pub client: reqwest::Client,
    cookie_store: Arc<CookieStoreMutex>,

    line: UploadLine,
    credential: Credential,
}

impl Client {
    pub fn new(upload_line: UploadLine, credential: Credential) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("Referer", HeaderValue::from_static("https://www.bilibili.com/"));
        headers.insert("Connection", HeaderValue::from_static("keep-alive"));

        let cookie_store = cookie_store::CookieStore::default();
        let cookie_store = CookieStoreMutex::new(cookie_store);
        let cookie_store = Arc::new(cookie_store);

        let mut me = Self {
            client: reqwest::Client::builder()
                .cookie_provider(cookie_store.clone())
                .user_agent(USER_AGENT.read().as_str())
                .default_headers(headers)
                .timeout(Duration::new(60, 0))
                .build()
                .unwrap(),
            cookie_store,
            line: upload_line,
            credential,
        };

        me.load_credential();
        me
    }

    pub async fn auto(credential: Credential) -> anyhow::Result<Self> {
        Ok(Self::new(UploadLine::auto().await?, credential))
    }

    /// 加载 LoginInfo 进入 Client
    fn load_credential(&mut self) {
        let mut store = self.cookie_store.lock().unwrap();
        let link = Url::parse("https://bilibili.com").unwrap();
        for cookie in &self.credential.cookie_info.cookies {
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

    /// 投稿
    pub async fn submit(&self, form: Video) -> anyhow::Result<()> {
        let ret: serde_json::Value = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/63.0.3239.108")
            .timeout(Duration::new(60, 0))
            .build()?
            .post(format!(
                "http://member.bilibili.com/x/vu/client/add?access_key={}",
                self.credential.token_info.access_token
            ))
            .json(&form)
            .send()
            .await?
            .json()
            .await?;
        println!("{}", ret);
        if ret["code"] == 0 {
            println!("投稿成功");
            // Ok(ret)
            Ok(())
        } else {
            bail!("{}", ret)
        }
    }
}
