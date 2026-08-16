//! 过程宏接收并返回 TokenStream。它必须定义在独立的 `proc-macro` crate 中。

use demo_macros::{Describe, trace};

pub trait Describe {
  fn describe(&self) -> String;
}

#[derive(Describe)]
#[describe(label = "用户")]
struct User {
  id: u64,
  name: String,
}

// 属性宏能读取并改写整个函数。这里会在函数体前后插入日志。
#[trace]
fn add(left: i32, right: i32) -> i32 {
  left + right
}

// `#[trace]` 对 `add` 的近似展开，真实展开还会保留可见性、签名和其他属性。
// 闭包让原函数中的 `return` 先返回到闭包，保证 exit 日志仍会执行。
#[allow(clippy::redundant_closure_call)]
fn add_expanded(left: i32, right: i32) -> i32 {
  println!("[trace] enter add_expanded");
  let result = (|| left + right)();
  println!("[trace] exit add_expanded");
  result
}

pub fn demo() {
  let user = User {
    id: 7,
    name: "Lin".to_owned(),
  };

  // `#[derive(Describe)]` 生成了 `impl Describe for User`。
  println!("{}", user.describe());
  assert_eq!(user.describe(), "用户 { id: 7, name: \"Lin\" }");

  assert_eq!(add(20, 22), 42);
  assert_eq!(add_expanded(20, 22), 42);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn derive_macro_implements_the_trait() {
    let user = User {
      id: 1,
      name: "Ada".to_owned(),
    };
    assert_eq!(user.describe(), "用户 { id: 1, name: \"Ada\" }");
  }

  #[test]
  fn attribute_macro_preserves_the_function_result() {
    assert_eq!(add(2, 3), 5);
  }
}
