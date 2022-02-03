use std::collections::HashMap;
use std::path::PathBuf;
use crate::video::VideoPart;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Config {
    pub config: ShushuConfig,
    pub template: HashMap<String, VideoTemplate>,
}

#[derive(Serialize, Deserialize)]
pub struct ShushuConfig {
    /// 手动选择线路
    pub line: String,
    /// 用户帐号所在路径
    pub account_path: PathBuf,
    /// 加密用户帐号的密码
    pub account_pass: Option<String>,
}

/// 分P描述格式
pub enum ConfigVideoPart {
    /// 简单分P描述，根据视频文件名自动生成
    Simple(PathBuf),
    /// 详细分P描述，需要手动填写各字段
    Detailed(VideoPart),
}

#[derive(Deserialize)]
pub struct VideoTemplate {
}