mod thread_01_spawn;
mod thread_02_channel;
mod thread_03_send_sync;
mod thread_04_arc_mutex;
mod thread_05_rwlock;
mod thread_06_atomic;

fn main() {
  let demos: &[(&str, fn())] = &[
    ("1. 线程创建与 join", thread_01_spawn::demo),
    ("2. channel 消息传递", thread_02_channel::demo),
    ("3. Send 与 Sync", thread_03_send_sync::demo),
    ("4. Arc<Mutex<T>>", thread_04_arc_mutex::demo),
    ("5. RwLock", thread_05_rwlock::demo),
    ("6. 原子类型", thread_06_atomic::demo),
  ];

  for (title, demo) in demos {
    println!("\n{:=^60}", format!(" {title} "));
    demo();
  }
}
