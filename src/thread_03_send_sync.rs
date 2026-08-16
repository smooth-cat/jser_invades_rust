//! `Send` 和 `Sync` 是线程安全的类型级契约。
//!
//! - `Send`：值的所有权可以移动到另一个线程。
//! - `Sync`：`&T` 可以在线程之间共享；等价于 `T: Sync`。

use std::thread;

#[derive(Debug)]
struct Score {
  value: u32,
}

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

pub fn demo() {
  compile_time_contracts();
  send_moves_ownership();
  sync_shares_read_only_borrow();
}

fn compile_time_contracts() {
  println!("\n---- 1) Send / Sync 编译期检查 ----");

  // 这些调用没有运行时行为，但能在编译期验证 trait 约束。
  assert_send::<String>();
  assert_send::<Score>();
  assert_sync::<String>();
  assert_sync::<Score>();
  println!("String 和 Score 同时满足 Send + Sync");

  // Rc<T> 没有 Send / Sync，不能直接 move 到 thread::spawn。
  // assert_send::<std::rc::Rc<String>>(); // 编译错误：Rc 不是线程安全的
}

fn send_moves_ownership() {
  println!("\n---- 2) Send：move 所有权到另一个线程 ----");

  let score = Score { value: 42 };
  let handle = thread::spawn(move || score.value + 1);
  println!(
    "线程拿到 Score 后计算 = {}",
    handle.join().expect("thread panicked")
  );
}

fn sync_shares_read_only_borrow() {
  println!("\n---- 3) Sync：多个线程共享只读借用 ----");

  let text = String::from("同一份只读字符串");
  // thread::scope 允许线程借用当前栈上的 text；所有线程结束后 scope 才返回。
  let lengths = thread::scope(|scope| {
    let first = scope.spawn(|| text.len());
    let second = scope.spawn(|| text.chars().count());
    [
      first.join().expect("reader thread panicked"),
      second.join().expect("reader thread panicked"),
    ]
  });

  println!("两个线程读取同一份数据 = {lengths:?}");
}
