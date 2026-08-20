//! 05 - `select!`：等待多个 future，哪个先 ready 就处理哪个。
//!
//! 被选中的分支之外的 future 会被丢弃，因此循环中的 future 要考虑“取消安全”。
//! `select!` 默认会随机化分支检查顺序；需要优先级时可以写 `biased;`。

use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("05. select!");
  let (sender, receiver) = tokio::sync::oneshot::channel::<&'static str>();
  tokio::spawn(async move {
    tokio::time::sleep(Duration::from_millis(1)).await;
    let _ = sender.send("消息先到");
  });

  let mut timeout = Box::pin(tokio::time::sleep(Duration::from_millis(50)));
  let mut message = Box::pin(receiver);
  let winner = tokio::select! {
    value = &mut message => value.expect("sender 不应提前丢弃"),
    _ = &mut timeout => "超时先到",
  };

  println!("select! 获胜分支: {winner}");
  assert_eq!(winner, "消息先到");

  // `biased` 将轮询顺序固定为从上到下，适合明确的优先级策略。
  let mut ready = std::future::ready(7_u8);
  let biased_value = tokio::select! {
    biased;
    value = &mut ready => value,
    _ = std::future::pending::<()>() => 0,
  };
  assert_eq!(biased_value, 7);
}
