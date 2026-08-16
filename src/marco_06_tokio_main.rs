//! `#[tokio::main]` 把 `async fn` 改写成创建 runtime 并 `block_on` 的同步函数。

async fn async_answer() -> u32 {
  tokio::task::yield_now().await;
  42
}

#[tokio::main(flavor = "current_thread")]
async fn run_with_tokio_main() -> u32 {
  async_answer().await
}

// 上面属性宏的近似展开。细节会随 Tokio 版本变化，但核心结构相同。
fn run_with_manual_runtime() -> u32 {
  tokio::runtime::Builder::new_current_thread()
    .build()
    .expect("failed to build Tokio runtime")
    .block_on(async { async_answer().await })
}

pub fn demo() {
  // 源码写的是 async fn，但属性宏展开后它已变成可直接调用的同步函数。
  let from_attribute = run_with_tokio_main();
  let from_manual_runtime = run_with_manual_runtime();

  println!("#[tokio::main] 的结果: {from_attribute}");
  println!("手写 runtime 的结果: {from_manual_runtime}");
  assert_eq!(from_attribute, from_manual_runtime);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn macro_and_manual_expansion_run_the_same_future() {
    assert_eq!(run_with_tokio_main(), run_with_manual_runtime());
  }
}
