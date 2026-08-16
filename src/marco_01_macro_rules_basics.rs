//! `macro_rules!` 是按语法结构匹配并生成 Rust 代码的声明宏。
//! 宏调用发生在编译期；运行时只能看到它生成的代码。

macro_rules! js_log {
  // `$label:expr` 匹配表达式，`$value:expr` 也匹配表达式。
  ($label:expr, $value:expr) => {{
    // 两层花括号创建独立作用域，避免宏内变量泄漏到调用处。
    let value = $value;
    println!("{}: {:?}", $label, &value);
    value
  }};
}

macro_rules! make_greeting {
  // `ident` 匹配标识符，`literal` 匹配字面量。
  ($function_name:ident, $message:literal) => {
    fn $function_name(name: &str) -> String {
      format!(concat!($message, ", {}!"), name)
    }
  };
}

make_greeting!(hello, "Hello");
make_greeting!(ni_hao, "你好");

pub fn demo() {
  let answer = js_log!("1 + 2", 1 + 2);
  assert_eq!(answer, 3);

  println!("{}", hello("Rust"));
  println!("{}", ni_hao("宏"));

  // `js_log!("1 + 2", 1 + 2)` 的近似展开：
  let expanded_answer = {
    let value = 1 + 2;
    println!("手写展开: {value:?}");
    value
  };
  assert_eq!(answer, expanded_answer);
}

#[cfg(test)]
mod tests {
  use crate::marco_01_macro_rules_basics;

  #[test]
  fn a_macro_can_return_the_value_of_its_generated_block() {
    let value = js_log!("test", 40 + 2);
    assert_eq!(value, 42);
  }

  #[test]
  fn a_macro_can_generate_items() {
    assert_eq!(marco_01_macro_rules_basics::hello("JSer"), "Hello, JSer!");
  }
}
