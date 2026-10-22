#[allow(unused)]
pub mod ignore {
  /*----------------- owner 的读写 与 指针互斥 -----------------*/
  pub fn owner_conflict_r_w() {
    let mut num = 10;
    let mut ptr = &mut num;
    // let num2 = num; // 有写指针时 owner 不可读
    *ptr = 30;         // ptr 存活到最后一次使用
  }

  pub fn owner_write_only() {
    let mut num = 10;
    let mut ptr = &mut num;
    // num = 20;         // 有写指针 owner 无法写
    println!("{}", ptr); // ptr 存活到最后一次使用
  }
  
  /*----------------- 指针读写互斥 -----------------*/
  pub fn borrow_conflict_r_w() { 
    let mut num = 10;
    let mut write = &mut num;
    // let read = &num;    // 已经有写指针 没办法再创建读指针
    println!("{}", write)
  }

  pub fn borrow_write_only() { 
    let mut num = 10;
    let mut write1 = &mut num; 
    // let mut write2 = &mut num; // 写唯一
    println!("{}", write1)
  }
}