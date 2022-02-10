use std::path::PathBuf;
use clap::Parser;
use anni_clap_handler::{Context as ClapContext, Handler, handler};
use anyhow::{bail, Context};
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use tokio::fs;
use ssup::{Client, Credential, UploadLine};
use ssup::constants::set_useragent;
use crate::config::Config;
use crate::ffmpeg;
use crate::template::VideoTemplate;

#[derive(Parser, Clone)]
pub(crate) struct Args {
    /// 配置文件所在的目录，留空时默认通过 directories-next 获取
    #[clap(short, long)]
    config_root: Option<PathBuf>,

    /// 手动指定投稿时的 User-Agent
    #[clap(long = "ua")]
    user_agent: Option<String>,

    /// 执行的子命令
    #[clap(subcommand)]
    command: SsCommand,
}

#[anni_clap_handler::async_trait]
impl Handler for Args {
    async fn handle_command(&mut self, ctx: &mut ClapContext) -> anyhow::Result<()> {
        // 初始化配置文件目录
        let config_root = self.config_root.as_deref().and_then(|path| {
            if path.is_absolute() {
                Some(path.to_path_buf())
            } else {
                path.canonicalize().ok()
            }
        }).unwrap_or_else(|| directories_next::ProjectDirs::from("moe.mmf", "Yesterday17", "sswa")
            .unwrap()
            .config_dir()
            .to_path_buf()
        );
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
    /// 上传视频相关功能
    Upload(SsUploadCommand),
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

    /// 投稿帐号
    #[clap(short = 'u', long = "user")]
    account: Option<String>,

    /// 是否跳过投稿前的检查
    #[clap(short = 'y')]
    skip_confirm: bool,

    /// 是否自动缩放封面到 960*600
    #[clap(long)]
    scale_cover: Option<bool>,

    /// 待投稿的视频
    videos: Vec<PathBuf>,
}

impl SsUploadCommand {
    /// 尝试导入用户凭据，失败时则以该名称创建新的凭据
    async fn credential(&self, root: &PathBuf, default_user: Option<&str>) -> anyhow::Result<Credential> {
        let account = root.join("accounts")
            .join(format!("{}.json", self.account.as_deref().or(default_user).expect("account not specified")));
        if account.exists() {
            // 凭据存在，读取并返回
            let account = fs::read_to_string(account).await?;
            let account = serde_json::from_str(&account)?;

            // TODO: 验证登录是否有效

            Ok(account)
        } else {
            // 凭据不存在，新登录
            let qrcode = Credential::get_qrcode().await?;
            eprintln!("请打开以下链接登录：\n{}", qrcode["data"]["url"].as_str().unwrap());
            let credential = Credential::from_qrcode(qrcode).await?;
            fs::write(account, serde_json::to_string(&credential)?).await?;
            Ok(credential)
        }
    }

    /// 尝试导入视频模板
    async fn template(&self, root: &PathBuf) -> anyhow::Result<VideoTemplate> {
        // 最低优先级：环境变量
        // 较高优先级：变量文件
        if let Some(variables) = &self.variable_file {
            dotenv::from_path(variables)?;
        }
        // 最高优先级：命令行变量
        for variable in self.variables.iter() {
            let (key, value) = variable.split_once('=').unwrap_or((&variable, ""));
            std::env::set_var(key, value);
        }

        let template = root.join("templates").join(format!("{}.toml", self.template));
        if !template.exists() {
            bail!("Template not found!");
        }

        let template = fs::read_to_string(template).await?;
        Ok(toml::from_str(&template)?)
    }
}

#[handler(SsUploadCommand)]
async fn handle_upload(this: &SsUploadCommand, config_root: &PathBuf, config: &Config) -> anyhow::Result<()> {
    let progress = indicatif::MultiProgress::new();

    // 加载模板
    let template = this.template(&config_root).await?;
    // 预定义变量，在 this.template 方法之后执行，覆盖所有可能冲突的环境变量
    std::env::set_var("ss_config_root", config_root.as_os_str());
    std::env::set_var("ss_file_name", this.videos[0].file_name().unwrap());
    std::env::set_var("ss_file_stem", this.videos[0].file_stem().unwrap());
    std::env::set_var("ss_file_pwd", this.videos[0].canonicalize()?.parent().unwrap().as_os_str());
    // 模板变量检查
    template.validate(this.skip_confirm).with_context(|| "validate template")?;

    // 用户登录检查
    let credential = this.credential(config_root, template.default_user.as_deref().or(config.default_user.as_deref())).await?;

    // 线路选择
    let client = {
        let p_line = progress.add(ProgressBar::new_spinner());
        p_line.set_message("选择线路…");
        let line = config.line.as_deref().unwrap_or("auto");
        let line = match line {
            "kodo" => UploadLine::kodo(),
            "bda2" => UploadLine::bda2(),
            "ws" => UploadLine::ws(),
            "qn" => UploadLine::qn(),
            "auto" => UploadLine::auto().await.with_context(|| "auto select upload line")?,
            _ => unimplemented!(),
        };
        p_line.finish_with_message("线路选择完成！");
        Client::new(line, credential)
    };

    // 上传封面
    let cover = {
        let cover = if template.auto_cover() {
            let duration = ffmpeg::get_duration(&this.videos[0]).with_context(|| "ffmpeg::get_duration")?;
            let rnd = rand::thread_rng().gen_range(0..duration);
            Some(ffmpeg::auto_cover(&this.videos[0], rnd)?)
        } else if this.scale_cover.unwrap_or(config.need_scale_cover()) {
            Some(ffmpeg::scale_cover(&template.cover).with_context(|| "ffmpeg::scale_cover")?)
        } else {
            None
        };
        let cover_path = match &cover {
            Some(cover) => cover.to_path_buf(),
            None => template.cover.to_string().into(),
        };

        let p_cover = progress.add(ProgressBar::new_spinner());
        p_cover.set_message("上传封面…");
        let cover = client.upload_cover(cover_path).await.with_context(|| "upload cover")?;
        p_cover.finish_with_message("封面上传成功！");
        cover
    };

    // 准备分P
    let mut parts = Vec::with_capacity(this.videos.len() + template.video_prefix_len() + template.video_suffix_len());
    let video_files: Vec<_> = template.video_prefix().into_iter()
        .chain(this.videos.clone().into_iter())
        .chain(template.video_suffix().into_iter()).collect();
    // 检查文件存在
    for video in video_files.iter() {
        if !video.exists() {
            bail!("Video not found: {}", video.display());
        }
    }

    // 上传分P
    for video in video_files {
        let (sx, mut rx) = tokio::sync::mpsc::channel(1);
        let metadata = tokio::fs::metadata(&video).await?;
        let total_size = metadata.len() as usize;

        let upload = client.upload_video_part(&video, total_size, sx);
        tokio::pin!(upload);

        let p_filename = progress.add(ProgressBar::new_spinner());
        p_filename.set_message(format!("{}", video.file_name().unwrap().to_string_lossy()));
        let pb = progress.add(ProgressBar::new(total_size as u64));
        let format = format!("{{spinner:.green}} [{{wide_bar:.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{bytes_per_sec}}, {{eta}})");
        pb.set_style(ProgressStyle::default_bar().template(&format));
        pb.enable_steady_tick(1000);

        loop {
            tokio::select! {
                Some(size) = rx.recv() => {
                    // 上传进度
                    pb.inc(size as u64);
                }
                video = &mut upload => {
                    // 上传完成
                    parts.push(video?);
                    p_filename.finish();
                    pb.finish();
                    break;
                }
            }
        }
    }

    // 提交视频
    let video = template.into_video(parts, cover).await?;
    client.submit(video).await?;
    eprintln!("投稿成功！");
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
    /// 帐号名称，在后续投稿时需要作为参数传递进来
    name: String,
}

#[handler(SsAccountLoginCommand)]
async fn account_login(this: &SsAccountLoginCommand, config_root: &PathBuf) -> anyhow::Result<()> {
    let account_path = config_root.join("accounts").join(format!("{}.toml", this.name));
    if account_path.exists() {
        bail!("帐号 {} 已存在！", this.name);
    }

    let qrcode = Credential::get_qrcode().await?;
    eprintln!("请打开以下链接登录：\n{}", qrcode["data"]["url"].as_str().unwrap());
    let credential = Credential::from_qrcode(qrcode).await?;
    fs::write(account_path, serde_json::to_string(&credential)?).await?;
    eprintln!("帐号 {} 已登录！", this.name);
    Ok(())
}

#[derive(Parser, Clone)]
pub(crate) struct SsAccountLogoutCommand {
    /// 待删除登录凭据的帐号名称
    name: String,
}

#[handler(SsAccountLogoutCommand)]
async fn account_logout(this: &SsAccountLogoutCommand, config_root: &PathBuf) -> anyhow::Result<()> {
    let account_path = config_root.join("accounts").join(format!("{}.toml", this.name));
    if !account_path.exists() {
        bail!("帐号 {} 不存在！", this.name);
    }

    fs::remove_file(account_path).await?;
    eprintln!("帐号 {} 已删除！", this.name);
    Ok(())
}
