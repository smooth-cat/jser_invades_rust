use std::hash::{BuildHasher, Hasher};

use hashbrown::HashMap;

// 1. 定义你的哈希器
#[derive(Default)]
struct MyHasher {
  state: u64,
}

impl Hasher for MyHasher {
  fn write_u32(&mut self, i: u32) {
    // 'a' => 97 ，'z' => 122
    let mut code = match i {
      // 遵循左闭右开原则
      // 'a' ~ 'z' 映射为 1 ~ 26
      97..123 => i as u64 - 97 + 1,
      // 其他值映射为 27
      _ => 27,
    };
    // 模拟 'a', 'b' hash 出现 hash 碰撞的情况
    if code == 2 {
      code = 1;
    }
    // 让高 7 位 和低 57 位都是 code 值
    let code64 = (code << 57) + code;
    self.state = code64;
  }

  fn finish(&self) -> u64 {
    self.state
  }
  // 强制实现，但我们会直接用 write_u64 重写的代码来达到为 state 赋值
  fn write(&mut self, _: &[u8]) {}
}

// 2. 定义 BuildHasher，负责创建 Hasher 实例
#[derive(Default, Clone)]
struct MyBuildHasher;

impl BuildHasher for MyBuildHasher {
  type Hasher = MyHasher;

  fn build_hasher(&self) -> Self::Hasher {
    MyHasher::default()
  }
}

#[test]
fn do_it() {
  hash_test()
}
pub fn hash_test() {
  let mut map = HashMap::<char, i32, MyBuildHasher>::with_hasher(MyBuildHasher {});
  map.insert('a', 10);
  map.insert('b', 20);
  map.insert('c', 30);
  let va = map.get(&'a');
  let vb = map.get(&'b');
  println!("a: {:?}, b: {:?}", va, vb);
}
