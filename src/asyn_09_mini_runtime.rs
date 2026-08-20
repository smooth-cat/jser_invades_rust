//! 09 - 极简 运行时：用标准库的 `Future`、`Waker` 和线程 park/unpark 驱动一个 future，
//! 并配套实现 `#[mini_runtime::main]` / `#[mini_runtime::test]` 属性宏。
//!
//! 真实 Tokio 还包括 IO 反应器、时间轮、工作窃取线程池、任务队列等；这个版本只
//! 保留最核心的执行器循环，适合观察 `poll -> Pending -> wake -> poll` 的闭环。

pub use mini_runtime::runner::MyLoop;

/// 这个函数仍然写成 async，但自制属性宏会把它展开成普通同步函数。
#[mini_runtime::main]
async fn main_macro() {
  let answer = crate::asyn_01_future_basics::answer().await;
  println!("answer = {}", answer);
}

pub fn main_normal() {
  let mut runtime = MyLoop::new();
  runtime.add_async(
    Box::pin(async {
      // 这个 future 第一次 poll 时会 wake 当前线程，park 随即返回；第二次 poll 完成。
      crate::asyn_01_future_basics::YieldOnce::new().await;
      40 + 2
    }),
    |result| {
      println!("result = {}", result);
    },
  );
  runtime.run();
}

pub fn demo() {
  crate::section("09. 极简 运行时 + 属性宏");
  // main_normal();
  main_macro();
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn test_normal() {
    main_normal();
  }

  #[test]
  fn test_macro() {
    main_macro();
  }
}
