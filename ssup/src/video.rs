use serde::{Serialize, Deserialize};

/// 视频
#[derive(Serialize)]
pub struct Video {
    /// 1 为自制，2 为转载
    pub copyright: u8,
    pub source: String,
    /// 分区号
    pub tid: u16,
    /// 封面链接
    pub cover: String,
    pub title: String,
    /// 为 0
    pub desc_format_id: u8,
    /// 描述
    pub desc: String,
    /// 动态文本
    pub dynamic: String,
    pub subtitle: Subtitle,
    /// 由 `,` 连接的 Tag
    pub tag: String,
    /// 分P
    pub videos: Vec<VideoPart>,
    /// 秒为单位的定时投稿时间
    #[serde(rename = "dtime")]
    pub display_time: Option<i32>,
    pub open_subtitle: bool,
}

#[derive(Serialize)]
pub struct Subtitle {
    pub open: i8,
    pub lan: String,
}

/// 分P
#[derive(Serialize, Deserialize)]
pub struct VideoPart {
    pub title: Option<String>,
    pub filename: String,
    pub desc: String,
}
