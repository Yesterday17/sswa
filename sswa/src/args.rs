use crate::config::Config;
use crate::context::CONTEXT;
use crate::ffmpeg;
use crate::template::VideoTemplate;
use anyhow::{bail, Context};
use clap::Parser;
use clap_handler::{handler, Context as ClapContext, Handler};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::Rng;
use serde_json::Value;
use ssup::constants::set_useragent;
use ssup::video::{VideoCardItem, VideoPart};
use ssup::{Client, CookieEntry, CookieInfo, Credential, VideoId};
use std::collections::HashMap;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::fs;

#[derive(Parser, Clone)]
pub(crate) struct Args {
    /// 配置文件所在的目录，留空时默认通过 directories-next 获取
    #[clap(short, long)]
    config_root: Option<PathBuf>,

    /// 手动指定投稿时的 User-Agent
    #[clap(long = "ua")]
    user_agent: Option<String>,

    /// 投稿帐号
    #[clap(short = 'u', long = "user", global = true)]
    account: Option<String>,

    /// 执行的子命令
    #[clap(subcommand)]
    command: SsCommand,
}

#[clap_handler::async_trait]
impl Handler for Args {
    async fn handle_command(&mut self, ctx: &mut ClapContext) -> anyhow::Result<()> {
        // 初始化配置文件目录
        let config_root = self
            .config_root
            .as_deref()
            .and_then(|path| {
                if path.is_absolute() {
                    Some(path.to_path_buf())
                } else {
                    path.canonicalize().ok()
                }
            })
            .unwrap_or_else(|| {
                directories_next::ProjectDirs::from("moe.mmf", "Yesterday17", "sswa")
                    .unwrap()
                    .config_dir()
                    .to_path_buf()
            });
        // 创建配置文件目录
        let _ = fs::create_dir_all(&config_root).await?;
        let _ = fs::create_dir(config_root.join("templates")).await;
        let _ = fs::create_dir(config_root.join("accounts")).await;

        // 初始化读取配置文件
        let config: Config = match fs::read_to_string(config_root.join("config.toml")).await {
            Ok(config) => toml::from_str(&config)?,
            Err(_) => Config::new(),
        };

        // 设置 User-Agent
        if let Some(ref user_agent) = self.user_agent {
            set_useragent(user_agent.to_string());
        }

        ctx.insert(config_root);
        ctx.insert(config);
        Ok(())
    }

    async fn handle_subcommand(&mut self, ctx: ClapContext) -> anyhow::Result<()> {
        self.command.execute(ctx).await
    }
}

#[derive(Parser, Handler, Clone)]
pub(crate) enum SsCommand {
    /// 输出配置文件所在路径
    Config(SsConfigCommand),
    /// 上传视频
    Upload(SsUploadCommand),
    /// 增加分P
    Append(SsAppendCommand),
    /// 查看已投稿视频
    View(SsViewCommand),
    /// 修改视频分段章节
    Card(SsCardCommand),
    /// 帐号登录
    Login(SsAccountLoginCommand),
    /// 帐号登出
    Logout(SsAccountLogoutCommand),
    /// 列出已登录帐号
    Accounts(SsAccountListCommand),
}

#[derive(Parser, Clone)]
pub(crate) struct SsConfigCommand;

#[handler(SsConfigCommand)]
async fn handle_config(config_root: &PathBuf) -> anyhow::Result<()> {
    print!("{}", config_root.display());
    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsUploadCommand {
    /// 投稿使用的模板
    #[clap(short, long)]
    template: String,

    /// 投稿模板对应的变量
    #[clap(short, long = "var")]
    variables: Vec<String>,

    /// 变量文件
    #[clap(short = 'f', long = "variable-file")]
    variable_file: Option<PathBuf>,

    /// 检查的等级
    ///
    /// 出现1次：跳过投稿信息确认
    /// 出现2次：跳过变量输入，当存在必填变量时会产生错误
    /// 出现3次：跳过所有变量输入且不产生错误
    #[clap(short = 'y', parse(from_occurrences))]
    skip_level: u8,

    /// 是否自动缩放封面到 960*600
    #[clap(long)]
    scale_cover: Option<bool>,

    /// 是否保留简单变量文件中值前后的引号（包括单引号和双引号）
    #[clap(short = 'q', long = "quotes")]
    keep_quote_pairs: bool,

    /// 是否模拟投稿
    ///
    /// 模拟投稿时，不会实际向叔叔服务器上传视频和封面
    #[clap(short, long)]
    dry_run: bool,

    /// 视频分p标题
    /// 当为空时自动选取视频文件名作为标题
    /// 不包含前缀和后缀
    #[clap(short, long)]
    names: Vec<String>,

    /// 待投稿的视频
    #[clap(required = true)]
    videos: Vec<PathBuf>,
}

/// 尝试导入用户凭据，失败时则以该名称创建新的凭据
async fn credential(
    root: &PathBuf,
    account: Option<&str>,
    default_user: Option<&str>,
) -> anyhow::Result<Credential> {
    let account_file = root.join("accounts").join(format!(
        "{}.json",
        account.or(default_user).expect("account not specified")
    ));
    if account_file.exists() {
        // 凭据存在，读取并返回
        let account = fs::read_to_string(&account_file).await?;
        let mut account: Credential = serde_json::from_str(&account)?;

        // 自动更新凭据
        let refreshed = account.refresh(false).await?;
        if refreshed {
            fs::write(&account_file, serde_json::to_string(&account)?).await?;
        }

        if let Ok(nickname) = account.get_nickname().await {
            eprintln!("投稿用户：{nickname}");
            return Ok(account);
        } else {
            eprintln!("登录已失效！请重新登录。");
        }
    }

    // 凭据不存在，新登录
    let qrcode = Credential::get_qrcode().await?;
    eprintln!(
        "请打开以下链接登录：\n{}",
        qrcode["data"]["url"].as_str().unwrap()
    );
    let credential = Credential::from_qrcode(qrcode).await?;
    fs::write(account_file, serde_json::to_string(&credential)?).await?;
    Ok(credential)
}

async fn upload_videos(
    client: &Client,
    progress: &MultiProgress,
    videos: &[(PathBuf, &str)],
    dry_run: bool,
) -> anyhow::Result<Vec<VideoPart>> {
    let mut parts = Vec::with_capacity(videos.len());

    for (video, video_name) in videos {
        let metadata = tokio::fs::metadata(&video).await?;
        let total_size = metadata.len() as usize;

        let p_filename = progress.add(ProgressBar::new_spinner());
        p_filename.set_message(format!("{}", video.file_name().unwrap().to_string_lossy()));
        let pb = progress.add(ProgressBar::new(total_size as u64));
        let format = format!("{{spinner:.green}} [{{wide_bar:.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{bytes_per_sec}}, {{eta}})");
        pb.set_style(ProgressStyle::default_bar().template(&format)?);

        if dry_run {
            pb.inc(total_size as u64);
            pb.finish();
        } else {
            pb.set_position(0);

            let (sx, mut rx) = tokio::sync::mpsc::channel(1);
            let upload = client
                .upload_video_part(&video, total_size, sx, None /* TODO: Add part name */);
            tokio::pin!(upload);
            let result = loop {
                tokio::select! {
                    Some(size) = rx.recv() => {
                        // 上传进度
                        pb.inc(size as u64);
                    }
                    video = &mut upload => {
                        // 上传完成
                        p_filename.finish();
                        pb.finish();
                        break video;
                    }
                }
            };
            if let Ok(part) = result {
                parts.push(part);
            } else {
                // once again
                pb.set_position(0);

                let (sx, mut rx) = tokio::sync::mpsc::channel(1);
                let upload = client.upload_video_part(
                    &video,
                    total_size,
                    sx,
                    if video_name.is_empty() {
                        None
                    } else {
                        Some(video_name.to_string())
                    },
                );
                tokio::pin!(upload);
                loop {
                    tokio::select! {
                        Some(size) = rx.recv() => {
                            // 上传进度
                            pb.inc(size as u64);
                        }
                        video = &mut upload => {
                            // 上传完成
                            p_filename.finish();
                            pb.finish();
                            parts.push(video?);
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(parts)
}

impl SsUploadCommand {
    /// 尝试导入视频模板
    async fn template(&self, root: &PathBuf) -> anyhow::Result<VideoTemplate> {
        fn set_variable<I>(key: &str, value: I)
        where
            I: Into<Value>,
        {
            if key.starts_with('$') || key.starts_with("ss_") {
                eprintln!("跳过变量：{key}");
            } else {
                CONTEXT.insert(key.to_string(), value);
            }
        }

        let template = root
            .join("templates")
            .join(format!("{}.toml", self.template));
        if !template.exists() {
            bail!("Template not found!");
        }

        let template = fs::read_to_string(template).await?;
        let template: VideoTemplate = toml::from_str(&template)?;
        for (variable, detail) in template.variables.iter() {
            if detail.can_skip && detail.default.is_none() {
                set_variable(variable, String::new());
            }
        }

        // 低优先级：变量文件
        if let Some(variables) = &self.variable_file {
            let file = fs::read_to_string(variables).await?;
            if file.starts_with('{') {
                // parse as json file
                let json: HashMap<String, Value> = serde_json::from_str(&file)?;
                for (key, value) in json {
                    set_variable(&key, value);
                }
            } else {
                for mut line in file.split('\n') {
                    line = line.trim();
                    if !line.is_empty() && !line.starts_with('#') {
                        let (key, value) = line.split_once('=').unwrap_or((&line, ""));
                        let key = key.trim();
                        let mut value = value.trim_matches(' ');
                        if !self.keep_quote_pairs
                            && ((value.starts_with('"') && value.ends_with('"'))
                                || (value.starts_with('\'') && value.ends_with('\'')))
                        {
                            value = &value[1..value.len() - 1];
                        }
                        let value = value.replace("\\n", "\n");
                        set_variable(key, value);
                    }
                }
            }
        }
        // 高优先级：命令行变量
        for variable in self.variables.iter() {
            let (key, value) = variable.split_once('=').unwrap_or((&variable, ""));
            set_variable(key.trim(), value.trim());
        }

        Ok(template)
    }
}

#[handler(SsUploadCommand)]
async fn handle_upload(
    this: &SsUploadCommand,
    config_root: &PathBuf,
    config: &Config,
    args: &Args,
) -> anyhow::Result<()> {
    let progress = indicatif::MultiProgress::new();

    // 加载模板
    let template = this.template(&config_root).await?;

    // 预定义变量
    CONTEXT.insert_sys("config_root".to_string(), config_root.to_string_lossy());
    CONTEXT.insert_sys(
        "file_name".to_string(),
        this.videos[0].file_name().unwrap().to_string_lossy(),
    );
    CONTEXT.insert_sys(
        "file_stem".to_string(),
        this.videos[0].file_stem().unwrap().to_string_lossy(),
    );
    CONTEXT.insert_sys(
        "file_pwd".to_string(),
        this.videos[0]
            .canonicalize()?
            .parent()
            .unwrap()
            .to_string_lossy(),
    );

    // 模板字符串编译
    let tmpl = template
        .build(this.skip_level)
        .with_context(|| "build template")?;

    // 模板变量检查
    template
        .validate(&tmpl, this.skip_level)
        .with_context(|| "validate template")?;

    // 用户登录检查
    let credential = credential(
        config_root,
        args.account.as_deref(),
        template
            .default_user
            .as_deref()
            .or(config.default_user.as_deref()),
    )
    .await?;

    // 线路选择
    let client = {
        let line = config.line().await?;
        progress.println(format!("已选择线路：{}", line.probe_url()))?;
        Client::new(line, credential)
    };

    // 上传封面
    let cover = {
        let cover = if template.auto_cover() {
            let duration =
                ffmpeg::get_duration(&this.videos[0]).with_context(|| "ffmpeg::get_duration")?;
            let rnd = rand::thread_rng().gen_range(0..duration);
            Some(ffmpeg::auto_cover(&this.videos[0], rnd)?)
        } else if this.scale_cover.unwrap_or(config.need_scale_cover()) {
            Some(
                ffmpeg::scale_cover(&template.cover(&tmpl)?)
                    .with_context(|| "ffmpeg::scale_cover")?,
            )
        } else {
            None
        };
        let cover_path = match &cover {
            Some(cover) => cover.to_path_buf(),
            None => template.cover(&tmpl)?.into(),
        };

        let cover = if !this.dry_run {
            client
                .upload_cover(cover_path)
                .await
                .with_context(|| "upload cover")?
        } else {
            "".into()
        };
        progress.println("封面已上传！")?;
        cover
    };

    // 准备分P
    let video_files: Vec<(PathBuf, &str)> = template
        .video_prefix(&tmpl)
        .into_iter()
        .map(|v| (v, ""))
        .chain(
            this.videos
                .clone()
                .into_iter()
                .enumerate()
                .map(|(i, v)| (v, this.names.get(i).map(|r| r.as_str()).unwrap_or(""))),
        )
        .chain(template.video_suffix(&tmpl).into_iter().map(|v| (v, "")))
        .collect();
    // 检查文件存在
    for (video, _) in video_files.iter() {
        if !video.exists() {
            bail!("Video not found: {}", video.display());
        }
    }

    // 上传分P
    let parts = upload_videos(&client, &progress, &video_files, this.dry_run).await?;

    // 提交视频
    let video = template.to_video(&tmpl, parts, cover)?;
    if !this.dry_run {
        let mut retry = config.submit_retry();
        loop {
            match client.submit(&video).await {
                Ok(_) => {
                    eprintln!("投稿成功！");
                    break;
                }
                Err(err) => {
                    if retry == 0 {
                        bail!("投稿失败：{}", err);
                    } else {
                        println!("投稿失败：{}", err);
                        retry -= 1;
                        println!("正在重试，剩余 {} 次", retry);
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsAppendCommand {
    /// 待增加分P的视频 ID
    #[clap(short = 'v', long)]
    video_id: VideoId,

    /// 视频分p标题
    /// 当为空时自动选取视频文件名作为标题
    /// 不包含前缀和后缀
    #[clap(short, long)]
    names: Vec<String>,

    /// 添加的视频
    #[clap(required = true)]
    videos: Vec<PathBuf>,
}

#[handler(SsAppendCommand)]
async fn handle_append(
    this: &SsAppendCommand,
    config_root: &PathBuf,
    config: &Config,
    args: &Args,
) -> anyhow::Result<()> {
    // 1. 获取待修改视频
    let credential = credential(
        config_root,
        args.account.as_deref(),
        config.default_user.as_deref(),
    )
    .await?;
    let line = config.line().await?;
    let client = Client::new(line, credential);
    let mut video = client.get_video(&this.video_id).await?;

    // 2. 检查文件存在
    for video in this.videos.iter() {
        if !video.exists() {
            bail!("Video not found: {}", video.display());
        }
    }

    // 3. 准备进度条
    let progress = indicatif::MultiProgress::new();

    // 4. 准备文件名
    let videos: Vec<_> = this
        .videos
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                v.clone(),
                this.names.get(i).map(|s| s.as_str()).unwrap_or(""),
            )
        })
        .collect();

    // 5. 上传分P
    let mut parts = upload_videos(&client, &progress, &videos, false)
        .await?
        .into_iter()
        .map(|p| p.into())
        .collect();
    video.videos.append(&mut parts);

    // 6. 提交视频
    eprintln!("准备投稿…");
    let mut retry = config.submit_retry();
    loop {
        match client.submit_edit(&video).await {
            Ok(_) => {
                eprintln!("投稿成功！");
                break;
            }
            Err(err) => {
                if retry == 0 {
                    bail!("投稿失败：{}", err);
                } else {
                    println!("投稿失败：{}", err);
                    retry -= 1;
                    println!("正在重试，剩余 {} 次", retry);
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
            }
        }
    }

    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsViewCommand {
    /// 使用的帐号
    #[clap(short = 'u', long = "user")]
    account: Option<String>,

    /// 查看的视频 ID
    video_id: VideoId,
}

#[handler(SsViewCommand)]
async fn handle_view(
    this: &SsViewCommand,
    config_root: &PathBuf,
    config: &Config,
) -> anyhow::Result<()> {
    let credential = credential(
        config_root,
        this.account.as_deref(),
        config.default_user.as_deref(),
    )
    .await?;
    let client = Client::auto(credential).await?;
    let video = client.get_video(&this.video_id).await?;
    println!("{:#?}", video);
    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsCardCommand {
    /// 使用的帐号
    #[clap(short = 'u', long = "user")]
    account: Option<String>,

    /// 视频 ID
    #[clap(short, long)]
    video_id: VideoId,

    /// 视频分P编号
    #[clap(short, long = "part")]
    part_id: Option<usize>,

    /// 是否强制进度条显示
    /// 在客户端上会遮挡字幕，如有需要请手动开启
    #[clap(long)]
    permanent: bool,

    /// 分段文件路径
    ///
    /// 分段文件的格式如下：
    /// [start],[description]
    card_file: PathBuf,
}

#[handler(SsCardCommand)]
async fn handle_card(
    this: &SsCardCommand,
    config_root: &PathBuf,
    config: &Config,
) -> anyhow::Result<()> {
    fn parse_time_point(input: &str) -> Result<u64, ParseIntError> {
        if input.contains(':') {
            let mut result = 0;
            for part in input.split(':') {
                result = result * 60 + part.parse::<u64>()?;
            }
            Ok(result)
        } else {
            input.parse()
        }
    }

    // parse file first
    let data = fs::read_to_string(&this.card_file).await?;
    let time_points: Vec<(u64, &str)> = data
        .split('\n')
        .into_iter()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                None
            } else {
                Some(line)
            }
        })
        .map::<anyhow::Result<_>, _>(|line| {
            let (start, content) = line
                .split_once(',')
                .ok_or_else(|| anyhow::anyhow!("invalid line"))?;
            let start = parse_time_point(start)?;
            Ok((start, content))
        })
        .collect::<Result<_, _>>()
        .with_context(|| "parse card file")?;

    // get video info
    let credential = credential(
        config_root,
        this.account.as_deref(),
        config.default_user.as_deref(),
    )
    .await?;
    let client = Client::auto(credential).await?;
    let video = client.get_video(&this.video_id).await?;

    let part_index = match this.part_id {
        None => 0,
        Some(0) => 0,
        Some(i) => i - 1,
    };
    if video.videos.len() <= part_index {
        bail!("分P不存在！");
    }
    let part = &video.videos[part_index];
    let cid = part.cid.expect("cid not found");

    // prepare cards
    let mut cards = Vec::with_capacity(time_points.len());
    let mut prev_end = part.duration;
    for i in (0..time_points.len()).rev() {
        let (start, content) = &time_points[i];
        cards.push(VideoCardItem {
            from: *start,
            to: prev_end,
            content: content.to_string(),
        });
        prev_end = *start;
    }
    cards.reverse();
    cards[0].from = 0;
    // eprintln!("{:#?}", cards);

    client
        .edit_card(video.aid, cid, cards, this.permanent)
        .await?;

    eprintln!("分段章节修改成功！");
    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsAccountListCommand;

#[handler(SsAccountListCommand)]
async fn account_list(config_root: &PathBuf) -> anyhow::Result<()> {
    let accounts = config_root.join("accounts");
    let mut dir = fs::read_dir(accounts).await?;
    while let Some(next) = dir.next_entry().await? {
        if let Some("json") = next.path().extension().map(|s| s.to_str().unwrap()) {
            println!("{}", next.path().file_stem().unwrap().to_string_lossy());
        }
    }

    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsAccountLoginCommand {
    /// 可选的 cookie，用于自动登录
    #[clap(short, long = "cookie")]
    cookies: Vec<String>,
    /// 帐号名称，在后续投稿时需要作为参数传递进来
    name: String,
}

#[handler(SsAccountLoginCommand)]
async fn account_login(this: &SsAccountLoginCommand, config_root: &PathBuf) -> anyhow::Result<()> {
    let account_path = config_root
        .join("accounts")
        .join(format!("{}.json", this.name));
    if account_path.exists() {
        bail!("帐号 {} 已存在！", this.name);
    }

    let credential = if this.cookies.is_empty() {
        let qrcode = Credential::get_qrcode().await?;
        eprintln!(
            "请打开以下链接登录：\n{}",
            qrcode["data"]["url"].as_str().unwrap()
        );
        Credential::from_qrcode(qrcode).await?
    } else {
        let cookies: Vec<_> = this
            .cookies
            .iter()
            .filter_map(|c| CookieEntry::from_str(c).ok())
            .collect();
        Credential::from_cookies(&CookieInfo::new(cookies)).await?
    };

    fs::write(account_path, serde_json::to_string(&credential)?).await?;
    let nickname = credential.get_nickname().await?;
    eprintln!("帐号 {} 已登录！帐号名为：{nickname}", this.name);
    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsAccountLogoutCommand {
    /// 待删除登录凭据的帐号名称
    name: String,
}

#[handler(SsAccountLogoutCommand)]
async fn account_logout(
    this: &SsAccountLogoutCommand,
    config_root: &PathBuf,
) -> anyhow::Result<()> {
    let account_path = config_root
        .join("accounts")
        .join(format!("{}.json", this.name));
    if !account_path.exists() {
        bail!("帐号 {} 不存在！", this.name);
    }

    fs::remove_file(account_path).await?;
    eprintln!("帐号 {} 已删除！", this.name);
    Ok(())
}
