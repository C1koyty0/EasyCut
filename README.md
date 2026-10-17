# EasyCut

Windows 上可运行的轻量视频剪辑工具，基于 Rust 开发。

## 项目简介

EasyCut 旨在提供简单易用的视频剪辑体验，适合日常快速裁剪、拼接等基础操作，无需依赖复杂的专业软件。

## 技术栈

- **Rust** — 核心语言，兼顾性能与内存安全

## 目标平台

- Windows 64 位（`x86_64-pc-windows-msvc`）
- macOS Apple Silicon（`aarch64-apple-darwin`）

## 支持格式

| 格式 | 状态 |
|------|------|
| MP4  | 支持 |

## 预览

打开视频后可播放、暂停、拖动定位并调节音量。带音轨的视频以音频输出进度作为画面时钟；视频缓冲不足时会暂停音频，待画面追上后继续播放。无音轨或音频设备不可用时仍可进行无声视频预览。`Set in` 和 `Set out` 可以标记片段范围，点击 `Export MP4` 后在后台调用本机 `ffmpeg` 生成 H.264/AAC MP4。保存对话框默认文件名为 `源文件名-片段时长(s).mp4`，导出期间显示编码进度。需要先安装带 `libx264` 和 AAC 编码器的 FFmpeg。

从 workspace 根目录运行桌面应用：

```bash
cargo run -p easycut-core
```

## 环境要求

- [Rust](https://www.rust-lang.org/tools/install)（推荐 stable 工具链）
- Windows 10 及以上

## 快速开始

```bash
# 克隆仓库
git clone <repository-url>
cd EasyCut

# 编译
cargo build --release

# 运行
cargo run
```

## 开发

```bash
cargo check   # 快速语法检查
cargo build   # 调试构建
cargo test    # 运行测试
cargo run     # 运行开发版本
```

## GitHub Actions 发布

推送版本 tag 会通过 GitHub Actions 构建并创建 GitHub Release，产物包括：

- `EasyCut-macos-arm64.tar.gz`：Apple Silicon macOS（`aarch64-apple-darwin`）
- `EasyCut-windows-amd64.zip`：Windows 64 位（`x86_64-pc-windows-msvc`）

工作流使用 GitHub-hosted Runner：`macos-14` 构建 Apple Silicon，
`windows-latest` 构建 Windows 64 位。CI 不构建 Intel macOS，也不构建 32 位
Windows。发布构建只在 `v*` tag workflow 中执行，例如：

工作流会在构建时安装 Rust stable、FFmpeg 开发库和 `pkg-config`；Windows
通过 vcpkg 提供 FFmpeg，因为 `video-rs` 在编译期需要 FFmpeg 的 headers 和
libraries。

```bash
git tag v0.1.0
git push origin v0.1.0
```

导出功能仍需要运行环境自行安装带 `libx264` 和 AAC 编码器的 FFmpeg；FFmpeg
不会被静态打包进桌面压缩包。

## 许可证

本项目采用 [MIT License](LICENSE) 开源。

Copyright (c) 2026 EasyCut contributors
