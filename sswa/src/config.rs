use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Config {
    /// 手动选择线路
    pub line: Option<String>,
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
}
