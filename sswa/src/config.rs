use anyhow::Context;
use serde::Deserialize;
use ssup::UploadLine;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Config {
    /// 手动选择线路
    line: Option<String>,
    /// 默认投稿用户
    pub default_user: Option<String>,
    /// 默认是否缩放封面
    scale_cover: Option<bool>,
}

impl Config {
    pub(crate) fn new() -> Self {
        Config {
            line: None,
            default_user: None,
            scale_cover: None,
        }
    }

    pub(crate) fn need_scale_cover(&self) -> bool {
        self.scale_cover.unwrap_or(false)
    }

    pub(crate) async fn line(&self) -> anyhow::Result<UploadLine> {
        let line = self.line.as_deref().unwrap_or("auto");
        let line = match line {
            "kodo" => UploadLine::kodo(),
            "bda2" => UploadLine::bda2(),
            "ws" => UploadLine::ws(),
            "qn" => UploadLine::qn(),
            "auto" => UploadLine::auto().await.with_context(|| "auto select upload line")?,
            _ => unimplemented!(),
        };
        Ok(line)
    }
}
