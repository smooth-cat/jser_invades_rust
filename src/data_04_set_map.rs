use std::collections::{BTreeSet, HashSet};
use indexmap::{IndexSet,IndexMap};
use hashbrown::HashMap;
#[test]
fn run() {
  demo()
}
#[allow(unused)]
pub fn demo() {
  /*----------------- set -----------------*/
  // 不可遍历
  let mut hash_set = HashSet::from([1, 2, 3]);
  hash_set.get(&4);
  hash_set.insert(4);
  hash_set.remove(&4);

  // indexmap 库，按插入序遍历
  let index_set = IndexSet::from([1, 2, 3]);
  for (i, item) in index_set.iter().enumerate() {
    println!("第{i}项是：{item}");
  }

  /*
    按元素升序存储 需要实现 Ord Trait
    适用于实时排行、订单系统、时间序列日志与监控报警系统
  */
  let b_tree_set = BTreeSet::from([1, 2, 3]);

  /*----------------- Map -----------------*/
  // 不可遍历
  let mut hash_map = HashMap::<char, i32>::new();
  hash_map.get(&'a');
  hash_map.insert('a', 7);
  hash_map.remove(&'a');

  // 按插入序遍历
  let index_map = IndexMap::from([('a', 7), ('b', 8)]);
  for (key, value) in &index_map {
    println!("{key}的值是：{value}");
  }


  /*----------------- 自定义结构体作为 Key -----------------*/
  /*
    宏展开:
    impl Hash for Data {
      fn hash<H: Hasher>(&self, state: &mut H) {
        // 挨个值做 hash 计算
        self.v.hash(state); 
      }
    }

    impl PartialEq for Data {
      fn eq(&self, other: &Self) -> bool {
        // 挨个值做比较
        self.v == other.v
      }
    }

    // 仅作为编译时标记
    impl Eq for Data {}
  */
  #[derive(Hash, PartialEq, Eq)]
  struct Data {
    v: i32,
  }

  let hash_set = HashSet::from([Data{v: 1}, Data{v: 2}, Data{v: 3}]);
}
