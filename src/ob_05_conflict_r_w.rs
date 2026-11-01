#[allow(unused)]
pub mod ignore {
  /*----------------- 所有者 与 指针关系 -----------------*/
  pub fn owner_conflict_r_w() {
    let mut owner = 10;
    let mut write = &mut owner;
    // let data = owner; // 有写指针时 owner 不可读
    *write = 30;         // ptr 存活到最后一次使用
  }

  pub fn owner_write_only() {
    let mut owner = 10;
    let mut write = &mut owner;
    // owner = 20;         // 有写指针 owner 无法写
    println!("{}", write)  // ptr 存活到最后一次使用
  }
  
  /*----------------- 指针之间的关系 -----------------*/
  pub fn borrow_conflict_r_w() { 
    let mut num = 10;
    let read = &num;    // 已经有写指针 没办法再创建读指针
    // let mut write = &mut num;
    println!("{}", read)
  }

  pub fn borrow_write_only() { 
    let mut num = 10;
    let mut write1 = &mut num; 
    // let mut write2 = &mut num; // 写唯一
    println!("{}", write1)
  }
}