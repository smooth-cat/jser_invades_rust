/*----------------- 正常不应该实现在 运行时内部 -----------------*/
use std::{
  future::Future,                          // Future trait，异步任务的基础
  panic::{AssertUnwindSafe, catch_unwind}, // 捕获线程 panic，避免任务卡死
  pin::Pin,                                // 固定指针，确保 Future 不被移动
  sync::{Arc, Mutex},                      // 多生产者单消费者通道、原子引用计数、互斥锁
  task::{Context, Poll, Waker},            // 异步任务上下文、轮询结果、唤醒机制
  thread,                                  // 线程相关操作
};

// 把传入的 Waker 存起来
struct State<T> {
  result: Option<thread::Result<T>>, // 线程执行结果，可能包含 panic 信息
  waker: Option<Waker>,              // 唤醒器，用于通知执行器重新 poll
}

// 在完成前被取消，工作线程也会继续执行，但不会再有主线程回调。
pub struct AsyncSpawn<T> {
  state: Arc<Mutex<State<T>>>, // 共享状态，工作线程和执行器都可以访问
}

impl<T: Send + 'static> AsyncSpawn<T> {
  // 创建一个新的异步线程任务。
  pub fn new<F>(f: F) -> Self
  where
    F: FnOnce() -> T + Send + 'static,
  {
    // 初始化共享状态，结果为空，唤醒器为空
    let state = Arc::new(Mutex::new(State {
      result: None,
      waker: None,
    }));

    // 克隆 Arc 供工作线程使用
    let state_for_worker = Arc::clone(&state);
    // 启动工作线程
    thread::spawn(move || {
      // 执行 f, 把 Panic 变成 Result
      let result = catch_unwind(AssertUnwindSafe(f));
      // 获取唤醒器并写入结果
      let waker = {
        let mut state = state_for_worker.lock().unwrap(); // 锁定共享状态
        state.result = Some(result); // 保存执行结果
        state.waker.take() // 取出唤醒器（如果有）
      }; // 锁在此作用域结束自动释放

      // 解锁后再唤醒，避免 waker 的实现同步 poll 时产生死锁。
      if let Some(waker) = waker {
        waker.wake(); // 唤醒执行器，通知任务完成
      }
    });
    Self { state }
  }
}

impl<T: Send + 'static> Future for AsyncSpawn<T> {
  type Output = thread::Result<T>; // 输出为线程结果，可能包含 panic

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    // 锁定状态并获取可变引用
    let state = &mut *self.get_mut().state.lock().unwrap();
    match state.result.take() {
      // 如果结果已经存在，返回 Ready
      Some(result) => Poll::Ready(result),
      None => {
        // 否则，保存当前任务的 Waker，稍后由工作线程唤醒
        state.waker = Some(cx.waker().clone());
        Poll::Pending
      }
    }
  }
}
