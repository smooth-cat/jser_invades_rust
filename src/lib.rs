//! 第 13 集：异步编程学习示例。
//!
//! 每个 `asyn_*` 模块都围绕 README 中的一个知识点展开，并尽量把“发生了什么”
//! 写在代码旁边。运行 `cargo run` 可以按顺序看到所有 demo。


pub mod asyn_01_future_basics;
pub mod asyn_02_async_await;
pub mod asyn_03_runtime;
pub mod asyn_04_task;
pub mod asyn_05_select;
pub mod asyn_06_channel;
pub mod asyn_07_cancellation;
pub mod asyn_08_pin;
pub mod asyn_09_mini_runtime;
pub mod asyn_10_example_manual;
pub(crate) fn section(title: &str) {
  println!("\n{:-^72}", format!(" {title} "));
}
