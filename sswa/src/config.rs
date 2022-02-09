use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// 手动选择线路
    pub line: Option<String>,
    pub default_user: Option<String>,
    scale_cover: Option<bool>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            line: None,
            default_user: None,
            scale_cover: None,
        }
    }

    pub fn scale_cover(&self) -> bool {
        self.scale_cover.unwrap_or(false)
    }
}
