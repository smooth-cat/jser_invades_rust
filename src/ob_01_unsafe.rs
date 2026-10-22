use std::{mem, ptr};
#[allow(unused)]
#[derive(Debug)]
struct Person { name: String }

#[allow(unused)]
pub fn demo() {
  // 生成一个占位符让 a, b 函数结构一致
  let count = 0;
  let placeholder = &count as *const i32;

  // 执行 a 获取一个指针
  let a_ptr = a(placeholder);

  // 执行 b 读取指针指向的数据
  b(a_ptr);

  let person = Person { name: String::from("a") };
}

#[allow(unused)]
fn a(placeholder: *const i32) -> *const Person {
  let person = Person { name: String::from("a") };

  let a_ptr = &person as *const Person;

  // 返回指向 a 调用帧的指针
  return a_ptr;
}

#[allow(unused)]
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
