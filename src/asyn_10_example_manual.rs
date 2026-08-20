//! 根据 example.mir 手工还原出的外层 Future 状态机。
//!
//! 这不是 rustc 输出的原始源码，而是一个便于阅读的等价实现：
//! MIR 中的 discriminant 被表达成 State 枚举，example::{closure#0}
//! 被表达成 ExampleFuture 的 Future::poll 方法。

use std::future::{Future, Pending, Ready};
use std::mem;
use std::pin::{Pin, pin};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

/// 对应 example.rs 中的 foo；它本身仍由编译器生成一个 Future。
pub async fn foo() -> i32 {
  10
}

/// 对应 example.rs 中的 bar；它本身仍由编译器生成一个 Future。
pub async fn bar(a: i32) -> i32 {
  a + 1
}

async fn _example(x: i32) -> i32 {
  let a = foo().await;
  let b = bar(a).await;
  b + x
}

/// 为了在手写状态机中保存不同的匿名 Future，将它们擦除为同一类型。
type BoxFutureI32 = Pin<Box<dyn Future<Output = i32>>>;

/// 对应 MIR 中外层 Future 的不同 discriminant 状态。
enum State {
  /// MIR 的初始状态 0，还没有开始执行函数体。
  Start { x: i32 },
  /// MIR 的状态 3：暂停在 foo().await。
  WaitingFoo { x: i32, future: BoxFutureI32 },
  /// MIR 的状态 4：暂停在 bar(a).await。
  WaitingBar { x: i32, future: BoxFutureI32 },
  /// 临时占位状态，用于从 self.state 中取出当前状态。
  Polling,
  /// MIR 的完成状态 1。
  Done,
}

/// 手工表达 MIR 中 `{async fn body of example()}` 的 Future。
pub struct ExampleFuture {
  state: State,
}

impl ExampleFuture {
  /// 对应 MIR 中的 `example(_1)`：只创建状态机，不执行函数体。
  pub fn new(x: i32) -> Self {
    Self {
      state: State::Start { x },
    }
  }
}

impl Future for ExampleFuture {
  type Output = i32;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    // 当前结构只保存指针和整数；get_mut 不会移动被 Pin 的子 Future。
    let this = self.as_mut().get_mut();

    loop {
      // MIR 的 bb0 会读取 discriminant；这里取出并匹配 State。
      // Polling 是临时占位，避免在匹配旧状态时同时修改 self.state。
      let state = mem::replace(&mut this.state, State::Polling);

      match state {
        State::Start { x } => {
          // 对应 MIR bb1-bb3：创建 foo()，并把它保存到状态机。
          this.state = State::WaitingFoo {
            x,
            future: Box::pin(foo()),
          };
        }

        State::WaitingFoo { x, mut future } => {
          // 对应 MIR bb4-bb6：Pin 后 poll foo Future。
          match future.as_mut().poll(cx) {
            Poll::Pending => {
              // 对应 MIR bb8：保持状态 3，并暂停整个 Future。
              this.state = State::WaitingFoo { x, future };
              return Poll::Pending;
            }

            Poll::Ready(a) => {
              // 对应 MIR bb9-bb12：得到 a，然后创建 bar(a)。
              this.state = State::WaitingBar {
                x,
                future: Box::pin(bar(a)),
              };
            }
          }
        }

        State::WaitingBar { x, mut future } => {
          // 对应 MIR bb13-bb15：Pin 后 poll bar Future。
          match future.as_mut().poll(cx) {
            Poll::Pending => {
              // 对应 MIR bb16：保持状态 4，并暂停整个 Future。
              this.state = State::WaitingBar { x, future };
              return Poll::Pending;
            }

            Poll::Ready(b) => {
              // 对应 MIR bb17-bb19：计算 b + x 并完成 Future。
              this.state = State::Done;
              return Poll::Ready(b + x);
            }
          }
        }

        State::Polling => {
          // 这是内部占位状态，正常 poll 流程不会观察到它。
          unreachable!("state was observed while it was being polled");
        }

        State::Done => {
          // 对应 MIR bb26：Future 完成后再次 poll 是非法的。
          panic!("Future polled after completion");
        }
      }
    }
  }
}

struct MyWaker(thread::Thread);

impl Wake for MyWaker {
  fn wake(self: Arc<Self>) {
    self.0.unpark();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.0.unpark();
  }
}

/// 手写 Future 版本的 example。
fn example(x: i32) {
  // 准备 ctx
  let waker = Waker::from(Arc::new(MyWaker(thread::current())));
  let mut ctx = Context::from_waker(&waker);

  // 用 pin 包裹 machine
  let machine = ExampleFuture::new(x);
  let mut pined_machine = pin!(machine);
  let f = pined_machine.as_mut();

  // 执行
  let state = f.poll(&mut ctx);
  println!("state: {:?}", state);
}

pub fn demo() {
  crate::section("10. async => Feature");
  example(9);
}
