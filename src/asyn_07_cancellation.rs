//! 07 - 取消：future 被 drop、task 被 abort，或由应用协议协作退出。
//!
//! Rust 没有强制终止任意代码的“取消异常”；取消通常发生在 `.await` 边界。需要
//! 清理资源时，把清理逻辑放在 guard 的 `Drop` 中，并让业务循环显式监听取消信号。

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

struct CleanupGuard(Arc<AtomicBool>);

impl Drop for CleanupGuard {
  fn drop(&mut self) {
    self.0.store(true, Ordering::SeqCst);
  }
}

async fn worker(mut cancel: tokio::sync::watch::Receiver<bool>, cleaned: Arc<AtomicBool>) {
  let _guard = CleanupGuard(cleaned);
  loop {
    tokio::select! {
      changed = cancel.changed() => {
        if changed.is_err() || *cancel.borrow() {
          break;
        }
      }
      _ = tokio::time::sleep(Duration::from_millis(1)) => {
        // 模拟一小段工作，然后回到 select! 检查取消信号。
      }
    }
  }
}

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("07. cancellation");
  let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
  let cleaned = Arc::new(AtomicBool::new(false));
  let handle = tokio::spawn(worker(cancel_rx, Arc::clone(&cleaned)));

  cancel_tx.send(true).expect("worker 仍然订阅取消信号");
  handle.await.expect("worker 不应 panic");
  println!(
    "协作式取消后，Drop 清理 = {}",
    cleaned.load(Ordering::SeqCst)
  );
  assert!(cleaned.load(Ordering::SeqCst));

  // abort 会让 Tokio 在下一个可取消点丢弃 task future，同样会运行 Drop。
  let aborted = tokio::spawn(async {
    std::future::pending::<()>().await;
  });
  aborted.abort();
  assert!(aborted.await.expect_err("任务已 abort").is_cancelled());
}
