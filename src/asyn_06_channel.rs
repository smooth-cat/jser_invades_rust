//! 06 - 异步 channel：在 task 之间传递值，而不是共享可变状态。
//!
//! `mpsc` 是多生产者单消费者；有界 channel 的 `send().await` 在缓冲区满时会
//! 让出 task。`oneshot` 适合一次性响应，`watch` 适合只关心最新状态的订阅者。

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("06. async channel");
  let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
  let producer = tokio::spawn(async move {
    for value in 1..=3 {
      sender.send(value).await.expect("receiver 仍然存在");
    }
  });

  let mut values = Vec::new();
  while let Some(value) = receiver.recv().await {
    values.push(value);
  }
  producer.await.expect("producer panic");
  println!("mpsc 收到: {values:?}");
  assert_eq!(values, [1, 2, 3]);

  let (request, response) = tokio::sync::oneshot::channel();
  tokio::spawn(async move {
    let _ = request.send("一次性响应");
  });
  assert_eq!(response.await.expect("响应已发送"), "一次性响应");

  let (state_tx, mut state_rx) = tokio::sync::watch::channel("starting");
  state_tx.send("ready").expect("watch receiver 仍然存在");
  state_rx.changed().await.expect("状态有更新");
  println!("watch 最新状态: {}", *state_rx.borrow());
  assert_eq!(*state_rx.borrow(), "ready");
}