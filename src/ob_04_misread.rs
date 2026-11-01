use std::{mem, ptr};
#[allow(unused)]
#[derive(Debug)]
struct Person {
  name: String,
}

pub fn demo() {
  // 执行 a 获取一个指针
  let a_ptr = a(&1);
  // 执行 b 读取指针指向的数据
  b(a_ptr);
}

fn a(placeholder: *const i32) -> *const Person {
  let person = Person { name: String::from("a") };
  let a_ptr = &person as *const Person;
  // 返回指向 a 调用帧的指针
  return a_ptr;
}

fn b(a_ptr: *const Person) {
  let person = Person { name: String::from("b") };
  unsafe {
    // 从 a_ptr 读出数据
    let maybe = ptr::read::<Person>(a_ptr);
    println!("人是：{:?}", maybe);
    mem::forget(maybe);
  }
}

#[test]
fn run() { demo() }

pub fn foo() -> &'static i32 {
  let x = 1;
  let x_ptr = &x;
  // return x_ptr;
  return &1;
}