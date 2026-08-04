// 6. 闭包常见坑

pub fn demo() {
  // ---- 坑 1: 闭包的可变借用阻塞外部访问 ----
  let mut data = vec![1, 2, 3];
  let mut push = |x: i32| data.push(x);
  push(4);
  // data.push(5); // 编译错误: data 仍被 push 可变借用
  drop(push); // 释放借用
  data.push(5);
  println!("data = {data:?}");

  // ---- 坑 2: 不可变借用可共存, 可变借用互斥 ----
  let v = vec![1, 2, 3];
  let read1 = || v.len();
  let read2 = || v[0];
  println!("多个只读闭包共存: {} {}", read1(), read2());

  // ---- 坑 3: 按值捕获后原变量失效 ----
  let s = String::from("hello");
  let consume = || drop(s);
  consume();
  // println!("{s}"); // 编译错误: s 已移入闭包

  // ---- 坑 4: 捕获粒度: edition 2021 起按字段捕获, 旧版按整个变量 ----
  // 旧教程常说的"整体捕获"在 2021 版已改为按字段捕获
  let pair = (String::from("a"), String::from("b"));
  let take_first = move || pair.0; // 只捕获字段 0
  println!("pair.1 未被捕获, 仍可用: {}", pair.1);
  let first = take_first();
  println!("first = {first}");
  // println!("{}", pair.0); // 编译错误: 已随闭包移走

  // ---- 坑 5: 闭包不能直接递归 (体内无法引用自身) ----
  // let fib = |n| if n <= 1 { n } else { fib(n - 1) + fib(n - 2) };
  // 解法 A: 改用普通函数
  fn fib(n: u64) -> u64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
  }
  println!("普通函数递归 fib(10) = {}", fib(10));
  // 解法 B: 装箱, 体内调用普通函数
  let fib_boxed: Box<dyn Fn(u64) -> u64> = Box::new(move |n: u64| {
    fn go(n: u64) -> u64 {
      if n <= 1 { n } else { go(n - 1) + go(n - 2) }
    }
    go(n)
  });
  println!("装箱版 fib(10) = {}", fib_boxed(10));

  // ---- 坑 6: 返回借用局部变量的闭包 -> 必须 move ----
  // fn bad() -> impl Fn() -> i32 {
  //     let n = 42;
  //     || n // 编译错误: n 的存活时间不够
  // }
  fn good() -> impl Fn() -> i32 {
    let n = 42;
    move || n
  }
  println!("good()() = {}", good()());

  // ---- 坑 7: FnOnce 闭包非 Copy, 传参即移动 ----
  let t = String::from("once");
  let once = move || t;
  let _ = once; // 移动
  // println!("{}", once()); // 编译错误: 已移动

  // ---- 坑 8: 循环内捕获变量, 必须 move 才能得到独立副本 ----
  // JS: for(var i) 的闭包全部捕获同一个 i; 同样的问题在 Rust 借壳存在
  let handlers: Vec<Box<dyn Fn() -> i32>> = (0..3)
    .map(|i| -> Box<dyn Fn() -> i32> { Box::new(move || i) })
    .collect();
  for h in &handlers {
    println!("handler = {}", h());
  }
}
