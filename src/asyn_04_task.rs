//! 04 - task：把 future 交给运行时调度，获得独立的 `JoinHandle`。
//!
//! task 之间共享线程池，但每个 task 有自己的 future 状态。`JoinHandle` 既可以
//! 等待结果，也可以 `abort` 请求取消；丢弃 handle 则表示不再等待结果。

use std::future::pending;

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("04. task");
  let first = tokio::spawn(async {
    tokio::task::yield_now().await;
    20_u32
  });
  let second = tokio::spawn(async {
    tokio::task::yield_now().await;
    22_u32
  });

  let (first, second) = tokio::join!(first, second);
  let sum = first.expect("first task panic") + second.expect("second task panic");
  println!("两个 task 的结果: {sum}");
  assert_eq!(sum, 42);

  let cancelled = tokio::spawn(async {
    pending::<()>().await;
  });
  cancelled.abort();
  let error = cancelled.await.expect_err("task 应该被取消");
  println!(
    "abort 后 JoinError::is_cancelled() = {}",
    error.is_cancelled()
  );
  assert!(error.is_cancelled());
}
