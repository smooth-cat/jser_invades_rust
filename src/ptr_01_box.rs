// Box 智能指针: 全部示例
// JS:  对象天然在堆上, 变量持引用; Rust 用 Box 显式装箱
// 1. 最简单的 Box 包装 Dog
// 2. Box trait 对象: Dog + Cat 混装 Vec
// 3. Box 单向链表
// 4. Box<dyn Fn> 存闭包 / Box::leak 转 'static / 大数组转移只移指针

use crate::animal::{Animal, Cat, Dog};
use std::{fmt, ptr::NonNull};

pub fn demo() {
  box_01_basic();
  box_02_trait_object();
  box_03_linked_list();
}

// ---- 1 最简单的 Box: 包装一个 Dog 实例 ----
fn box_01_basic() {
  println!("\n---- 1) 最简单的 Box 包装 Dog ----");

  // 直接创建 Box<Dog>: 数据在堆上, 栈上只放一个指针
  // Box 实现了 Drop Trait 释放时自动释放堆上的数据
  let dog = Box::new(Dog {
    name: "旺财".to_string(),
  });
  // 等价于 (*dog).name()
  println!("{} 叫: {}", dog.name(), dog.bark());

  // 把堆上的 dog 拿来了
  let inner: Dog = *dog;
  // 报错 borrow of moved value: `dog`
  // println!("{dog:?}");
  println!("解引用后 name = {}", inner.name);

  // 修改内部数据: 需要可变绑定, 自动解引用可写字段
  let mut dog2 = Box::new(Dog {
    name: "大黄".to_string(),
  });
  dog2.name = "二黄".to_string();
}

// ---- Box<Dog> 和 Box<Cat> 放同一个 Vector: trait 对象 ----
// Rust: 类型不同不能直接混装, 必须装箱成同一个 trait 对象 Box<dyn Animal>
fn box_02_trait_object() {
  println!("\n---- 2) Box trait 对象: Dog + Cat 混装 Vec ----");

  // 如果直接 vec![Dog, Cat, Dog] 会编译报错: 期望 Dog 找到 Cat (类型不一致)
  let animals: Vec<Box<dyn Animal>> = vec![
    Box::new(Dog {
      name: "旺财".to_string(),
    }),
    Box::new(Cat {
      name: "咪咪".to_string(),
    }),
    Box::new(Dog {
      name: "大黄".to_string(),
    }),
  ];

  for animal in &animals {
    println!("{} 叫: {}", animal.name(), animal.bark());
  }

  // Box 拥有所有权, 传参即移动
  for animal in animals {
    speak(animal);
  }
}

// 接收任意实现了 Animal 的装箱对象: 动态分发 (运行期查虚表)
fn speak(animal: Box<dyn Animal>) {
  println!("speak(): {} 叫: {}", animal.name(), animal.bark());
}

// ---- 基于 Box 的单向链表 (struct + Option) ----
// 不使用 box 编译器无法提前算出 函数的栈帧大小
// 使用 box， 栈指针寄存器 RSP (栈顶) 就能在进入函数时正确移动
fn box_03_linked_list() {
  struct Node<T> {
    value: T,
    next: Option<Box<Node<T>>>,
  }

  struct List<T> {
    head: Option<Box<Node<T>>>,
    // NonNull 相当于 *mut Node<T>, 这里可以理解为 Option<usize>
    // tail: 裸指针指向链表的尾节点, 让 push_back 从 O(n) 降到 O(1)
    tail: Option<NonNull<Node<T>>>,
  }

  impl<T> List<T> {
    fn new() -> Self {
      List {
        head: None,
        tail: None,
      }
    }

    // 头插: 旧头挪进新节点的 next, 新节点成为 head
    fn push_head(&mut self, v: T) {
      let head_is_none = self.head.is_none();
      let mut node = Box::new(Node {
        value: v,
        // 生成新 Option<Box<Node<T>>> 并取走 head 中的 Box 值
        next: self.head.take(),
      });
      if head_is_none {
        self.tail = Some(NonNull::from(&mut *node));
      }
      self.head = Some(node);
    }

    // 尾插 O(1): 靠 tail 直接找到尾节点挂新节点, 不用从头遍历
    fn push_tail(&mut self, v: T) {
      let mut node = Box::new(Node {
        value: v,
        next: None,
      });
      // 生成新 尾裸指针
      let new_tail = Some(NonNull::from(node.as_mut()));
      match self.tail {
        // 旧尾存在时 next = 新尾
        Some(mut tail) => unsafe {
          let ptr = tail.as_mut();
          ptr.next = Some(node);
        },
        None => self.head = Some(node),
      }
      // 新尾指针赋给 tail
      self.tail = new_tail;
    }

    fn from_iter(values: impl IntoIterator<Item = T>) -> Self {
      // 先收集再倒序头插, 保证顺序和入参一致 (JS: reduceRight 链式拼接)
      let mut list = List::new();
      for v in values.into_iter() {
        list.push_tail(v);
      }
      list
    }

    fn len(&self) -> usize {
      let mut count = 0;
      let mut cur = &self.head;
      while let Some(node) = cur {
        count += 1;
        cur = &node.next;
      }
      count
    }
  }

  // sum 只对数值有意义, 单独为 List<i32> 实现
  impl List<i32> {
    fn sum(&self) -> i32 {
      let mut total = 0;
      let mut cur = &self.head;
      while let Some(node) = cur {
        total += node.value;
        cur = &node.next;
      }
      total
    }
  }

  impl<T: fmt::Debug> fmt::Debug for List<T> {
    // 打印成 JS 风格: 1 -> 2 -> 3 -> None
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      let mut cur = &self.head;
      while let Some(node) = cur {
        write!(f, "{:?} -> ", node.value)?;
        cur = &node.next;
      }
      write!(f, "None")
    }
  }
  println!("\n---- 3) Box 单向链表 (struct + Option) ----");

  // 如果不加 Box, 编译报错: recursive type `Node` has infinite size
  // 值类型由调用者指定: 这里用 i32
  let list = List::from_iter([1, 2, 3]);
  println!("list = {list:?}");
  println!("len = {}, sum = {}", list.len(), list.sum());

  let empty: List<i32> = List::new();
  println!("empty = {empty:?}");

  // push_back: O(1) 尾插 (tail 直接定位, 无需遍历), 调用者指定新值
  let mut list2 = List::from_iter([1, 2, 3]);
  list2.push_tail(4);
  list2.push_tail(5);
  println!("push_back(4)(5) 后 = {list2:?}");

  // push_front: 头插
  let mut push = List::from_iter([2, 3]);
  push.push_head(1);
  println!("push_front(1) 后 = {push:?}");

  // 泛型: 换成 String 链表, 同一个实现直接复用
  let names = List::from_iter(["旺财".to_string(), "咪咪".to_string()]);
  println!("String 链表 = {names:?}");
}
