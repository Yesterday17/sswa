use crate::client::Client;
use crate::uploader::upos::Upos;
use crate::video::VideoPart;
use anyhow::bail;
use futures::TryStreamExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::time::Instant;
use tokio::sync::mpsc::Sender;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Uploader {
    Upos,
}

impl Uploader {
    fn profile(&self) -> &'static str {
        "ugcupos/bup"
    }
}

/// 上传线路
#[derive(Deserialize)]
pub struct UploadLine {
    os: Uploader,
    probe_url: String,
    query: String,
    #[serde(skip)]
    cost: u128,
}

impl UploadLine {
    pub fn probe_url(&self) -> &str {
        &self.probe_url
    }

    pub async fn pre_upload<T, S>(
        &self,
        client: &Client,
        file_name: S,
        total_size: usize,
    ) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
        S: AsRef<str>,
    {
        let query: serde_json::Value = json!({
            "r": self.os,
            "profile": self.os.profile(),
            "ssl": 0u8,
            "version": "2.10.4",
            "build": 2100400,
            "name": file_name.as_ref(),
            "size": total_size,
        });
        log::debug!("Pre uploading with query: {}", query);
        Ok(client
            .client
            .get(format!(
                "https://member.bilibili.com/preupload?{}",
                self.query
            ))
            .query(&query)
            .send()
            .await?
            .json()
            .await?)
    }

    pub(crate) async fn upload<P>(
        &self,
        client: &Client,
        file_path: P,
        total_size: usize,
        sx: Sender<usize>,
    ) -> anyhow::Result<VideoPart>
    where
        P: AsRef<Path>,
    {
        match self.os {
            Uploader::Upos => {
                log::debug!("Uploading with upos");
                let file_name = file_path.as_ref().file_name().unwrap().to_str().unwrap();
                let bucket = self.pre_upload(client, file_name, total_size).await?;
                let upos = Upos::from(bucket).await?;

                let mut parts = Vec::new();
                let stream = upos.upload_stream(file_path.as_ref()).await?;
                tokio::pin!(stream);

                while let Some((part, size)) = stream.try_next().await? {
                    parts.push(part);
                    sx.send(size).await?;
                }
                upos.get_ret_video_info(&parts, file_name).await
            }
        }
    }

    /// 挑选条件最好的线路
    pub async fn auto() -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        struct ProbeResponse {
            #[serde(rename = "OK")]
            ok: u8,
            lines: Vec<UploadLine>,
            probe: serde_json::Value,
        }
        let res: ProbeResponse = reqwest::get("https://member.bilibili.com/preupload?r=probe")
            .await?
            .json()
            .await?;
        if res.ok != 1 {
            bail!("TODO in line.rs");
        }

        let do_probe = if !res.probe["get"].is_null() {
            |url| reqwest::Client::new().get(url)
        } else {
            |url| {
                reqwest::Client::new()
                    .post(url)
                    .body(vec![0; (1024. * 0.1 * 1024.) as usize])
            }
        };
        let mut line_chosen: UploadLine = Default::default();
        for mut line in res.lines {
            let instant = Instant::now();
            if do_probe(format!("https:{}", line.probe_url))
                .send()
                .await?
                .status()
                == 200
            {
                line.cost = instant.elapsed().as_millis();
                if line_chosen.cost > line.cost {
                    line_chosen = line
                }
            };
        }
        Ok(line_chosen)
    }

    pub fn bda2() -> Self {
        Self {
            os: Uploader::Upos,
            probe_url: "//upos-sz-upcdnbda2.bilivideo.com/OK".to_string(),
            query: "upcdn=bda2&probe_version=20211012".to_string(),
            cost: 0,
        }
    }

    pub fn ws() -> Self {
        Self {
            os: Uploader::Upos,
            probe_url: "//upos-sz-upcdnws.bilivideo.com/OK".to_string(),
            query: "upcdn=ws&probe_version=20211012".to_string(),
            cost: 0,
        }
    }

    pub fn qn() -> Self {
        Self {
            os: Uploader::Upos,
            probe_url: "//upos-sz-upcdnqn.bilivideo.com/OK".to_string(),
            query: "upcdn=qn&probe_version=20211012".to_string(),
            cost: 0,
        }
    }
}

impl Default for UploadLine {
    fn default() -> Self {
        let cost = u128::MAX;
        Self {
            cost,
            ..UploadLine::bda2()
        }
    }
}
