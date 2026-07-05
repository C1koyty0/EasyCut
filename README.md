# EasyCut

Windows 上可运行的轻量视频剪辑工具，基于 Rust 开发。

## 项目简介

EasyCut 旨在提供简单易用的视频剪辑体验，适合日常快速裁剪、拼接等基础操作，无需依赖复杂的专业软件。

## 技术栈

- **Rust** — 核心语言，兼顾性能与内存安全

## 目标平台

- Windows

## 支持格式

| 格式 | 状态 |
|------|------|
| MP4  | 支持 |

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

## 许可证

本项目采用 [MIT License](LICENSE) 开源。

Copyright (c) 2026 EasyCut contributors
