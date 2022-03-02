use date_time_parser::{DateParser, TimeParser};
use serde::Deserialize;
use ssup::video::{Subtitle, Video, VideoPart};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct VideoTemplate {
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
    #[serde(default)]
    tags: Vec<TemplateString>,
    /// 发布时间
    display_time: Option<String>,
    /// 前缀视频
    #[serde(default)]
    video_prefix: Vec<TemplateString>,
    /// 后缀视频
    #[serde(default)]
    video_suffix: Vec<TemplateString>,
    /// 默认用户
    pub default_user: Option<String>,
    /// 变量解释
    #[serde(default)]
    variables: HashMap<String, String>,
}

impl VideoTemplate {
    /// 校验模板字符串
    pub(crate) fn validate(&self, skip_confirm: bool) -> anyhow::Result<()> {
        let title = self.title.to_string(&self.variables)?;
        let desc = self.description.to_string(&self.variables)?;
        let dynamic = self.dynamic_text.to_string(&self.variables)?;

        let forward_source = if let Some(forward_source) = &self.forward_source {
            forward_source.to_string(&self.variables)?
        } else {
            String::new()
        };

        let mut tags = Vec::new();
        for tag in self.tags.iter() {
            let result = tag.to_string(&self.variables)?;
            if !result.is_empty() {
                tags.push(result);
            }
        }

        self.display_timestamp()?;

        for video in self.video_prefix.iter() {
            video.to_string(&self.variables)?;
        }
        for video in self.video_suffix.iter() {
            video.to_string(&self.variables)?;
        }

        if !skip_confirm {
            eprintln!("标题：{title}\n来源：{forward_source}\n简介：\n---简介开始---\n{desc}\n---简介结束---\n标签：{tags}\n动态：{dynamic}",
                      dynamic = if dynamic.is_empty() { "（空）" } else { &dynamic },
                      tags = tags.join(","),
            );
            let question = requestty::Question::confirm("anonymous")
                .message("投稿信息如上，是否正确？")
                .build();
            let confirm = requestty::prompt_one(question)?;
            if !confirm.as_bool().unwrap_or(false) {
                exit(0);
            }
        }
        Ok(())
    }

    fn forward_source(&self) -> String {
        if let Some(source) = &self.forward_source {
            source.to_string(&self.variables).unwrap()
        } else {
            String::new()
        }
    }

    fn display_timestamp(&self) -> anyhow::Result<Option<i64>> {
        Ok(match &self.display_time {
            Some(display_time) => {
                if display_time.is_empty() {
                    None
                } else {
                    let date = DateParser::parse(&display_time);
                    let time = TimeParser::parse(&display_time);
                    match (date, time) {
                        (Some(date), Some(time)) => Some(date.and_time(time).timestamp()),
                        _ => anyhow::bail!("定时投稿时间解析失败！"),
                    }
                }
            }
            None => None,
        })
    }

    pub(crate) async fn into_video(
        self,
        parts: Vec<VideoPart>,
        cover: String,
    ) -> anyhow::Result<Video> {
        Ok(Video {
            copyright: match &self.forward_source {
                Some(source) if !source.is_empty() => 2,
                _ => 1,
            },
            source: self.forward_source(),
            tid: self.tid,
            cover,
            title: self.title.to_string(&self.variables)?,
            desc_format_id: 0,
            desc: self.description.to_string(&self.variables)?,
            dynamic: self.dynamic_text.to_string(&self.variables)?,
            subtitle: Subtitle {
                open: 0,
                lan: "".to_string(),
            },
            tag: self
                .tags
                .iter()
                .map(|s| s.to_string(&self.variables))
                .filter_map(|s| match s {
                    Ok(s) if !s.is_empty() => Some(s),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(","),
            videos: parts,
            display_time: self.display_timestamp()?,
            open_subtitle: false,
        })
    }

    pub(crate) fn auto_cover(&self) -> bool {
        self.cover.is_empty() || self.cover == "auto"
    }

    pub(crate) fn video_prefix(&self) -> Vec<PathBuf> {
        self.video_prefix
            .iter()
            .map(|s| s.to_string(&self.variables))
            .filter_map(|s| match s {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            })
            .map(|s| PathBuf::from(s))
            .collect()
    }

    pub(crate) fn video_prefix_len(&self) -> usize {
        self.video_prefix.len()
    }

    pub(crate) fn video_suffix(&self) -> Vec<PathBuf> {
        self.video_suffix
            .iter()
            .map(|s| s.to_string(&self.variables))
            .filter_map(|s| match s {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            })
            .map(|s| PathBuf::from(s))
            .collect()
    }

    pub(crate) fn video_suffix_len(&self) -> usize {
        self.video_suffix.len()
    }
}

#[derive(Deserialize)]
struct TemplateString(String);

impl TemplateString {
    fn to_string(&self, description: &HashMap<String, String>) -> anyhow::Result<String> {
        let regex = regex::Regex::new(r"\{\{(.*?)\}\}").unwrap();
        let matches = regex
            .captures_iter(&self.0)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>();
        let mut result = self.0.clone();
        if !matches.is_empty() {
            for variable in matches.iter() {
                let var = dotenv::var(&variable).or_else(|_| -> anyhow::Result<_> {
                    if variable.starts_with("ss") {
                        anyhow::bail!("未定义的预设变量：{}", variable)
                    };

                    let description = match description.get(variable) {
                        Some(description) => format!("{description}({variable})"),
                        None => format!("{variable}"),
                    };

                    // 用户输入变量
                    let question = requestty::Question::input(variable)
                        .message(description)
                        .build();
                    let ans = requestty::prompt_one(question)?;
                    let ans = ans.as_string().unwrap();
                    std::env::set_var(&variable, ans);
                    let ans = dotenv::var(&variable)?;
                    Ok(ans)
                })?;
                result = result.replace(&format!("{{{{{variable}}}}}"), &var);
            }
        }
        Ok(result)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
