/**
 * 3. move 改为 值捕获
 * 捕获 策略
 * 1. 默认捕获 指针
 * 2. 特殊情况 单独 let x: String = y; 这一行 捕获值
 * 3. 使用 move 关键字后都按值捕获
 */

use std::{mem, thread};

pub fn demo() {
  let s = String::from("hello");
  /*----------------- Fn vs Fn + move -----------------*/
  // struct Fn<'a> { s: &'a s } 捕获借用
  let print_no_move = || {
    println!("无 move 闭包: {}", s);
  };
  print_no_move();

  println!("s 在 move 闭包外的地址{:p}", &s);
  // struct Fn { s: s } 捕获所有权
  let print_move = move || {
    println!("s 在 move 闭包内的地址{:p}", &s);
    println!("有 move 闭包: {}", s); // 内部是借用 &[s]
  };
  // println!("{s}"); // borrow of moved value: `s`
  print_move();

  /*----------------- FnOnce vs FnOnce + move -----------------*/
  #[derive(Debug)]
  struct Foo { age: i32 }
  impl Drop for Foo {
    fn drop(&mut self) {
      println!("Foo 被释放，内容为: {:?}", self);
    }
  }

  let foo = Foo { age: 1 };
  let other = Foo { age: 100 };
  // struct FnOnce { foo: foo, other：&other } 捕获 &other 借用，s2 移动
  let once_only = || {
    let moved = foo;
    println!("fn once 闭包: {:?}, 捕获的 other: {:?}", moved, other);
  };
  // 释放 once_only ， 释放 foo
  mem::drop(once_only);

  let foo2 = Foo { age: 2 };
  // struct FnOnce { foo2: foo2, other：other } 捕获 other 所有权，s3 移动
  let once_move = move || {
    let moved = foo2;
    println!("once_move 闭包执行: {:?}, 移动的 other: {:?}", moved, other);
  };

  /*----------------- 多线程闭包 -----------------*/
  // 1 有 move 的线程闭包: 捕获指针 -> 捕获所有权 ----
  let data = vec![1, 2, 3];
  // struct MoveClosure { data: Vec<i32> }  move: 所有权搬入闭包
  let handle = thread::spawn(move || {
    println!("线程中使用 data: {data:?}");
  });
  handle.join().unwrap();
  // println!("{data:?}"); // 编译错误: data 已移入线程

  // 2 无 move 的线程闭包: 只是借用, 但线程存活期间值必须有效 ----
  // scoped 线程保证"闭包返回前不退出", 所以结构体里存引用指针就够了:
  // struct BorrowClosure<'a> { local: &'a String } 只借不拿
  let local = String::from("borrowed");
  thread::scope(|scope| {
    scope.spawn(|| {
      println!("scope 线程借用: {local}");
    });
  });
  println!("scope 结束后原变量仍可用: {local}");
}
