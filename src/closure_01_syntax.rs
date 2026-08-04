// 1. 闭包定义与语法
// JS:  const add = (a, b) => a + b;
// Rust: let add = |a, b| a + b;

pub fn demo() {
  // ---- 1) 基础写法: 类型推断 + 表达式体 (最常用) ----
  let double = |x| x * 2;
  println!("double(21) = {}", double(21));

  // ---- 2) 多参数 + 显式类型标注 + 返回类型 + 块体 ----
  let add = |x: i32, y: i32| -> i32 {
    let sum = x + y;
    sum // 块体最后一行即返回值, 无需 return
  };
  println!("add(1, 2) = {}", add(1, 2));

  // ---- 3) 捕获环境变量 (细节见 03) ----
  let base = 100;
  let add_base = |x: i32| x + base;
  println!("add_base(1) = {}", add_base(1));
}
