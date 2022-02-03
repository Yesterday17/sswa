use serde::{Serialize, Deserialize};

/// 视频
#[derive(Serialize, Deserialize)]
pub struct Video {
    pub videos: Vec<VideoPart>,
}

/// 分P
#[derive(Serialize, Deserialize)]
pub struct VideoPart {
    pub title: Option<String>,
    pub filename: String,
    pub desc: String,
}

#[derive(Serialize)]
pub struct VideoSubmitForm {
    /// 1 为自制，2 为转载
    copyright: u8,
    source: String,
    /// 分区号
    tid: u16,
    /// 封面链接
    cover: String,
    title: String,
    /// 为 0
    desc_format_id: u8,
    /// 描述
    desc: String,
    /// 动态文本
    dynamic: String,
    subtitle: Subtitle,
    /// 由 [,] 连接的 Tag
    tag: String,
    /// 分P
    videos: Vec<VideoPart>,
    /// 秒为单位的定时投稿时间
    #[serde(rename = "dtime")]
    display_time: Option<i32>,
    open_subtitle: bool,
}

#[derive(Serialize)]
pub struct Subtitle {
    open: i8,
    lan: String,
}