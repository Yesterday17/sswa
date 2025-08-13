use crate::constants::USER_AGENT;
use crate::credential::{Credential, ResponseData, ResponseValue};
use crate::line::UploadLine;
use crate::video::{EditVideo, EditVideoPart, Video, VideoCardItem, VideoId, VideoPart};
use anyhow::bail;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Url;
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::mpsc::Sender;

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
        headers.insert(
            "Referer",
            HeaderValue::from_static("https://www.bilibili.com/"),
        );
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
    where
        P: AsRef<Path>,
    {
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
            } => {
                return Ok(value["url"]
                    .as_str()
                    .ok_or(anyhow::anyhow!("cover_up error"))?
                    .into())
            }
            _ => {
                unreachable!()
            }
        };
    }

    /// 上传单个分P
    pub async fn upload_video_part<P>(
        &self,
        video: P,
        total_size: usize,
        sx: Sender<usize>,
        part_name: Option<String>,
    ) -> anyhow::Result<VideoPart>
    where
        P: AsRef<Path>,
    {
        let mut part = self.line.upload(self, video, total_size, sx).await?;
        if let Some(name) = part_name {
            part.title = Some(name);
        }
        Ok(part)
    }

    pub fn upload_line(&self) -> &UploadLine {
        &self.line
    }

    /// 查看现有投稿信息
    pub async fn get_video(&self, id: &VideoId) -> anyhow::Result<EditVideo> {
        let id = match id {
            VideoId::AId(aid) => format!("aid={aid}"),
            VideoId::BVId(bvid) => format!("bvid={bvid}"),
        };

        let ret: serde_json::Value = self
            .client
            .get(format!(
                "https://member.bilibili.com/x/client/archive/view?{id}"
            ))
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
            aid: data["aid"].as_u64().unwrap(),
            copyright: data["copyright"].as_i64().unwrap(),
            source: data["source"].as_str().unwrap().into(),
            tid: data["tid"].as_u64().unwrap() as u16,
            cover: data["cover"].as_str().unwrap().into(),
            title: data["title"].as_str().unwrap().into(),
            desc_format_id: data["desc_format_id"].as_i64().unwrap(),
            desc: data["desc"].as_str().unwrap().into(),
            dynamic: data["dynamic"].as_str().unwrap().into(),
            tag: data["tag"].as_str().unwrap().into(),
            videos: ret["videos"]
                .as_array()
                .unwrap()
                .iter()
                .map(|video| EditVideoPart {
                    title: Some(video["title"].as_str().unwrap().into()),
                    filename: video["filename"].as_str().unwrap().into(),
                    desc: video["desc"].as_str().unwrap().into(),
                    cid: Some(video["cid"].as_u64().unwrap()),
                    duration: video["duration"].as_u64().unwrap(),
                })
                .collect(),
            display_time: data["dtime"].as_i64(),
        };
        Ok(video)
    }

    /// 投稿
    pub async fn submit(&self, form: &Video) -> anyhow::Result<()> {
        let ret: serde_json::Value = self
            .client
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

    pub async fn submit_by_app(&self, studio: &Video) -> anyhow::Result<()> {
        let payload = {
            let mut payload = json!({
                "access_key": self.credential.token_info.access_token,
                "appkey": "4409e2ce8ffd12b8",
                "build": 7800300,
                "c_locale": "zh-Hans_CN",
                "channel": "bili",
                "disable_rcmd": 0,
                "mobi_app": "android",
                "platform": "android",
                "s_locale": "zh-Hans_CN",
                "statistics": "\"appId\":1,\"platform\":3,\"version\":\"7.80.0\",\"abtest\":\"\"",
                "ts": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            });

            let urlencoded = serde_urlencoded::to_string(&payload)?;
            let sign = crate::credential::Credential::sign(
                &urlencoded,
                "59b43e04ad6965f34319062b478f83dd",
            );
            payload["sign"] = serde_json::Value::from(sign);
            payload
        };

        let ret: Value = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 BiliDroid/7.80.0 (bbcallen@gmail.com) os/android model/MI 6 mobi_app/android build/7800300 channel/bili innerVer/7800310 osVer/13 network/2")
            .timeout(Duration::new(60, 0))
            .build()?
            .post("https://member.bilibili.com/x/vu/app/add")
            .query(&payload)
            .json(studio)
            .send()
            .await?
            .json()
            .await?;
        log::info!("{:?}", ret);
        if ret["code"] == 0 {
            Ok(())
        } else {
            anyhow::bail!("{ret:?}")
        }
    }

    /// 修改现有投稿
    pub async fn submit_edit(&self, form: &EditVideo) -> anyhow::Result<()> {
        let ret: serde_json::Value = self
            .client
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

    /// 修改投稿分段章节
    pub async fn edit_card(
        &self,
        aid: u64,
        cid: u64,
        cards: Vec<VideoCardItem>,
        permanent: bool,
    ) -> anyhow::Result<()> {
        let csrf = self.credential.cookie_info.get("bili_jct").unwrap();
        let cards = serde_json::to_string(&cards)?;
        let response: serde_json::Value = self
            .client
            .post("https://member.bilibili.com/x/web/card/submit")
            .form(&json!({
                "aid": aid,
                "cid": cid,
                "type": 2, // TODO: why 2?
                "cards": cards,
                "permanent": permanent,
                "csrf": csrf,
            }))
            .send()
            .await?
            .json()
            .await?;

        if response["code"] == 0 {
            Ok(())
        } else {
            bail!("{}", response)
        }
    }
}
