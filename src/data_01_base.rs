#[test]
fn run() {
  demo()
}
#[allow(unused)]
pub fn demo() {
  /*----------------- 基础数据类型(Copy 类型， 实现了 Copy 特性) -----------------*/
  // 字符
  let char = 'a';
  let char_temp = char; // 拷贝值

  // number 包含 `i8`, `i16`, `i32`, `i64`, `i128`, `isize` `u8`, `u16`, `u32`, `u64`, `u128`, `usize` `f32`, `f64`
  let bit: u32 = 0b10; // 无符号整数，常用于无负数的 二进制 逻辑运算

  let age = 18i32; // 有符合整数，常用于整数运算，也可用于 带符号的二进制运算(基于补码机制)

  let height = 1.8; // 浮点数

  let index = 1usize; // rust 数组下标必须是 usize 类型

  // boolean
  let flag = true;

  /*----------------- 指针 -----------------*/
  // 指针，也可以叫不可变借用
  let age_ptr = &age;
  // 从地址取值
  let _age = *age_ptr;

  // 裸指针 高性能场景 或 (用于调用 c 库)
  let age_raw_ptr = &age as *const i32;

  // 字符串也属于不可变指针
  let hello = "hello";

  /*----------------- 复合 Copy 类型，整个对象及子对象都实现了 Copy 特性 -----------------*/
  // None 是 Option 类型，薛定谔的类型
  let mut empty: Option<i32> = None;
  empty = Some(1);

  let tuple = ('a', 2, true);
  let arr = [1, 2, 3]; // 栈内存

  /*------------------------ const 编译时常量，类似于 __DEV__, 在运行时会直接被编译成对应值 ------------------------*/
  struct Person {
    age: u32,
  }

  // 整体不可变
  const PERSON: Person = Person {
    // age 不可改变
    age: 10,
  };

  let tmp = PERSON.age; // let tmp = 10;
}
