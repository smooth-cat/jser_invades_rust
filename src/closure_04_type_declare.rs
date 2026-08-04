//  4. 闭包本身无法添加生命周期，只能在接收它的函数中添加生命周期标注
pub fn demo() {
  /*----------------- 通过回调函数设 生命周期 -----------------*/
  fn param_type<F>(f: F)
  where
    F: for<'a> Fn(&'a str, &'a str) -> &'a str,
  {
    let s1 = String::from("hello");
    {
      let s2 = String::from(" world!");
      println!("{}", f(&s1, &s2));
    }
  }
  param_type(|a, b| if a.len() > b.len() { a } else { b });

  /*----------------- 纯函数 - 函数指针代替 -----------------*/
  fn compare<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
  }
  fn param_fn(f: for<'a> fn(&'a str, &'a str) -> &'a str) {
    let s1 = String::from("hello");
    {
      let s2 = String::from(" world!");
      println!("{}", f(&s1, &s2));
    }
  }
  param_fn(compare);

  /*----------------- 通过 Box + dyn 保存闭包 泛型中可以写声明周期约束 -----------------*/
  let bias = 1;

  let box_fn: Box<dyn for<'a> Fn(&'a str, &'a str) -> &'a str> =
    Box::new(move |x, y| if x.len() + bias > y.len() { x } else { y });

  let a = "hello";
  let b = "rust";

  println!("{}", box_fn(a, b));
  /*----------------- 手动写 结构体捕获 -----------------*/
  struct CatchFn {
    bias: usize,
  }
  impl CatchFn {
    pub fn call<'a>(&self, x: &'a str, y: &'a str) -> &'a str {
      if x.len() + self.bias > y.len() { x } else { y }
    }
  }
  let a = "hello";
  let b = "rust";
  let bias = 1;
  let catch = CatchFn { bias };
  println!("{}", catch.call(a, b));

  /*----------------- 高阶函数 返回 捕获指针 或 参数指针时  -----------------*/
  // 枚举 Borrowed 、Owned 外部可以通过模式匹配处理
  // 也就是说闭包可能返回所有权，或者返回指针
  // 调用后拿到 &str 或 String 那么可以调一些共同的方法
  use std::borrow::Cow;
  let fallback = String::from("default");
  fn make_f(fallback: String) -> impl for<'a> Fn(&'a str) -> Cow<'a, str> {
    return move |x: &str| {
      if !x.is_empty() {
        Cow::Borrowed(x)
      } else {
        Cow::Owned(fallback.clone())
      }
    };
  }

  let f = make_f(fallback);
  println!("{}", f("hello"));
  println!("{}", f(""));
}
