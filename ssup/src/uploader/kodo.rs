use anyhow::{anyhow, bail, Result};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use futures::{StreamExt, TryStreamExt};
use reqwest::header;
use reqwest::header::{HeaderMap, HeaderName};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use tokio::fs::File;
use tokio::sync::mpsc::Sender;
use crate::constants::USER_AGENT;
use crate::uploader::utils::read_chunk;
use crate::video::VideoPart;

pub struct Kodo {
    client: ClientWithMiddleware,
    bucket: Bucket,
    url: String,
}

impl Kodo {
    pub async fn from(bucket: Bucket) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("UpToken {}", bucket.uptoken).parse()?,
        );
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT.read().as_str())
            .default_headers(headers)
            .timeout(Duration::new(60, 0))
            .build()
            .unwrap();
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        let url = format!("https:{}/mkblk", bucket.endpoint); // 视频上传路径
        Ok(Kodo {
            client,
            bucket,
            url,
        })
    }

    pub async fn upload<P>(self, path: P, sx: Sender<usize>) -> Result<VideoPart>
        where P: AsRef<Path> {
        let file = File::open(path.as_ref()).await?;
        let total_size = file.metadata().await?.len();
        let chunk_size = 4194304;
        let mut parts = Vec::new();

        let client = &self.client;
        let url = &self.url;

        let stream = read_chunk(file, chunk_size)
            .enumerate()
            .map(|(i, chunk)| async move {
                let chunk = chunk?;
                let len = chunk.len();
                let url = format!("{url}/{len}");
                let ctx: serde_json::Value = client.post(url)
                    .body(chunk)
                    .send()
                    .await?
                    .json()
                    .await?;
                Ok::<_, reqwest_middleware::Error>((
                    Ctx {
                        index: i,
                        ctx: ctx["ctx"].as_str().unwrap_or_default().into(),
                    },
                    len,
                ))
            })
            .buffer_unordered(3);
        tokio::pin!(stream);
        while let Some((part, size)) = stream.try_next().await? {
            parts.push(part);
            sx.send(size).await?;
        }
        parts.sort_by_key(|x| x.index);
        let key = base64::encode_config(self.bucket.key, base64::URL_SAFE);
        self.client
            .post(format!(
                "https:{}/mkfile/{total_size}/key/{key}",
                self.bucket.endpoint,
            ))
            .body(
                parts
                    .iter()
                    .map(|x| &x.ctx[..])
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .send()
            .await?;
        let mut headers = HeaderMap::new();
        for (key, value) in self.bucket.fetch_headers {
            headers.insert(HeaderName::from_str(&key)?, value.parse()?);
        }
        let result: serde_json::Value = self
            .client
            .post(format!("https:{}", self.bucket.fetch_url))
            .headers(headers)
            .send()
            .await?
            .json()
            .await?;
        Ok(match result.get("OK") {
            Some(x) if x.as_i64().ok_or(anyhow!("kodo fetch err"))? != 1 => {
                bail!("{result}")
            }
            _ => VideoPart {
                title: path.as_ref()
                    .file_stem()
                    .map(|x| x.to_string_lossy().into_owned()),
                filename: self.bucket.bili_filename,
                desc: "".into(),
            },
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Ctx {
    index: usize,
    ctx: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Bucket {
    bili_filename: String,
    fetch_url: String,
    endpoint: String,
    uptoken: String,
    key: String,
    fetch_headers: HashMap<String, String>,
}
