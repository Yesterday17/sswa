use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use anyhow::bail;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Url;
use reqwest_cookie_store::CookieStoreMutex;
use tokio::sync::mpsc::Sender;
use crate::constants::USER_AGENT;
use crate::credential::Credential;
use crate::line::UploadLine;
use crate::video::{VideoPart, Video};

/// 上传使用的客户端
pub struct Client {
    pub(crate) client: reqwest::Client,
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

    /// 上传单个分P
    pub async fn upload<P>(&self, video: P, total_size: usize, sx: Sender<usize>) -> anyhow::Result<VideoPart>
        where P: AsRef<Path> {
        self.line.upload(self, video, total_size, sx).await
    }

    /// 投稿
    pub async fn submit(&self, form: Video) -> anyhow::Result<()> {
        let ret: serde_json::Value = self.client
            .post(format!(
                "https://member.bilibili.com/x/vu/client/add?access_key={}",
                self.credential.token_info.access_token
            ))
            .json(&form)
            .send()
            .await?
            .json()
            .await?;
        if ret["code"] == 0 {
            // Ok(ret)
            Ok(())
        } else {
            bail!("{}", ret)
        }
    }
}
