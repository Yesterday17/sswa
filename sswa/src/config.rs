use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    /// 手动选择线路
    pub line: Option<String>,
    // 加密用户帐号的密码
    // pub account_pass: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            line: None,
            // TODO: 默认生成随机密码
            // account_pass: None,
        }
    }
}
