use crate::constants::{CONCURRENCY, USER_AGENT};
use crate::uploader::utils::read_chunk;
use crate::video::VideoPart;
use anyhow::{anyhow, bail};
use futures::{Stream, StreamExt, TryStreamExt};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

pub struct Upos {
    client: ClientWithMiddleware,
    bucket: UposBucket,
    url: String,
    upload_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UposBucket {
    chunk_size: usize,
    auth: String,
    endpoint: String,
    biz_id: usize,
    upos_uri: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Protocol<'a> {
    upload_id: &'a str,
    chunks: usize,
    total: u64,
    chunk: usize,
    size: usize,
    part_number: usize,
    start: usize,
    end: usize,
}

impl Upos {
    pub async fn from(bucket: UposBucket) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("X-Upos-Auth", HeaderValue::from_str(&bucket.auth)?);
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT.read().as_str())
            .default_headers(headers)
            .timeout(Duration::new(300, 0))
            .build()
            .unwrap();
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        let url = format!(
            "https:{}/{}",
            bucket.endpoint,
            bucket.upos_uri.replace("upos://", "")
        );
        let ret: serde_json::Value = client
            .post(format!("{url}?uploads&output=json"))
            .send()
            .await?
            .json()
            .await?;
        let upload_id = ret["upload_id"].as_str().unwrap().into();
        Ok(Upos {
            client,
            bucket,
            url,
            upload_id,
        })
    }

    pub(crate) async fn upload_stream<P>(
        &self,
        file_path: P,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<(Value, usize)>> + '_>
    where
        P: AsRef<Path>,
    {
        let file = tokio::fs::File::open(file_path.as_ref()).await?;

        let total_size = file.metadata().await?.len();
        let chunk_size = self.bucket.chunk_size;
        let chunks_num = (total_size as f64 / chunk_size as f64).ceil() as usize; // 获取分块数量

        let client = &self.client;
        let url = &self.url;
        let upload_id = &*self.upload_id;
        let stream = read_chunk(file, chunk_size)
            .enumerate()
            .map(move |(i, chunk)| async move {
                let chunk = chunk?;
                let len = chunk.len();
                let params = Protocol {
                    upload_id,
                    chunks: chunks_num,
                    total: total_size,
                    chunk: i,
                    size: len,
                    part_number: i + 1,
                    start: i * chunk_size,
                    end: i * chunk_size + len,
                };

                let response = client.put(url).query(&params).body(chunk).send().await?;
                response.error_for_status()?;

                Ok::<_, anyhow::Error>((
                    json!({"partNumber": params.chunk + 1, "eTag": "etag"}),
                    len,
                ))
            })
            .buffer_unordered(*CONCURRENCY.read());
        Ok(stream)
    }

    // TODO
    pub async fn upload<P>(&self, path: P, sx: Sender<usize>) -> anyhow::Result<VideoPart>
    where
        P: AsRef<Path>,
    {
        let parts: Vec<_> = self
            .upload_stream(path.as_ref())
            .await?
            .map(|union| Ok::<_, reqwest_middleware::Error>(union?.0))
            .try_collect()
            .await?;
        self.get_ret_video_info(&parts, path).await
    }

    pub(crate) async fn get_ret_video_info<P>(
        &self,
        parts: &[Value],
        path: P,
    ) -> anyhow::Result<VideoPart>
    where
        P: AsRef<Path>,
    {
        let value = json!({
            "name": path.as_ref().file_name().and_then(OsStr::to_str),
            "uploadId": self.upload_id,
            "biz_id": self.bucket.biz_id,
            "output": "json",
            "profile": "ugcupos/bup"
        });
        let res: serde_json::Value = self
            .client
            .post(&self.url)
            .query(&value)
            .json(&json!({ "parts": parts }))
            .send()
            .await?
            .json()
            .await?;
        if res["OK"] != 1 {
            bail!("{}", res)
        }
        Ok(VideoPart {
            title: path
                .as_ref()
                .file_stem()
                .map(|p| p.to_string_lossy().to_string()),
            filename: Path::new(&self.bucket.upos_uri)
                .file_stem()
                .ok_or(anyhow!("no file stem found"))?
                .to_string_lossy()
                .to_string(),
            desc: "".to_string(),
        })
    }
}
