//! 02 - `async/await`：把状态机写成接近同步代码的形式。
//!
//! `async fn` 调用时不会立刻执行函数体，只会构造一个 future；`.await` 才会让
//! 当前任务在这个 future 完成前挂起。多个独立 future 可以用 `join!` 并发推进。
use std::time::Duration;

async fn fetch(label: &'static str, delay_ms: u64) -> String {
  tokio::time::sleep(Duration::from_millis(delay_ms)).await;
  format!("{label} 完成")
}

async fn sequential() -> (String, String) {
  // 第二个操作要等第一个 await 完成后才开始。
  let first = fetch("顺序任务 A", 2).await;
  let second = fetch("顺序任务 B", 2).await;
  (first, second)
}

async fn concurrent() -> (String, String) {
  // `join!` 在同一个 task 内交替 poll 两个 future，不会额外创建 task。
  tokio::join!(fetch("并发任务 A", 4), fetch("并发任务 B", 1))
}

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("02. async/await");
  let sequential_result = sequential().await;
  let concurrent_result = concurrent().await;
  println!("sequential: {sequential_result:?}");
  println!("join!:     {concurrent_result:?}");
  assert_eq!(concurrent_result.0, "并发任务 A 完成");
  assert_eq!(concurrent_result.1, "并发任务 B 完成");

  // async block 也能捕获环境，和 JavaScript 的 async 箭头函数很相似。
  let prefix = "async block";
  let block_result = async move { format!("{prefix}: {}", fetch("工作", 0).await) }.await;
  println!("{block_result}");
}
