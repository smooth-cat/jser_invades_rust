//! 用 channel 传递消息：线程之间不直接共享数据，而是转移消息所有权。

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn demo() {
  single_producer();
  multiple_producers();
  bounded_channel();
}

fn single_producer() {
  println!("\n---- 1) 单生产者 channel ----");

  let (sender, receiver) = mpsc::channel::<String>();
  let handle = thread::spawn(move || {
    for message in ["一个", "接着", "一个"] {
      sender.send(message.to_string()).expect("receiver dropped");
    }
  });

  // drop sender 后，接收端的 for 循环会在消息读完时结束。
  handle.join().expect("producer thread panicked");
  let messages: Vec<_> = receiver.into_iter().collect();
  println!("收到消息 = {messages:?}");
}

fn multiple_producers() {
  println!("\n---- 2) 多生产者：Sender::clone ----");

  let (sender, receiver) = mpsc::channel::<String>();
  let mut handles = Vec::new();

  for producer_id in 1..=3 {
    let sender = sender.clone();
    handles.push(thread::spawn(move || {
      for message_id in 1..=2 {
        // 人为制造不同延迟，让接收顺序更容易和排序后的顺序区分开。
        thread::sleep(Duration::from_millis((3 - producer_id) * 3));
        sender
          .send(format!("producer_{producer_id}: message_{message_id}"))
          .expect("receiver dropped");
      }
    }));
  }
  // 原始 sender 也算一个发送端，必须释放，否则 receiver 永远等不到结束信号。
  drop(sender);

  let mut messages: Vec<_> = receiver.into_iter().collect();
  for handle in handles {
    handle.join().expect("producer thread panicked");
  }
  // 消息到达顺序由调度决定；需要稳定展示时，在业务层排序。
  println!("排序前的消息 = {messages:?}\n");
  messages.sort();
  println!("排序后的消息 = {messages:?}");
}

fn bounded_channel() {
  println!("\n---- 3) sync_channel：有界队列和背压 ----");

  let (sender, receiver) = mpsc::sync_channel::<u32>(1);
  let handle = thread::spawn(move || {
    for value in 1..=3 {
      sender.send(value).expect("receiver dropped");
    }
  });

  let values: Vec<_> = receiver.into_iter().collect();
  handle.join().expect("producer thread panicked");
  println!("有界 channel 收到 = {values:?}");
}
