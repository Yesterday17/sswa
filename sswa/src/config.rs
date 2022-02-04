use std::path::PathBuf;
use ssup::video::{VideoPart};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    /// 手动选择线路
    pub line: Option<String>,
    /// 用户帐号所在路径
    pub account_path: PathBuf,
    /// 加密用户帐号的密码
    pub account_pass: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            line: None,
            account_path: "account.json".into(),
            // TODO: 默认生成随机密码
            account_pass: None,
        }
    }
}

/// 分P描述格式
pub enum ConfigVideoPart {
    /// 简单分P描述，根据视频文件名自动生成
    Simple(PathBuf),
    /// 详细分P描述，需要手动填写各字段
    Detailed(VideoPart),
}