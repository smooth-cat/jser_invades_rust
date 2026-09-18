use std::collections::{LinkedList, VecDeque};
#[test]
fn run() {
  demo()
}
#[allow(unused)]
pub fn demo() {
  /*----------------- 可变长度数组 -----------------*/
  // 不定长数组 (每个元素相同) Vec<i32>， 特性在堆上，内存始终是连续的，通常按 2倍 扩容，不缩容
  // 适合频繁访问的集合
  let mut vector = vec![1, 2, 3];
  let capacity = vector.capacity();

  // 申请内存，扩容为原来的两倍 6，并执行整块内存拷贝 8 EiB 900w TB
  vector.push(4);
  let capacity = vector.capacity();

  // 双端队列 推荐 ✅，适用于频繁访问的集合，场景 任务队列
  let queue = VecDeque::from([1, 2, 3]);
  let queue_cap = queue.capacity();

  // 双向链表 不推荐 ⭕️，虽然插入是 O(1), 但是由于它的内存分布分散，性能会下降
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
}
