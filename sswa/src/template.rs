use ssup::video::{Subtitle, VideoPart, Video};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct VideoTemplate {
    /// 视频标题
    title: TemplateString,
    /// 简介
    description: TemplateString,
    /// 转载来源
    forward_source: Option<TemplateString>,
    /// 分区号
    tid: u16,
    /// 封面图片
    pub cover: String,
    /// 动态文本
    dynamic_text: TemplateString,
    /// 标签
    tags: Vec<String>,
}

impl VideoTemplate {
    pub async fn into_video(self, parts: Vec<VideoPart>, cover: String) -> anyhow::Result<Video> {
        Ok(Video {
            copyright: match &self.forward_source {
                Some(source) if !source.is_empty() => 2,
                _ => 1,
            },
            source: self.forward_source
                .and_then(|s| s.into_string().ok())
                .unwrap_or("".into()),
            tid: self.tid,
            cover,
            title: self.title.into_string()?,
            desc_format_id: 0,
            desc: self.description.into_string()?,
            dynamic: self.dynamic_text.into_string()?,
            subtitle: Subtitle {
                open: 0,
                lan: "".to_string(),
            },
            tag: self.tags.join(","),
            videos: parts,
            display_time: None,
            open_subtitle: false,
        })
    }
}

#[derive(Deserialize)]
struct TemplateString(String);

impl TemplateString {
    fn into_string(self) -> anyhow::Result<String> {
        let regex = regex::Regex::new(r"\{\{(.*?)\}\}").unwrap();
        let matches = regex.captures_iter(&self.0)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>();
        let mut result = self.0;
        if !matches.is_empty() {
            for variable in matches.iter() {
                result = result.replace(
                    &format!("{{{{{variable}}}}}"),
                    &dotenv::var(variable)
                        .map_err(|_| anyhow::anyhow!("variable `{}` not provided", variable))?,
                );
            }
        }
        Ok(result)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}