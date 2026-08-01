use crate::data::WriteWrap;

pub fn demo() {
  let mut a = "hello".to_string();

  let mut write = &mut a; // 写指针

  let warp = WriteWrap { write };

  let warp_read = &warp; // 读指针

  // 读写权限都是深度的

  // warp_read 只读，没有所有权转移的能力
  // let target = warp_read.write; 

  // 读指针指向的 struct 包含的写指针 不可写
  // warp_read.write.push('a');

  println!("a: {:?}", write);
}