use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use std::path::Path;
use std::time::Instant;
use anyhow::bail;
use futures::TryStreamExt;
use serde_json::json;
use tokio::sync::mpsc::Sender;
use crate::client::Client;
use crate::uploader::*;
use crate::video::VideoPart;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Uploader {
    Upos,
    Kodo,
    Bos,
    Gcs,
    Cos,
}

impl Uploader {
    fn profile(&self) -> &'static str {
        if let Uploader::Upos = self {
            "ugcupos/bup"
        } else {
            "ugcupos/bupfetch"
        }
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

    pub(crate) async fn pre_upload<T, P>(&self, client: &Client, file_path: P, total_size: usize) -> anyhow::Result<T>
        where T: DeserializeOwned, P: AsRef<Path> {
        let file_name = file_path.as_ref().file_name().ok_or("No filename").unwrap().to_str();

        let query: serde_json::Value = json!({
            "r": self.os,
            "profile": self.os.profile(),
            "ssl": 0u8,
            "version": "2.10.4",
            "build": 2110400,
            "name": file_name,
            "size": total_size,
        });
        log::debug!("Pre uploading with query: {}", query);
        Ok(client
            .client
            .get(format!("https://member.bilibili.com/preupload?{}", self.query))
            .query(&query)
            .send()
            .await?
            .json()
            .await?
        )
    }

    pub(crate) async fn upload<P>(&self, client: &Client, file_path: P, total_size: usize, sx: Sender<usize>) -> anyhow::Result<VideoPart>
        where P: AsRef<Path> {
        match self.os {
            Uploader::Upos => {
                log::debug!("Uploading with upos");
                let bucket = self.pre_upload(client, file_path.as_ref(), total_size).await?;
                let upos = Upos::from(bucket).await?;

                let mut parts = Vec::new();
                let stream = upos.upload_stream(file_path.as_ref()).await?;
                tokio::pin!(stream);

                while let Some((part, size)) = stream.try_next().await? {
                    parts.push(part);
                    sx.send(size).await?;
                }
                upos.get_ret_video_info(&parts, file_path.as_ref()).await
            }
            Uploader::Kodo => {
                log::debug!("Uploading with kodo");
                let bucket = self.pre_upload(client, file_path.as_ref(), total_size).await?;
                Kodo::from(bucket)
                    .await?
                    .upload(file_path, sx)
                    .await
            }
            _ => unimplemented!(),
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

    pub fn kodo() -> Self {
        Self {
            os: Uploader::Kodo,
            probe_url: "//up-na0.qbox.me/crossdomain.xml".to_string(),
            query: "bucket=bvcupcdnkodobm&probe_version=20211012".to_string(),
            cost: 0,
        }
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
