/*----------------- owner 的读写 与 场上存在的读写指针互斥 -----------------*/
pub fn no_read_when_mut_borrow() {
  let mut num = 10;
  let mut ptr = &mut num;
  // let num2 = num; // 可变借用时 owner 不可读
  *ptr = 30;         // ptr 存活到最后一次使用
}
pub fn no_write_when_borrow() {
  let mut num = 10;
  let ptr = &num;
  // num = 20;         // 借用时 owner 无法修改
  println!("{}", ptr); // ptr 存活到最后一次使用
}

/*----------------- 创建借用时存在读写互斥 -----------------*/
pub fn no_borrow_when_mut_borrowed() { 
  let mut num = 10;
  let mut write = &mut num;
  // let read = &num;    // 已经有写指针 没办法再创建读指针
  println!("{}", write)
}
pub fn no_mut_borrow_when_borrowed() { 
  let mut num = 10;
  let read = &num; 
  // let mut write = &mut num; // 已经有读指针 没办法再创建写指针
  println!("{}", read)
}