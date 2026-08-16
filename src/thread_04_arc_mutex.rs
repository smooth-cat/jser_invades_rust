//! `Arc<Mutex<T>>`：跨线程共享且可变的状态。
//! Mutex：互斥锁，同时只能有一个线程能访问，不论读写

use std::sync::{Arc, Mutex};
use std::thread;

pub fn demo() {
  shared_counter();
  lock_guard_scope();
}

fn shared_counter() {
  println!("\n---- 1) Arc<Mutex<T>> 共享计数器 ----");

  let counter = Arc::new(Mutex::new(0_u32));
  let mut handles = Vec::new();

  for _ in 0..4 {
    let counter = Arc::clone(&counter);
    handles.push(thread::spawn(move || {
      for _ in 0..1_000 {
        // 1. 如果数据被其他线程上锁，阻塞，等待锁
        // 2. 锁权限在作用域结束后释放
        let mut value = counter.lock().expect("mutex poisoned");
        *value += 1;
      }
    }));
  }

  for handle in handles {
    handle.join().expect("worker thread panicked");
  }
  println!(
    "4 个线程各加 1000 次，结果 = {}",
    *counter.lock().expect("mutex poisoned")
  );
}

fn lock_guard_scope() {
  println!("\n---- 2) MutexGuard 的作用域 ----");

  let state = Mutex::new(String::from("待处理"));
  {
    let mut guard = state.lock().expect("mutex poisoned");
    guard.push_str(" -> 已处理");
    println!("持有锁时 = {guard}");
  } // guard drop，锁在这里释放

  println!(
    "释放锁后仍可再次加锁 = {}",
    state.lock().expect("mutex poisoned")
  );
}
