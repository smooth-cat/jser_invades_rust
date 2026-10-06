use std::{collections::{LinkedList, VecDeque}, ops::Deref};
#[test]
fn run() {
  demo()
}
#[allow(unused)]
pub fn demo() {
  /*----------------- 可变长度数组 -----------------*/
  // 只在尾部增删，但经常按索引访问、遍历、排序、切片
  /*
    1. Box::new_uninit                  申请了 3 个 i32 大小的堆内存
    2. write_box_via_move               将 [10, 20, 30] 一次性写入堆内存中
    3. box_assume_init_into_vec_unsafe  把 Box 转换为 Vec
  */
  let mut arr = vec![10, 20, 30];

  // 申请内存，扩容为原来的两倍 6，并执行整块内存拷贝 8 EiB 900w TB
  arr.push(40);

  // 双端队列 推荐 ✅，解决 Vec 头部插入性能问题
  // 场景: 任务(消息)队列
  let queue = VecDeque::from([10, 20, 30]);
  
  // 双向链表 不推荐 ⭕️，虽然插入是 O(1), 但是由于它的内存分布分散，性能会下降
  // 适用于频繁变动的集合 例如 LRC 缓存
  let linked_queue = LinkedList::from([1, 2, 3]);
  

  /*----------------- 字符串 -----------------*/
  let mut hello: String = String::from("hello");

  // 只读指针，字符串切片 等价于 (&hello).deref();
  let slice: &str = &hello;

  // 写指针，指向 栈上的 24B String
  let string_ref: &mut String = &mut hello;

  // 优先使用只读指针看看能不能完成函数体编写
  fn get_joined(hello: &str) -> String {
    let new_string = String::from(hello);
    new_string + " world"
  }

  fn join(hello: &mut String) {
    hello.push_str(" world")
  }

}

/* 
  环绕函数，把堆块外的索引映射回内部
            +cap
          → → → → →
          ↑       ↓ 
-cap ... -1 [0 ... cap)   ... 2cap
             ↑       ↓
             ← ← ← ← ←
               -cap
*/
#[allow(unused)]
fn surround(i: i32, cap: i32) -> i32 {
  // [-cap, 0)
  if i < 0 { return i + cap }

  // [0, cap) 直接存
  if i < cap { return i }
  
  // [cap, 2cap)
  i - cap
}

#[test]
fn run_surround() {
  // 5 
  println!("-1 表示第 {} 个槽位", surround(-1, 6));
  // 0 
  println!(" 6 表示第 {} 个槽位", surround(6, 6));
}