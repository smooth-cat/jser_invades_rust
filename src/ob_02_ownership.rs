use ::core::marker::Copy;
use std::mem;
#[allow(unused)]
fn owner_move() {
  let x = String::from("xiao_ming");
  
  // 所有权转移
  let y = x;

  mem::drop(y);
}

#[allow(unused)]
fn owner_copy() {
  let a = 5i32;

  // 所有权复制
  let b = a;

  // ...
}

/*----------------- 函数传参、返回值 也有所有权操作 -----------------*/
pub fn fn_move() {
  let s1 = String::from("hello");

  let s2 = do_move(s1);
  
  println!("{}", s2);
}

fn do_move(mut s: String) -> String {
  s.push_str(" world!");
  s
}