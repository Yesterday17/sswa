use date_time_parser::{DateParser, TimeParser};
use serde::Deserialize;
use ssup::video::{Subtitle, Video, VideoPart};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;
use tinytemplate::instruction::PathStep;
use tinytemplate::TinyTemplate;
use crate::context::CONTEXT;

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
    cover: TemplateString,
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
    pub variables: TemplateVariables,
}

impl VideoTemplate {
    /// 获取封面路径
    pub(crate) fn cover(&self, template: &TinyTemplate) -> anyhow::Result<String> {
        self.cover.to_string(template)
    }

    /// 构建模板
    pub(crate) fn build(&self) -> anyhow::Result<TinyTemplate> {
        let mut template = TinyTemplate::new();
        // 常用 Formatter
        template.add_formatter("comma2cn", |input, output| {
            if input.is_string() {
                let result = input.as_str().unwrap().replace(",", "、");
                output.push_str(&result);
            }
            Ok(())
        });

        template.add_unnamed_template(&self.title.0)?;
        template.add_unnamed_template(&self.description.0)?;
        template.add_unnamed_template(&self.dynamic_text.0)?;
        template.add_unnamed_template(&self.cover.0)?;

        if let Some(forward_source) = &self.forward_source {
            template.add_unnamed_template(&forward_source.0)?;
        }

        for tag in self.tags.iter() {
            template.add_unnamed_template(&tag.0)?;
        }

        for video in self.video_prefix.iter() {
            template.add_unnamed_template(&video.0)?;
        }

        for video in self.video_suffix.iter() {
            template.add_unnamed_template(&video.0)?;
        }

        self.variables.add_templates(&mut template)?;

        // 检查变量
        let paths = template.get_paths();
        for path in paths {
            if path.len() == 1 {
                // 暂时只检查一层变量
                match path[0] {
                    PathStep::Name(variable) => {
                        if !CONTEXT.contains_key(variable) {
                            let default = if let Some(default) = self.variables.default(variable) {
                                default.to_string(&template)?
                            } else {
                                String::new()
                            };

                            let ans = if self.variables.is_required(variable) {
                                // 用户输入变量
                                let description = match self.variables.description(variable) {
                                    Some(description) => format!("{description}({variable})"),
                                    None => format!("{variable}"),
                                };

                                let question = requestty::Question::input(variable)
                                    .default(default)
                                    .message(description);

                                let question = question.build();
                                let ans = requestty::prompt_one(question)?;
                                ans.as_string().unwrap().to_string()
                            } else {
                                default
                            };


                            CONTEXT.insert(variable.to_string(), ans);
                        }
                    }
                    PathStep::Index(_, _) => {}
                }
            }
        }
        Ok(template)
    }

    /// 校验模板字符串
    pub(crate) fn validate(&self, template: &TinyTemplate, skip_confirm: bool) -> anyhow::Result<()> {
        let title = self.title.to_string(template)?;
        let desc = self.description.to_string(template)?;
        let dynamic = self.dynamic_text.to_string(template)?;
        let cover = self.cover.to_string(template)?;

        let forward_source = if let Some(forward_source) = &self.forward_source {
            forward_source.to_string(template)?
        } else {
            String::new()
        };

        let tags = self.tags(template)?;

        self.display_timestamp()?;

        for video in self.video_prefix.iter() {
            video.to_string(template)?;
        }
        for video in self.video_suffix.iter() {
            video.to_string(template)?;
        }

        // 输出投稿信息
        eprintln!("标题：{title}\n来源：{forward_source}\n简介：\n---简介开始---\n{desc}\n---简介结束---\n标签：{tags}\n动态：{dynamic}\n封面文件路径：{cover}",
                  dynamic = if dynamic.is_empty() { "（空）" } else { &dynamic },
        );
        if !skip_confirm {
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

    fn forward_source(&self, template: &TinyTemplate) -> String {
        if let Some(source) = &self.forward_source {
            source.to_string(&template).unwrap()
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

    fn tags(&self, template: &TinyTemplate) -> anyhow::Result<String> {
        let mut tags = Vec::new();
        for tag in self.tags.iter() {
            let result = tag.to_string(template)?;
            let result = result.trim();
            let results = result.split(',').filter_map(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            }).collect::<Vec<String>>();
            tags.extend(results);
        }
        Ok(tags.join(","))
    }

    pub(crate) fn to_video(
        &self,
        template: &TinyTemplate<'_>,
        parts: Vec<VideoPart>,
        cover: String,
    ) -> anyhow::Result<Video> {
        Ok(Video {
            copyright: match &self.forward_source {
                Some(source) if !source.is_empty() => 2,
                _ => 1,
            },
            source: self.forward_source(template),
            tid: self.tid,
            cover,
            title: self.title.to_string(template)?,
            desc_format_id: 0,
            desc: self.description.to_string(template)?,
            dynamic: self.dynamic_text.to_string(template)?,
            subtitle: Subtitle {
                open: 0,
                lan: "".to_string(),
            },
            tag: self.tags(template)?,
            videos: parts,
            display_time: self.display_timestamp()?,
            open_subtitle: false,
        })
    }

    pub(crate) fn auto_cover(&self) -> bool {
        self.cover.is_empty() || self.cover.0 == "auto"
    }

    pub(crate) fn video_prefix(&self, template: &TinyTemplate) -> Vec<PathBuf> {
        self.video_prefix
            .iter()
            .map(|s| s.to_string(&template))
            .filter_map(|s| match s {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            })
            .map(|s| PathBuf::from(s))
            .collect()
    }

    pub(crate) fn video_suffix(&self, template: &TinyTemplate) -> Vec<PathBuf> {
        self.video_suffix
            .iter()
            .map(|s| s.to_string(&template))
            .filter_map(|s| match s {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            })
            .map(|s| PathBuf::from(s))
            .collect()
    }
}

#[derive(Deserialize)]
pub struct TemplateString(String);

impl TemplateString {
    fn to_string(&self, template: &TinyTemplate) -> anyhow::Result<String> {
        Ok(template.render(&self.0, &*CONTEXT.0.read())?)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Deserialize, Default)]
pub struct TemplateVariables(HashMap<String, TemplateVariable>);

impl TemplateVariables {
    fn add_templates(&self, template: &mut TinyTemplate) -> anyhow::Result<()> {
        for variable in self.0.values() {
            match variable {
                TemplateVariable::Simple(_) => {}
                TemplateVariable::Detailed(detailed) => {
                    if let Some(default) = &detailed.default {
                        // leak template string here for convenience
                        template.add_unnamed_template(Box::leak(default.0.clone().into_boxed_str()))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item=(&str, &DetailedVariable)> {
        self.0.iter().filter_map(|(name, v)| match v {
            TemplateVariable::Simple(_) => None,
            TemplateVariable::Detailed(detailed) => Some((name.as_str(), detailed)),
        })
    }

    fn description(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.description())
    }

    fn default(&self, key: &str) -> Option<&TemplateString> {
        self.0.get(key).and_then(|v| v.default())
    }

    fn is_required(&self, key: &str) -> bool {
        self.0.get(key).map_or(true, |v| v.is_required())
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum TemplateVariable {
    /// 简单变量，仅包括变量简介
    /// 该变量是必选的
    Simple(String),
    /// 复杂变量，可以设置各种属性
    Detailed(DetailedVariable),
}

impl TemplateVariable {
    fn description(&self) -> Option<&str> {
        match &self {
            TemplateVariable::Simple(description) => Some(description.as_str()),
            TemplateVariable::Detailed(detailed) => detailed.description.as_deref(),
        }
    }

    pub fn default(&self) -> Option<&TemplateString> {
        match &self {
            TemplateVariable::Simple(_) => None,
            TemplateVariable::Detailed(detailed) => detailed.default.as_ref(),
        }
    }

    pub fn is_required(&self) -> bool {
        match &self {
            TemplateVariable::Simple(_) => true,
            TemplateVariable::Detailed(detailed) => !detailed.can_skip,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DetailedVariable {
    description: Option<String>,
    pub default: Option<TemplateString>,
    #[serde(default)]
    pub can_skip: bool,
}
