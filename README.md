# JSer 入侵 Rust
# 第 1 集 创建项目

本集不提供现成代码，旨在让玩家们体验创建项目的流程。第 1 集之后全为现成代码

1. 用 cargo 命令行 初始化项目
   ```shell
   # 创建 demo 项目，不初始化 git (我们已经有了)
   cargo init --lib --name demo --vcs none
   ```

2. 新建 `src/main.rs` 文件，写入以下代码
   ```rust
   fn main() {
   	println!("JSer 入侵 Rust");
   }
   ```

3. 命令行执行 `cargo run`  即可看到输出结果
   ```rust
   // 如果你的 vscode 开启了 Code Lens (在vscode配置项搜索 "Code Lens" 即可打开这个 run 按钮，前提是按 main 分支 README 正确安装了插件)
   // 👇 你会看到这个 run 按钮点击也可以执行代码
   run | debug
   fn main() {
   	println!("JSer 入侵 Rust");
   }
   ```

