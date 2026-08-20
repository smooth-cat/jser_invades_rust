//! 01 - `Future`：一个 future 只是“将来可能产生值”的惰性计算。
//!
//! `poll` 返回 `Pending` 时，future 必须安排一个唤醒；执行器收到唤醒后会再次
//! poll 它。`async` 块最终也只是编译器生成的 `Future`。

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// 一个只会返回一次值的最小 future。
pub struct Ready<T>(Option<T>);

// `Ready` 没有任何自引用或地址不变的约束，即使 `T: !Unpin` 也可以移动整个值。
impl<T> Unpin for Ready<T> {}

impl<T> Ready<T> {
  pub fn new(value: T) -> Self {
    Self(Some(value))
  }
}

impl<T> Future for Ready<T> {
  type Output = T;

  fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
    // `Ready` 没有自引用，所以这里可以安全地取得可变引用。
    Poll::Ready(
      self
        .as_mut()
        .get_mut()
        .0
        .take()
        .expect("polled after completion"),
    )
  }
}

/// 第一次 poll 主动让出执行权，第二次 poll 才完成。
pub struct YieldOnce {
  yielded: bool,
}

impl YieldOnce {
  pub fn new() -> Self {
    Self { yielded: false }
  }
}

impl Default for YieldOnce {
  fn default() -> Self {
    Self::new()
  }
}

impl Future for YieldOnce {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if self.yielded {
      Poll::Ready(())
    }
    // 第一次进入
    else {
      self.yielded = true;
      // 真正的 IO future 会在 IO 完成时调用这个 waker；这里立即唤醒以便演示。
      cx.waker().wake_by_ref();
      Poll::Pending
    }
  }
}

/// 创建一个什么也不做的 waker，只用于本文件的手动 poll 演示。
fn noop_waker() -> Waker {
  // 这四个函数不持有数据，因此 clone/drop/wake 都无需操作指针。
  unsafe fn clone(_: *const ()) -> RawWaker {
    RawWaker::new(std::ptr::null(), &NOOP_VTABLE)
  }
  unsafe fn wake(_: *const ()) {}
  unsafe fn wake_by_ref(_: *const ()) {}
  unsafe fn drop(_: *const ()) {}

  static NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

  // `RawWaker` 的 data 是空指针，且 vtable 中的函数不会解引用它。
  unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &NOOP_VTABLE)) }
}

pub async fn answer() -> u32 {
  YieldOnce::new().await;
  Ready::new(40).await + 2
}

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("01. Future");
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  let mut future = Box::pin(Ready::new(42));
  assert!(matches!(
    future.as_mut().poll(&mut context),
    Poll::Ready(42)
  ));

  // 不调用 `.await` 或 `poll`，上面的 future 就不会执行；await 是运行它的语法。
  let value = answer().await;
  println!("Future::poll 手动得到 {value}; async future 得到 {}", value);
  assert_eq!(value, 42);
}
