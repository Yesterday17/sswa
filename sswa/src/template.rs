use date_time_parser::{DateParser, TimeParser};
use serde::Deserialize;
use ssup::video::{Subtitle, Video, VideoPart};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;
use tinytemplate::TinyTemplate;

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
    /// 变量默认值
    #[serde(default)]
    defaults: HashMap<String, TemplateString>,

    #[serde(skip)]
    strings: Vec<String>,
}

impl VideoTemplate {
    /// 构建模板
    pub(crate) fn build(&mut self) -> anyhow::Result<()> {
        let mut template = TinyTemplate::new();
        template.add_template("title".to_string(), &self.title.0)?;
        template.add_template("description".to_string(), &self.description.0)?;
        template.add_template("dynamic".to_string(), &self.dynamic_text.0)?;

        if let Some(forward_source) = &self.forward_source {
            template.add_template("forward_source".to_string(), &forward_source.0)?;
        }

        for (i, tag) in self.tags.iter().enumerate() {
            template.add_template(format!("tag_{}", i), &tag.0)?;
        }

        for (i, video) in self.video_prefix.iter().enumerate() {
            template.add_template(format!("video_prefix_{}", i), &video.0)?;
        }

        for (i, video) in self.video_suffix.iter().enumerate() {
            template.add_template(format!("video_suffix_{}", i), &video.0)?;
        }
        Ok(())
    }

    /// 校验模板字符串
    pub(crate) fn validate(&self, skip_confirm: bool) -> anyhow::Result<()> {
        let title = self.title.to_string(&self.variables, &self.defaults)?;
        let desc = self.description.to_string(&self.variables, &self.defaults)?;
        let dynamic = self.dynamic_text.to_string(&self.variables, &self.defaults)?;

        let forward_source = if let Some(forward_source) = &self.forward_source {
            forward_source.to_string(&self.variables, &self.defaults)?
        } else {
            String::new()
        };

        let mut tags = Vec::new();
        for tag in self.tags.iter() {
            let result = tag.to_string(&self.variables, &self.defaults)?;
            if !result.is_empty() {
                tags.push(result);
            }
        }

        self.display_timestamp()?;

        for video in self.video_prefix.iter() {
            video.to_string(&self.variables, &self.defaults)?;
        }
        for video in self.video_suffix.iter() {
            video.to_string(&self.variables, &self.defaults)?;
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
            source.to_string(&self.variables, &self.defaults).unwrap()
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
                        (Some(date), Some(time)) => Some(date.and_time(time).timestamp() - 60 * 60 * 8),
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
            title: self.title.to_string(&self.variables, &self.defaults)?,
            desc_format_id: 0,
            desc: self.description.to_string(&self.variables, &self.defaults)?,
            dynamic: self.dynamic_text.to_string(&self.variables, &self.defaults)?,
            subtitle: Subtitle {
                open: 0,
                lan: "".to_string(),
            },
            tag: self
                .tags
                .iter()
                .map(|s| s.to_string(&self.variables, &self.defaults))
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
            .map(|s| s.to_string(&self.variables, &self.defaults))
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
            .map(|s| s.to_string(&self.variables, &self.defaults))
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
    fn to_string(&self, variable_description_map: &HashMap<String, String>, variable_default_value_map: &HashMap<String, TemplateString>) -> anyhow::Result<String> {
        let regex = regex::Regex::new(r"\{\{(.*?)\}\}").unwrap();
        let matches = regex
            .captures_iter(&self.0)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>();
        let mut result = self.0.clone();
        if !matches.is_empty() {
            for variable in matches.iter() {
                let var = dotenv::var(&variable).or_else(|_| -> anyhow::Result<_> {
                    if variable.starts_with("ss_") {
                        anyhow::bail!("未定义的预设变量：{}", variable)
                    };

                    let description = match variable_description_map.get(variable) {
                        Some(description) => format!("{description}({variable})"),
                        None => format!("{variable}"),
                    };

                    // 用户输入变量
                    let mut question = requestty::Question::input(variable)
                        .message(description);
                    if let Some(default) = variable_default_value_map.get(variable) {
                        question = question.default(default.to_string(&variable_description_map, &variable_default_value_map)?);
                    }
                    let question = question.build();
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
