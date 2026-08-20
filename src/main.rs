use demo::{
  asyn_01_future_basics, asyn_02_async_await, asyn_03_runtime, asyn_04_task, asyn_05_select, asyn_06_channel, asyn_07_cancellation, asyn_08_pin, asyn_09_mini_runtime, asyn_10_example_manual,
};

fn main() {
  println!("{:=^72}", " Rust 异步编程 ");
  asyn_01_future_basics::demo();
  asyn_02_async_await::demo();
  asyn_03_runtime::demo();
  asyn_04_task::demo();
  asyn_05_select::demo();
  asyn_06_channel::demo();
  asyn_07_cancellation::demo();
  asyn_08_pin::demo();
  asyn_09_mini_runtime::demo();
  asyn_10_example_manual::demo();
}
