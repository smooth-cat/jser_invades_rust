//! 线程创建、移动所有权和等待线程结束。
//!
//! JS 通常只有一个事件循环；Rust 可以显式创建操作系统线程，并用
//! `JoinHandle` 等待线程返回结果。

use std::thread;

pub fn demo() {
  spawn_and_join();
  move_ownership_into_thread();
  handle_thread_result();
}

fn spawn_and_join() {
  println!("\n---- 1) thread::spawn + join ----");

  let handles: Vec<_> = (1..=3)
    .map(|worker_id| {
      thread::spawn(move || {
        // `move` 把 worker_id 的所有权移入闭包，线程就不依赖外部栈帧。
        worker_id * worker_id
      })
    })
    .collect();

  let mut squares = Vec::new();
  for handle in handles {
    squares.push(handle.join().expect("worker thread panicked"));
  }
  println!("线程计算结果 = {squares:?}");
}

fn move_ownership_into_thread() {
  println!("\n---- 2) move 把 String 的所有权交给线程 ----");

  let message = String::from("hello from Rust thread");
  let handle = thread::spawn(move || message.to_uppercase());
  // message 已经移动进线程，不能再在这里使用。
  println!(
    "线程返回 = {}",
    handle.join().expect("worker thread panicked")
  );
}

fn handle_thread_result() {
  println!("\n---- 3) join 也可以拿到 Result ----");

  let handle = thread::spawn(|| -> Result<u32, &'static str> {
    let value = 21;
    if value > 0 {
      Ok(value * 2)
    } else {
      Err("value 必须大于 0")
    }
  });

  match handle.join().expect("worker thread panicked") {
    Ok(value) => println!("业务结果 = {value}"),
    Err(error) => println!("业务错误 = {error}"),
  }
}
