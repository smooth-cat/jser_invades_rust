use std::hash::{BuildHasher, Hasher};

use hashbrown::HashMap;

/*----------------- 1. 定义你的哈希器 -----------------*/
#[derive(Default)] // 会调用每个内置类型的 default 方法来生成默认值，u64 是 0.0
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

/*----------------- 2. 定义 BuildHasher，负责创建 Hasher 实例 -----------------*/
#[derive(Default, Clone)]
struct MyBuildHasher;

impl BuildHasher for MyBuildHasher {
  type Hasher = MyHasher;

  fn build_hasher(&self) -> Self::Hasher {
    MyHasher::default()
  }
}

#[test]
fn run() {
  demo()
}
pub fn demo() {
  // 演示 5：换一张小 map，演示"清成 EMPTY"的路径
  // 观察点：4 桶表存 a b c 后删除 'a'（桶 1）：
  //         桶 1 左右的连续 FULL 只有 2 个 < Group::WIDTH → 直接清成 EMPTY
  //         且 growth_left +1（这个槽可以重新插入了）
  let mut map = HashMap::<char, i32, MyBuildHasher>::with_hasher(MyBuildHasher {});

  map.insert('a', 7);
  map.debug_dump("insert('a', 7)");

  map.insert('b', 8);
  map.debug_dump("insert('b', 8)");

  map.insert('c', 9);
  map.debug_dump("insert('c', 9)");

  let v = map.get(&'a');

  map.insert('d', 10);
  map.debug_dump("insert('d', 10)");

  map.remove(&'a');

  map.debug_dump("small remove('a') → 周围连续满不足 8, 清成 EMPTY");
}

#[test]
fn test_rehash_in_place() {
  // 演示 6：原地 rehash（8 桶以下的表永远不会触发，这里用 16 桶演示）
  // 塞满 14 个(负载 7/8) → 删 8 个全留墓碑 → items=6, 墓碑=8, growth_left=0
  // 再插入：new_items=7 ≤ 满载容量/2(7) → rehash_in_place 原地清墓碑，桶数不变
  let mut big = HashMap::<char, i32, MyBuildHasher>::with_hasher(MyBuildHasher {});
  for (i, k) in (b'a'..=b'n').enumerate() {
    big.insert(k as char, (i as i32 + 1) * 10);
  }

  big.debug_dump("big 塞满 14 个(16 桶) → growth_left=0");

  for k in b'a'..=b'h' {
    big.remove(&(k as char));
  }

  big.debug_dump("big 删 8 个全留墓碑 → items=6, 墓碑=8, growth_left=0");

  big.insert('a', 10); // new_items=7 ≤ 14/2 → 原地 rehash，桶数不变，墓碑清零

  big.debug_dump("big 再 insert('a') → 原地 rehash 清墓碑, growth_left 恢复");
}
