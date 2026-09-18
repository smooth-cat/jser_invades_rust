use std::collections::{BTreeSet, HashSet};

use hashbrown::HashMap;
#[test]
fn run() {
  demo()
}
#[allow(unused)]
pub fn demo() {
  /*----------------- Set -----------------*/
  /*
    HashSet, 数据无序
    ！！！ 它 内的元素必须实现 Hash，Eq、PartialEq Trait
    你可以通过派生宏 #[derive(Hash, PartialEq, Eq)] 使用官方的默认实现
    // 展开后相当于：
    impl Hash for Xxx {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.x.hash(state); // 先哈希第一个字段
            self.y.hash(state); // 再哈希第二个字段
        }
    }
    impl PartialEq for Xxx {
        fn eq(&self, other: &Self) -> bool {
            self.x == other.x && self.y == other.y
        }
    }
    // Eq Rust 源码中默认是 PartialEq 的实现 (pub trait Eq: PartialEq {})
    impl Eq for Xxx {}
  */
  #[derive(Hash, PartialEq, Eq)]
  struct Data {
    value: i32,
  }
  impl Data {
    fn new(value: i32) -> Self {
      Self { value }
    }
  }
  let _hash_set = HashSet::from([Data::new(1), Data::new(2), Data::new(3)]);

  /*
   indexmap 第三方库实现 与 JS Set 类似功能
   按插入时间 存储
   它也需要 Hash，Eq、PartialEq Trait
  */
  use indexmap::IndexSet;
  let index_set = IndexSet::from([1, 2, 3]);
  for (i, item) in index_set.iter().enumerate() {
    println!("第{i}项是：{item}");
  }

  /*
    需要排序、范围查找、获取最值，场景，性能 NLogN
    按元素升序存储 需要实现 Ord Trait
    适用于实时排行、订单系统、时间序列日志与监控报警系统
  */
  let b_tree_set = BTreeSet::from([1, 2, 3]);

  /*----------------- Map -----------------*/
  // HashMap 的 key 需要实现 Hash，Eq、PartialEq Trait。 value 不需要
  let mut hash_map = HashMap::<char, i32>::new();
  hash_map.insert('a', 7);
  hash_map.get(&'a');
  hash_map.remove(&'a');

  // 可遍历的字典，按插入序遍历
  use indexmap::IndexMap;
  let index_map = IndexMap::from([('a', 7), ('b', 8)]);
  for (key, value) in &index_map {
    println!("{key}的值是：{value}");
  }
}
