mod closure_01_syntax;
mod closure_02_traits;
mod closure_03_move;
mod closure_04_type_declare;
mod closure_05_iterator;
mod closure_06_pitfalls;

fn main() {
  let demos: &[(&str, fn())] = &[
    ("1. 闭包定义与语法", closure_01_syntax::demo),
    ("2. 三大 Trait 模型与捕获方式", closure_02_traits::demo),
    ("3. move 关键字", closure_03_move::demo),
    ("4. 闭包作为参数", closure_04_type_declare::demo),
    ("5. 闭包 + 迭代器", closure_05_iterator::demo),
    ("6. 常见坑", closure_06_pitfalls::demo),
  ];

  for (title, demo) in demos {
    println!("\n{:=^60}", format!(" {title} "));
    demo();
  }
}
