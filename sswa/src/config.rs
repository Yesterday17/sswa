use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// 手动选择线路
    pub line: Option<String>,
    pub default_user: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            line: None,
            default_user: None,
        }
    }
}
