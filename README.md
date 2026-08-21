# JSer 入侵 Rust
## 简介

1. 本项目采用对比手法，以 JS 为主逐个比较 两者的语法来达到学习 Rust 的目的
2. 本项目采用分 一个分支 对应 一个大知识点的方式进行学习
3. 可以按照分支知识点标号挨个进行学习

## Main README 为 Rust 环境安装教程

### 安装 Rust 编译器进入 https://rustup.rs/

**Mac OS 安装** 直接执行官方给出的命令行即可

**WIndows 安装**

1. 安装 vs_BuildTools.exe https://visualstudio.microsoft.com/zh-hans/downloads ctrl+f 搜 “生成工具”
   1. 工作负荷选择：使用 C++ 的桌面开发
   2. 安装位置可自选
2. 安装 rustup-init.exe https://rustup.rs/  是官方给出的

#### 验证安装

```shel
cargo -V
```

### 安装 vscode 插件，在扩展商店中分别搜索以下插件并安装

1. rust-analyzer 负责代码诊断
3. CodeLLDB 负责 Debug

至此环境安装完毕，可以切换到其他分支开始学习 rust 使用
