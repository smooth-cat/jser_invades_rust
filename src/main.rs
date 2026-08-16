fn main() {
  let demos: &[(&str, fn())] = &[
    (
      "1. macro_rules! 基础",
      demo::marco_01_macro_rules_basics::demo,
    ),
    (
      "2. macro_rules! 匹配、重复与递归",
      demo::marco_02_macro_rules_patterns::demo,
    ),
    ("3. 阅读 derive 展开", demo::marco_03_derive::demo),
    ("4. cfg 与条件编译属性", demo::marco_04_cfg_attributes::demo),
    (
      "5. 自定义派生宏与属性宏",
      demo::marco_05_procedural_macros::demo,
    ),
    ("6. 阅读 tokio::main 展开", demo::marco_06_tokio_main::demo),
  ];

  for (title, demo) in demos {
    println!("\n{:=^60}", format!(" {title} "));
    demo();
  }
}
