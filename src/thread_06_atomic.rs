//! 原子类型：在不使用互斥锁的情况下安全更新简单共享状态。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

pub fn demo() {
  atomic_counter();
  compare_and_exchange();
}

fn atomic_counter() {
  println!("\n---- 1) AtomicUsize::fetch_add ----");

  let counter = Arc::new(AtomicUsize::new(0));
  let mut handles = Vec::new();

  for _ in 0..4 {
    let counter = Arc::clone(&counter);
    handles.push(thread::spawn(move || {
      for _ in 0..1_000 {
        // 计数器只关心加法本身的原子性，Relaxed 已足够。
        counter.fetch_add(1, Ordering::Relaxed);
      }
    }));
  }

  for handle in handles {
    handle.join().expect("worker thread panicked");
  }
  println!("原子计数结果 = {}", counter.load(Ordering::Relaxed));

  // AtomicBool 常用于线程间传递一个停止/开关信号。
  let enabled = AtomicBool::new(true);
  enabled.store(false, Ordering::Relaxed);
  println!("AtomicBool = {}", enabled.load(Ordering::Relaxed));
}

fn compare_and_exchange() {
  println!("\n---- 2) compare_exchange：无锁状态转换 ----");

  let state = AtomicUsize::new(0); // 0 = idle, 1 = running
  let started = state
    .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
    .is_ok();
  let started_again = state
    .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
    .is_ok();

  println!("第一次启动成功 = {started}，第二次启动成功 = {started_again}");
}
