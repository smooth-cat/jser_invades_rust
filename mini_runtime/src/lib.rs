//! 一个用于学习的极简异步运行时。
//!
//! 它只实现 Tokio 最核心的执行循环：把 future 固定在堆上，poll 它；遇到
//! `Pending` 就让当前线程休眠；收到 waker 后再唤醒线程继续 poll。真实 Tokio
//! 还会加入 task 队列、IO 反应器、定时器和多线程调度。

pub mod future;
pub mod runner;
pub mod runner_downcast;
// 过程宏展开使用绝对路径 `::mini_runtime::MiniRuntime`。库自身使用宏时也需要
// 把当前 crate 以这个名字放入 extern prelude。
extern crate self as mini_runtime;
pub use mini_runtime_macros::main;
