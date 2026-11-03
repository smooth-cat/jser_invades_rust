use std::mem;

use crate::ob_05_r_w::count_up;

/*----------------- 所有者 不用标记生命周期-----------------*/
fn do_move(mut s: String) -> String {
  s.push_str(" world!");
  s
}

pub fn fn_move() {
  let s1 = String::from("hello");

  let s2 = do_move(s1);
  
  println!("{}", s2);

  // mem::drop(s2);
}



/*----------------- 不能返回函数内产生的指针 -----------------*/
fn no_return_inner_ptr() {
  let s = 32;
  let ptr = &s;
  // return ptr;
}

/*----------------- 只能返回传入指针， 自动推导生命周期 -----------------*/
pub fn return_arg(v: &i32) -> &i32 {
  // ...
  return v;
}

pub fn demo() {
  let p2: &i32;
  {
    let a = 10;
    let p = &a;
    p2 = return_arg(p);
    println!("{}", p2);
  }
}

/*----------------- 手动标注 -----------------*/
// fn either(x: &i32, y: &i32) -> &i32 {
//   // count 可能是 1 或 2，每次运行都不一样
//   let count = count_up(); 
//   if count == 1 {
//     return x
//   } 
//   y
// }

// pub fn prize() { 
//   let p;

//   let v1 = 648;
//   let x = &v1;
//   {
//     let v2 = 888;
//     let y = &v2;
//     // let z = x;
//     p = either(x, y); 
//     println!("{}", p);
//   }
// }

// fn only_x<'a>(x: &'a i32, y: &i32) -> &'a i32 {
//   println!("{y}");
//   return x;
// }


/*----------------- 结构体、特征 生命周期 -----------------*/
// 定义带生命周期的结构体
struct Cat<'a> {
  name: &'a str,
}

// pub fn trait_life_circle<'x>() -> Cat<'x> {

//   let tom = String::from("tom");

//   let tom_cat = Cat { name: &tom };
  
//   return tom_cat;
// }

// 定义带生命周期的泛型
trait Name<'a> {
  fn get_name(&self) -> &'a str;
}
impl<'b> Name<'b> for Cat<'b> {
  fn get_name(&self) -> &'b str {
    return &self.name;
  }
}

/*----------------- 闭包生命周期 -----------------*/
/*----------------- 1. 返回值 与 入参相关 -----------------*/
fn from_param<F>(f: F, s: &str) -> &str
where
  F: Fn(&str) -> &str,
{
  f(s)
}
// 简化 for<'a>
fn from_param_sim(f: impl Fn(&str) -> &str, s: &str) -> &str {
  f(s)
}

pub fn closure_param() {
  let trimmed = from_param(|s| s.trim(), "  hello  ");
  let trimmed2 = from_param_sim(|s| s.trim(), "  hello  ");
}

/*----------------- 2. 返回值 与 捕获指针相关 -----------------*/
fn from_catch<'a>(f: impl Fn() -> &'a str) -> &'a str {
  f()
}

pub fn closure_return() {
  let data = String::from("hello");
  let result = from_catch(|| data.as_str());
  let mut a = 10;
  let mut b = &mut a;
}

/*----------------- 3. struct 存储 闭包 -----------------*/
struct Getter<'a> {
  // 闭包至少活 'a 这么长
  f: Box<dyn Fn(&'a str, &'a str) -> &'a str>,
}
pub fn closure_in_struct() {
  let f: Box<dyn for<'a> Fn(&'a str, &'a str) -> &'a str> =
    Box::new(|a, b| {
      if a.len() > b.len() { a } else { b }
    });
  let getter = Getter { f: f };
  // 使用 getter.f 表示字段。这种写法与 impl 定义的方法分开来保证语义准确，这里指调用 结构体中 f 字段存储的 闭包
  (getter.f)("hello", "world!");
}
