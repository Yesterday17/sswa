use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use anyhow::bail;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Url;
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::json;
use tokio::fs;
use tokio::sync::mpsc::Sender;
use crate::constants::USER_AGENT;
use crate::credential::{Credential, ResponseData, ResponseValue};
use crate::line::UploadLine;
use crate::video::{VideoPart, Video, EditVideo, EditVideoPart, VideoId};

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

    /// 上传封面
    pub async fn upload_cover<P>(&self, cover: P) -> anyhow::Result<String>
        where P: AsRef<Path> {
        let cover = fs::read(cover).await?;

        let csrf = self.credential.cookie_info.get("bili_jct").unwrap();
        let response: ResponseData = self
            .client
            .post("https://member.bilibili.com/x/vu/web/cover/up")
            .form(&json!({
                "cover": format!("data:image/jpeg;base64,{}", base64::encode(cover)),
                "csrf": csrf,
            }))
            .send()
            .await?
            .json()
            .await?;
        match &response {
            ResponseData {
                data: ResponseValue::Value(value),
                ..
            } if value.is_null() => bail!("{response:?}"),
            ResponseData {
                data: ResponseValue::Value(value),
                ..
            } => return Ok(value["url"]
                .as_str()
                .ok_or(anyhow::anyhow!("cover_up error"))?
                .into()),
            _ => {
                unreachable!()
            }
        };
    }

    /// 上传单个分P
    pub async fn upload_video_part<P>(&self, video: P, total_size: usize, sx: Sender<usize>, part_name: Option<String>) -> anyhow::Result<VideoPart>
        where P: AsRef<Path> {
        let mut part = self.line.upload(self, video, total_size, sx).await?;
        if let Some(name) = part_name {
            part.title = Some(name);
        }
        Ok(part)
    }

    /// 查看现有投稿信息
    pub async fn get_video(&self, id: &VideoId) -> anyhow::Result<EditVideo> {
        let id = match id {
            VideoId::AId(aid) => format!("aid={aid}"),
            VideoId::BVId(bvid) => format!("bvid={bvid}"),
        };

        let ret: serde_json::Value = self.client
            .get(format!("https://member.bilibili.com/x/client/archive/view?{id}"))
            .send()
            .await?
            .json()
            .await?;

        if ret["code"] != 0 {
            bail!("{:?}", ret);
        }

        let ret = &ret["data"];
        let data = &ret["archive"];
        let video = EditVideo {
            copyright: data["copyright"].as_u64().unwrap() as u8,
            source: data["source"].as_str().unwrap().into(),
            tid: data["tid"].as_u64().unwrap() as u16,
            cover: data["cover"].as_str().unwrap().into(),
            title: data["title"].as_str().unwrap().into(),
            desc_format_id: data["desc_format_id"].as_u64().unwrap() as u8,
            desc: data["desc"].as_str().unwrap().into(),
            dynamic: data["dynamic"].as_str().unwrap().into(),
            tag: data["tag"].as_str().unwrap().into(),
            videos: ret["videos"].as_array().unwrap().iter().map(|video| {
                EditVideoPart {
                    title: Some(video["title"].as_str().unwrap().into()),
                    filename: video["filename"].as_str().unwrap().into(),
                    desc: video["desc"].as_str().unwrap().into(),
                    cid: Some(video["cid"].as_u64().unwrap()),
                }
            }).collect(),
        };
        Ok(video)
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

    /// 修改现有投稿
    pub async fn submit_edit(&self, form: EditVideo) -> anyhow::Result<()> {
        let ret: serde_json::Value = self.client
            .post(format!(
                "https://member.bilibili.com/x/vu/client/edit?access_key={}",
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
