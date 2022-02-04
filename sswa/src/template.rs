use ssup::video::{Subtitle, VideoSubmitForm};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct VideoTemplate {
    /// 视频标题
    title: String,
    /// 简介
    description: String,
    /// 转载来源
    forward_source: Option<String>,
    /// 分区号
    tid: u16,
    /// 封面图片
    cover: String,
    /// 动态文本
    dynamic_text: String,
    /// 标签
    tags: Vec<String>,
}

impl VideoTemplate {
    pub async fn into_submit(self) -> VideoSubmitForm {
        VideoSubmitForm {
            copyright: match &self.forward_source {
                Some(source) if !source.is_empty() => 2,
                _ => 1,
            },
            source: self.forward_source.unwrap_or("".into()),
            tid: self.tid,
            cover: self.cover,
            title: self.title,
            desc_format_id: 0,
            desc: self.description,
            dynamic: self.dynamic_text,
            subtitle: Subtitle {
                open: 0,
                lan: "".to_string(),
            },
            tag: self.tags.join(","),
            videos: vec![],
            display_time: None,
            open_subtitle: false,
        }
    }
}
