use std::collections::{BTreeSet, HashSet, LinkedList, VecDeque};

use hashbrown::HashMap;

fn main() {
  /*----------------- 数组 -----------------*/
  // 定长数组 (每个元素相同)
  let arr = [1, 2, 3];

  // 不定长数组 (每个元素相同) Vec<i32>， 特性在堆上，内存始终是连续的，通常按 2倍 扩容，不缩容
  // 适合频繁访问的集合
  let mutable_len = vec![1, 2, 3];
  let mutable_len_cap = mutable_len.capacity();

  // 双端队列 推荐✅，适用于频繁访问的集合，场景 任务队列
  let queue = VecDeque::from([1, 2, 3]);
  let queue_cap = queue.capacity();

  // 双向链表 不推荐⭕️，虽然插入是 O(1), 但是由于它的内存分布分散，性能会下降
  // 适用于频繁变动的集合 例如 LRC 缓存
  let linked_queue = LinkedList::from([1, 2, 3]);

  /*----------------- 可变字符串 -----------------*/
  // 可变字符串
  let mut owned: String = String::from("hello"); 
  
  // 写指针，指向 栈上的 24B String
  let string_ref: &mut String = &mut owned;
  string_ref.push_str(" world!");

  // 只读指针，直接指向堆上数据， 等价于 &*owned，常用
  let slice: &str = &owned;
  
  // 直接指向 .rodata
  let literal: &'static str = "hello"; 
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
  #[derive(Debug, Hash, PartialEq, Eq)]
  struct Data {
    value: i32,
  }
  impl Data {
    fn new(value: i32) -> Self {
      Self { value }
    }
  }
  let hash_set = HashSet::from([Data::new(1), Data::new(2), Data::new(3)]);

  /*
   indexmap 第三方库实现 与 JS Set 类似功能
   按插入时间 存储
   它也需要 Hash，Eq、PartialEq Trait
  */
  use indexmap::IndexSet;
  let index_set = IndexSet::from([1, 2, 3]);

  /*
    需要排序、范围查找、获取最值，场景，性能 NLogN
    按元素升序存储 需要实现 Ord Trait
    适用于实时排行、订单系统、时间序列日志与监控报警系统
  */
  let b_tree_set = BTreeSet::from([1, 2, 3]);

  /*----------------- 对象 -----------------*/
  struct Person {
    name: String,
    age: i32,
  }
  let person = Person {
    name: "zhangsan".to_string(),
    age: 18,
  };

  /*----------------- Map -----------------*/
  // HashMap 的 key 需要实现 Hash，Eq、PartialEq Trait。 value 不需要
  let mut hash_map = HashMap::<char, i32>::new();
  hash_map.insert('a', 7);
  hash_map.insert('b', 8);
  hash_map.insert('c', 9);
  hash_map.insert('d', 10);
  hash_map.insert('e', 11);

  hash_map.get(&'a');
  
  hash_map.remove(&'a');

  /*----------------- Box -----------------*/
  // 1. Box 处理链表, 树 这种，不用 Box 会导致 rust 内存无限递归计算，把下一个 node 放 heap 上
  #[derive(Debug)]
  struct Node {
    value: i32,
    next: Option<Box<Node>>,
  }
  let nodes = Node {
    value: 1,
    next: Some(Box::new(Node {
      value: 2,
      next: None,
    })),
  };

  // 2. Box 动态分发
  trait TailAction: std::fmt::Debug {
    fn shake_tail(&self) {
      println!("shake_tail!");
    }
  }
  #[derive(Debug)]
  struct Cat;
  impl TailAction for Cat {}
  #[derive(Debug)]
  struct Dog;
  impl TailAction for Dog {}
  let animals: Vec<Box<dyn TailAction>> = vec![Box::new(Cat), Box::new(Dog)];
  for ele in &animals {
    ele.shake_tail();
  }

  /*----------------- &mut X  可变借用, 没有实现 Copy，“=” 变成了 “移动” 而不是 “复制” -----------------*/
  let mut num = 1;
  let mut num_ref = &mut num;
  let mut num_ref_2 = num_ref;

  println!("arr: {:?}", arr);
  println!(
    "mutable_len: {:?} 初始化容器大小：{}",
    mutable_len, mutable_len_cap
  );
  println!("queue: {:?} 初始化容器大小：{}", queue, queue_cap);
  println!("train: {:?}", linked_queue);
  println!("hash_set: {:?}", hash_set);
  println!("index_set: {:?}", index_set);
  println!("b_tree_set: {:?}", b_tree_set);
  println!("person: name={:?}, age={}", person.name, person.age);
  println!("hash_map: {:?}", hash_map);
  println!("nodes: {:?}", nodes);
  println!("animals: {:?}", animals);
  // 报错 因为 num_ref 持有的 num 的 指针被移动给了 num_ref_2
  // println!("num: {:p}", num_ref); 
  println!("num: {:p}", num_ref_2);
}
