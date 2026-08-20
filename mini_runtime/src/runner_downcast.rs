use std::{
  any::Any,
  collections::HashMap,
  future::Future,
  pin::Pin,
  sync::{Arc, mpsc},
  task::{Context, Poll, Wake, Waker},
};

/*----------------- 唤醒器 -----------------*/
pub struct MyWaker {
  id: usize,
  sender: mpsc::Sender<usize>,
}
impl MyWaker {
  pub fn new(sender: mpsc::Sender<usize>, id: usize) -> Arc<Self> {
    Arc::new(Self { id, sender })
  }
}
impl Wake for MyWaker {
  fn wake(self: Arc<Self>) {
    self.sender.send(self.id).unwrap();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.sender.send(self.id).unwrap();
  }
}
type AnyValue = Box<dyn Any + 'static>;
/*----------------- 任务类型 -----------------*/
// 同步任务
struct SyncTask {
  // 完成的回调
  on_ready: Box<dyn FnOnce() + 'static>,
}
// 异步任务
struct AsyncTask<T> {
  // 包含 poll 触发器
  future: Pin<Box<dyn Future<Output = T> + 'static>>,
  // 通知 event_loop 再去调用一下 poll
  waker: Waker,
  // 完成的回调
  on_ready: Box<dyn FnOnce(T) + 'static>,
}
// 任务枚举
enum Task<T> {
  Async(AsyncTask<T>),
  Sync(SyncTask),
}

/*----------------- 事件循环 -----------------*/
pub struct MyLoop {
  // 异步任务
  pending_tasks: HashMap<usize, Task<AnyValue>>,
  // 接收器
  ready_ids: mpsc::Receiver<usize>,
  // 发送器，同步任务要给自己发送 任务 id
  sender: mpsc::Sender<usize>,
  // AsyncTask 唯一标识
  task_id: usize,
}
impl MyLoop {
  pub fn new() -> Self {
    let (sender, ready_ids) = mpsc::channel();
    Self {
      pending_tasks: HashMap::new(),
      ready_ids,
      sender,
      task_id: 0,
    }
  }

  pub fn add_sync(&mut self, task: impl FnOnce() + 'static) {
    let id = self.task_id;
    // 统一加入 pending 队列
    self.pending_tasks.insert(
      id,
      Task::Sync(SyncTask {
        on_ready: Box::new(task),
      }),
    );
    // id 加入 ready_ids
    self.sender.send(id).unwrap();
    self.task_id += 1;
  }

  // 类型擦除后的内部入口。事件循环只需要处理统一的 T。
  pub fn _add_async(
    &mut self,
    mut future: Pin<Box<dyn Future<Output = AnyValue> + 'static>>,
    on_ready: impl FnOnce(AnyValue) + 'static,
  ) -> usize {
    let id = self.task_id;
    self.task_id += 1;
    let waker = Waker::from(MyWaker::new(self.sender.clone(), id));
    let mut cx = Context::from_waker(&waker);
    match future.as_mut().poll(&mut cx) {
      // 同步执行完成应该直接 需执行回调
      Poll::Ready(data) => {
        on_ready(data);
      }
      // 未完成
      Poll::Pending => {
        let task = AsyncTask {
          future,
          on_ready: Box::new(on_ready),
          waker,
        };
        self.pending_tasks.insert(id, Task::Async(task));
      }
    }
    id
  }

  // 对外接受任意 Future，在进入事件循环前转换为 AnyValue。
  pub fn add_async<F, O, C>(&mut self, future: F, then: C) -> usize
  where
    F: Future<Output = O> + 'static,
    O: 'static,
    C: FnOnce(O) + 'static,
  {
    let any_future: Pin<Box<dyn Future<Output = AnyValue> + 'static>> =
      Box::pin(async move { Box::new(future.await) as AnyValue });

    let any_then: Box<dyn FnOnce(AnyValue) + 'static> = Box::new(move |value| {
      let value = value.downcast::<O>().expect("async output type mismatch");
      then(*value);
    });

    self._add_async(any_future, any_then)
  }

  pub fn run(&mut self) {
    while let Ok(id) = self.ready_ids.recv() {
      if let Some(task) = self.pending_tasks.remove(&id) {
        match task {
          // 同步任务 直接执行
          Task::Sync(task) => {
            (task.on_ready)();
          }
          // 异步任务 先 poll 根据情况 调用 on_ready
          Task::Async(AsyncTask {
            mut future,
            on_ready,
            waker,
          }) => {
            let cx = &mut Context::from_waker(&waker);
            match future.as_mut().poll(cx) {
              // 未完成就继续 poll
              Poll::Pending => {
                self.pending_tasks.insert(
                  id,
                  Task::Async(AsyncTask {
                    future,
                    on_ready,
                    waker,
                  }),
                );
              }
              // 完成就执行回调
              Poll::Ready(data) => {
                (on_ready)(data);
              }
            }
          }
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{thread, time};

  use crate::{future::AsyncSpawn, runner_downcast::MyLoop};

  #[test]
  fn demo() {
    let mut even_loop = MyLoop::new();

    even_loop.add_async(
      AsyncSpawn::new(|| {
        thread::sleep(time::Duration::from_millis(2000));
        println!("2s 后 多线程执行");
        42
      }),
      |result| {
        if let Ok(num) = result {
          println!("多线程执行结果：{}", num);
        }
      },
    );

    even_loop.add_async(AsyncSpawn::new(|| String::from("123")), |str| {
      println!("异步字符串：{:?}", str);
    });

    even_loop.add_sync(|| {
      println!("同步执行");
    });
    even_loop.run();
    println!("开始执行");
  }
}

// Box::pin(async {
//   let res = AsyncSpawn::new(|| {
//     println!("多线程执行");
//     42
//   })
//   .await;
//   let double = res.map(|v| v * 2);
//   double
// }),
