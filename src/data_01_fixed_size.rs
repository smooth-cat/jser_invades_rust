#[test]
fn run() {
  demo()
}
/*----------------- 固定大小的数据 -----------------*/
#[allow(unused)]
pub fn demo() {
  /*----------------- 基础数据 -----------------*/
  // number 包含 `i8`, `i16`, `i32`, `i64`, `i128`, `isize` `u8`, `u16`, `u32`, `u64`, `u128`, `usize` `f32`, `f64`
  let bit: u32 = 0b10; // 无符号整数，常用于无负数的 二进制 逻辑运算

  let age = 18i32; // 有符合整数，常用于整数运算，也可用于 带符号的二进制运算(基于补码机制)

  let height = 1.8; // 浮点数

  let i = 1usize; // rust 数组下标必须是 usize 类型
  let item = [0, 1][i];

  // boolean
  let flag = true;

  // 字符类型
  let a = 'a';


  /*----------------- 指针 (使用额外变量对数据进行操作) -----------------*/
  struct Person { age: u32 }

  let person = Person { age: 18 };

  // 指针(借用) 常见于复杂数据传参
  let person_ptr = &person;

  fn get_age(ptr: &Person) -> u32 {
    // 自动解引用特性
    let age = ptr.age;
    // let age = (*ptr).age;
    return age;
  }

  let num = get_age(person_ptr);

  // 字符串切片 也属于指针
  let hello = "hello";


  // 裸指针 内存不安全 高性能场景 或 (用于调用 c 库)
  // let age_raw_ptr = &age as *const i32;
  /*----------------- 固定长度集合 -----------------*/
  let tuple = ('a', 2, true);
  let arr = [1, 2, 3];


  /*----------------- 判空 -----------------*/
  // use std::option::Option::{None,Some};
  // None 是 Option 类型，用于判空
  let mut maybe = None;
  maybe = Some(1);
  match maybe {
    None => println!("没数据"),
    Some(v) => println!("数据是:{v}"),
  }


  /*------------------------ const 编译时常量，类似于 __DEV__, 在运行时会直接被编译成对应值 ------------------------*/
  // 整体不可变
  const PERSON: Person = Person {
    // age 不可改变
    age: 10,
  };

  let temp = PERSON.age;
  // let temp = 10;
}
