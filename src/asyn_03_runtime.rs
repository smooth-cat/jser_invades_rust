//! 03 - 运行时：执行器负责反复 poll future，反应器负责在事件发生时唤醒 task。
//!
//! 这里直接给 demo 入口加 `#[tokio::main]`，这样能看到属性宏背后“创建 runtime +
//! `block_on`”的核心步骤；顶层 `main` 只需要按顺序调用普通同步函数。

use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("03. runtime");
  tokio::time::sleep(Duration::from_millis(1)).await;
  let value = 21 * 2;

  println!("#[tokio::main] 创建 runtime 并驱动 future，结果: {value}");
  assert_eq!(value, 42);
}

