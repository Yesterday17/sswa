# sswa

> 叔叔我啊……

本项目分为两个部分，可用作 `lib` 的 `ssup` 和可执行部分 `sswa`。

本项目很大程度上参考了 biliup-rs，但对投稿的方式进行了重新设计，以符合正常的投稿思维。

## 依赖

- ffmpeg

## 如何开始

1. 获取配置文件目录

```bash
sswa config
```

2. 进入 `配置目录/template`，按 [示例模板](examples/templates/mrrj.toml) 创建 `your_template_name.toml`
3. 使用命令：

```bash
# 以 your_name 用户投稿，模板为 your_template_name，视频文件为 video.mp4，分P名为 video
sswa upload --user your_name --template your_template_name video.mkv
```

4. 输入待输入的变量，上传。

## LICENSE

本项目遵循 [Apache 2.0](LICENSE) 协议，参考 biliup-rs 的部分遵循 MIT 协议。