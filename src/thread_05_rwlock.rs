//! `RwLock<T>`：读多写少时允许多个读者共享访问。
//!  读/写 互斥，多个读不互斥

use std::sync::{Arc, RwLock};
use std::thread;

#[derive(Debug)]
struct Config {
  version: u32,
  mode: String,
}

pub fn demo() {
  readers_and_writer();
}

fn readers_and_writer() {
  println!("\n---- RwLock：多个读者与独占写者 ----");

  let config = Arc::new(RwLock::new(Config {
    version: 1,
    mode: String::from("development"),
  }));

  // 两个读线程只拿 read 锁，可以同时进行，不需要复制 Config。
  let mut readers = Vec::new();
  for reader_id in 1..=2 {
    let config = Arc::clone(&config);
    readers.push(thread::spawn(move || {
      let guard = config.read().expect("rwlock poisoned");
      (reader_id, guard.version, guard.mode.clone())
    }));
  }

  let mut snapshots = Vec::new();
  for reader in readers {
    snapshots.push(reader.join().expect("reader thread panicked"));
  }
  snapshots.sort_by_key(|snapshot| snapshot.0);
  println!("读线程快照 = {snapshots:?}");

  // 读锁全部释放之后，写线程才能获得独占 write 锁。
  let config_for_writer = Arc::clone(&config);
  let writer = thread::spawn(move || {
    let mut guard = config_for_writer.write().expect("rwlock poisoned");
    guard.version += 1;
    guard.mode = String::from("production");
  });
  writer.join().expect("writer thread panicked");

  let final_config = config.read().expect("rwlock poisoned");
  println!("写线程更新后 = {final_config:?}");
}
