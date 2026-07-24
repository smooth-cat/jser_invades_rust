fn main() {
  /*----------------- 基础数据类型(Copy 类型， 实现了 Copy 特性) -----------------*/
  // 字符串
  let char = 'a';
  let char_temp = char; // 拷贝值

  let hello = "hello"; // ！！！不可变借用 指针全是 Copy 类型
  // let hello_temp = hello; println!("{hello_temp}");

  // number 包含 `i8`, `i16`, `i32`, `i64`, `i128`, `isize` `u8`, `u16`, `u32`, `u64`, `u128`, `usize` `f32`, `f64`
  let bit = 0b10u32;   // 无符号整数，常用于无负数的 二进制 逻辑运算
  let age = 18i32;     // 有符合整数，常用于整数运算，也可用于 带符号的二进制运算(基于补码机制)
  let height = 1.8f64; // 浮点数
  let index = 1usize;  // rust 数组下标必须是 usize 类型

  // boolean
  let flag = true;

  /*----------------- 复合 Copy 类型，整个对象及子对象都实现了 Copy 特性 -----------------*/
  // None 是 Option 类型，薛定谔的类型
  let mut empty: Option<i32> = None;
  empty = Some(1);

  let tuple = ('a', 2, true);
  let arr = [1, 2, 3]; // 栈内存
  let arr2 = arr;

  /*------------------------ const 编译时常量，类似于 __DEV__, 在运行时会直接被编译成对应值 ------------------------*/
  struct Person {
    age: u32,
  }
  // 整体不可变
  const PERSON: Person = Person {
    // age 不可改变
    age: 10,
  };
  let tmp = PERSON.age;

  println!("
    {char} 
    {char_temp} 
    {hello} 
    {bit} 
    {age} 
    {height} 
    {index} 
    {flag} 
    {empty:?} 
    {tmp}
  ");
}
