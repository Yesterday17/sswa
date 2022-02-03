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
    pub line: String,
    pub account_path: PathBuf,
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