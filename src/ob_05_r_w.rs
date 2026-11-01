use std::{
  ptr,
  thread::{scope, sleep},
  time::Duration,
};

#[derive(Clone, Copy)]
struct RawPtr(*mut i32);
unsafe impl Send for RawPtr {}
unsafe impl Sync for RawPtr {}
impl RawPtr {
  fn get(self) -> *mut i32 {
    self.0
  }
}
#[test]
fn run() { count_up(); }
pub fn count_up() -> i32 {
  let mut count = 0;
  scope(|s| {
    // 同时派出 2 个线程，每个线程都让 count + 1
    for _ in 1..=2 {
      let p = RawPtr(&mut count);
      s.spawn(move || unsafe {
        // *p = *p + 1;
        let p = p.get();
        let v = ptr::read(p);
        // 睡 1 纳秒
        sleep(Duration::from_nanos(1));
        ptr::write(p, v + 1);
      });
    }
  });
  println!("count: {count}");
  count
}
