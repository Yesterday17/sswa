use std::collections::HashMap;
use std::process::exit;
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
    /// 变量解释
    variables: HashMap<String, String>,
}

impl VideoTemplate {
    /// 校验模板字符串
    pub fn validate(&self, skip_confirm: bool) -> anyhow::Result<()> {
        let title = self.title.to_string(&self.variables)?;
        let desc = self.description.to_string(&self.variables)?;
        let dynamic = self.dynamic_text.to_string(&self.variables)?;

        let forward_source = if let Some(forward_source) = &self.forward_source {
            forward_source.to_string(&self.variables)?
        } else {
            String::new()
        };

        if !skip_confirm {
            eprintln!("标题：{title}\n来源：{forward_source}\n简介：\n---简介开始---\n{desc}\n---简介结束---\n动态：{dynamic}",
                      dynamic = if dynamic.is_empty() { "（空）" } else { &dynamic },
            );
            let question = requestty::Question::confirm("anonymous")
                .message("投稿信息如上，是否正确？")
                .build();
            let confirm = requestty::prompt_one(question).unwrap();
            if !confirm.as_bool().unwrap_or(false) {
                exit(0);
            }
        }
        Ok(())
    }

    pub async fn into_video(self, parts: Vec<VideoPart>, cover: String) -> anyhow::Result<Video> {
        Ok(Video {
            copyright: match &self.forward_source {
                Some(source) if !source.is_empty() => 2,
                _ => 1,
            },
            source: self.forward_source
                .and_then(|s| s.to_string(&self.variables).ok())
                .unwrap_or("".into()),
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
    fn to_string(&self, description: &HashMap<String, String>) -> anyhow::Result<String> {
        let regex = regex::Regex::new(r"\{\{(.*?)\}\}").unwrap();
        let matches = regex.captures_iter(&self.0)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>();
        let mut result = self.0.clone();
        if !matches.is_empty() {
            for variable in matches.iter() {
                let var = dotenv::var(&variable).or_else(|_| -> anyhow::Result<_>{
                    let description = match description.get(variable) {
                        Some(description) => format!("{description}({variable})"),
                        None => format!("{variable}"),
                    };

                    // 用户输入变量
                    let question = requestty::Question::input(variable).message(description).build();
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