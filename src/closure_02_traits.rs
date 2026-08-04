/**
 * 2. 三大 Trait 模型: Fn / FnMut / FnOnce 与三种捕获方式
 * 捕获发生在闭包声明时
 * 捕获步骤
 * 1. 创建一个对象
 * 2. 将捕获的数据塞入对象
 *    1. 塞入 指针
 *    3. 塞入 值，所有权转移
 * 
 * 捕获 策略
 * 1. 默认捕获 指针
 * 2. 特殊情况 let x: String = y; 捕获值
 * 3. 使用 move 关键字后都按值捕获
 */
pub fn demo() {
  /*----------------- Fn 只读借用 -----------------*/
  let n = 30;
  // struct Fn { n: &n }
  let readonly = || {
    /*
      copy 类型为所有权拷贝(编译时)，
      不是所有权转移，rust 将编译成 只捕获指针
    */
    let temp = n;
    println!("temp：{temp}, n: {n}"); // 内部实现了 &[n] 类似 的借用
  };
  run_fn(readonly);

  /*----------------- FnMut 可变借用 -----------------*/
  let mut value = String::from("js");
  // struct FnMut { value: &mut value } &String
  let mutable = || {
    value.push_str(" is good");
  };
  run_mut(mutable);
  println!("value：{value}");

  /*----------------- FnOnce 闭包可被调用一次 -----------------*/
  let source = String::from("hello");
  println!("source：{:p}", &source);
  // struct FnOnce { source: source };
  let once = || {
    let inner = source;
    println!("inner：{:p}", &inner);
  };
  run_once(once);
}

// 参数兼容性 FnOnce > FnMut > Fn，一般使用 FnMut 如果确定 f 在高阶函数中只调用一次可以使用 FnOnce
fn run_once<F: FnOnce()>(f: F) {
  f()
}

fn run_mut(mut f: impl FnMut()) {
  f()
}

fn run_fn(f: impl Fn()) {
  f()
}