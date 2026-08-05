// Rc / Weak 智能指针: 全部示例
// JS: 对象天生共享引用 + GC 计数; Rust 需要显式 Rc::clone 才能共享同一份数据
// 1. Rc 基础: 多变量共享 + Rc::clone 是浅拷贝(只 +1 计数)
// 2. strong_count 与 drop: 计数归零才释放
// 3. Weak: downgrade + upgrade, 数据释放后返回 None
// 4. 循环引用: 两个 Rc 互指 → 强计数永不归零 → 内存泄漏
// 5. 树形结构: 父 Rc 指子, 子 Weak 回指父 (打破循环)
// 6. Rc<RefCell<T>> 组合拳: 共享可变数据 (JS 对象习惯)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

pub fn demo() {
  rc_01_basic();
  rc_02_counts();
  rc_03_weak();
  rc_04_cycle_leak();
  rc_05_no_leak();
  rc_06_shared_mut();
}

// ---- 1 Rc 基础: 多个变量共享同一份数据 ----
fn rc_01_basic() {
  println!("\n---- 1) Rc 基础: 共享同一份数据 ----");

  // Rc::new 把数据放到堆上， 它是只读的
  let rc1 = Rc::new("共享的数据".to_string());
  // Rc::clone 只复制指针 + 强计数 +1, 不是深拷贝 (坑 7)
  let rc2 = Rc::clone(&rc1);
  let rc3 = rc1.clone(); // 等价写法

  println!("rc1 = {rc1}");
  println!("rc2 = {rc2}, rc3 = {rc3} (三个 Rc 指向同一块堆内存)");
  println!("强计数 = {}", Rc::strong_count(&rc1));

  // 对比深拷贝: String::clone 会真正复制数据, 两者独立
  let s1 = "hello".to_string();
  let s2 = s1.clone();
  println!("s1 = {s1}, s2 = {s2} (String::clone 是深拷贝, 数据独立)");

  // Rc<T> 只读共享: 内部数据不可变
  // rc1.push_str("x"); // 编译报错: cannot borrow data as mutable
  println!("Rc 只能读, 要改数据需配合 RefCell (见 6)");
}

// ---- 2 strong_count 与 drop: 计数归零才释放 ----
fn rc_02_counts() {
  println!("\n---- 2) strong_count 与 drop ----");

  let shared = Rc::new(vec![1, 2, 3]);
  println!("初始: 强计数 = {}", Rc::strong_count(&shared));

  {
    let _a = Rc::clone(&shared);
    let _b = Rc::clone(&shared);
    println!("克隆两个后: 强计数 = {}", Rc::strong_count(&shared));
  }
  // _a / _b 离开作用域自动 drop, 计数 -1
  println!("离开作用域后: 强计数 = {}", Rc::strong_count(&shared));

  // 最后一个 Rc 也 drop, 计数归零 → 堆数据被释放
  drop(shared);
}

// ---- 3 Weak: 弱引用不阻止释放 ----
fn rc_03_weak() {
  println!("\n---- 3) Weak: 弱引用, 不增加强计数 ----");

  let rc = Rc::new("data".to_string());
  // downgrade 得到 Weak<T>: 不增加强计数, 不阻止释放
  let weak = Rc::downgrade(&rc);
  println!(
    "强计数 = {}, 弱计数 = {}",
    Rc::strong_count(&rc),
    Rc::weak_count(&rc)
  );

  // upgrade() 尝试升级为强引用, 数据还在返回 Some(Rc<T>)
  match weak.upgrade() {
    Some(rc) => println!("upgrade() = Some({rc})"),
    None => println!("upgrade() = None"),
  }

  // drop 掉最后一个强引用 → 数据释放, 但 Weak 还在
  drop(rc);
  match weak.upgrade() {
    Some(rc) => println!("upgrade() = Some({rc})"),
    None => println!("upgrade() = None (数据已释放)"),
  }

  // 坑 5: None 直接 unwrap 会 panic, 一定要先判断
  // weak.upgrade().unwrap(); // panic: called `Option::unwrap()` on a `None` value
}

// ---- 4 循环引用: 两个 Rc 互指 → 内存泄漏 (坑 3) ----
fn rc_04_cycle_leak() {
  println!("\n---- 4) 循环引用: Rc 互指 → 内存泄漏 ----");

  // 图/双向链表里节点互相指向对方, 需要 RefCell 提供内部可变性
  struct Node {
    age: i32,
    next: RefCell<Option<Rc<Node>>>,
  }

  let a = Rc::new(Node {
    age: 18,
    next: RefCell::new(None),
  });
  let b = Rc::new(Node {
    age: 20,
    next: RefCell::new(None),
  });

  // a -> b, b -> a 形成环
  *a.next.borrow_mut() = Some(Rc::clone(&b));
  *b.next.borrow_mut() = Some(Rc::clone(&a));

  let b_from_a = a.next.borrow_mut();
  // cannot assign to data in an `Rc`
  // (*b_from_a).unwrap().age = 21;

  println!("a.next 指向的年龄: {}", b_from_a.as_ref().unwrap().age);
  println!(
    "a 强计数 = {}, b 强计数 = {}",
    Rc::strong_count(&a),
    Rc::strong_count(&b)
  );
  // 栈上的 Rc 指针释放了，堆上的指针无法释放
  println!("a/b 互指, 计数永不归零, 内存泄漏!");
}

// ---- 5 可变双向链表: 用 Weak 打破循环引用 ----
fn rc_05_no_leak() {
  println!("\n---- 5) 可变双向链表: Weak 打破循环引用 ----");

  // 节点定义
  #[derive(Debug)]
  struct Node {
    data: i32,
    // 下一个节点：我拥有它（强引用）
    next: Option<Rc<RefCell<Node>>>,
    // 上一个节点：我不拥有它（弱引用），防止循环
    prev: Option<Weak<RefCell<Node>>>,
  }

  impl Node {
    fn new(data: i32) -> Rc<RefCell<Node>> {
      Rc::new(RefCell::new(Node {
        data,
        next: None,
        prev: None,
      }))
    }
  }

  // 1. 创建三个节点（每个节点被其变量强引用，refcount = 1）
  let node1 = Node::new(1);
  let node2 = Node::new(2);
  let node3 = Node::new(3);

  // RefCell 包裹整个 Node，所以数据和链接都可修改
  node2.borrow_mut().data = 20;
  println!("node2.data 修改为: {}", node2.borrow().data);

  // 2. 建立链接：1 -> 2 -> 3，同时建立反向弱链接
  // 设置 node1.next = Some(node2)
  node1.borrow_mut().next = Some(Rc::clone(&node2));
  // 设置 node2.prev = Some(Rc::downgrade(&node1))（弱引用）
  node2.borrow_mut().prev = Some(Rc::downgrade(&node1));

  // 设置 node2.next = Some(node3)
  node2.borrow_mut().next = Some(Rc::clone(&node3));
  // 设置 node3.prev = Some(Rc::downgrade(&node2))
  node3.borrow_mut().prev = Some(Rc::downgrade(&node2));

  // 3. 通过强引用正向遍历（从 node1 到 node3）
  let mut current = Some(Rc::clone(&node1));
  while let Some(node_rc) = current {
    let (data, next) = {
      let node = node_rc.borrow();
      (node.data, node.next.as_ref().map(Rc::clone))
    }; // 离开块后释放 Ref<Node>，再进入下一轮借用

    println!("正向: {data}");
    current = next;
  }
  // 输出: 1, 20, 3

  // 4. 通过弱引用反向遍历（从 node3 回到 node1）
  let mut current = Some(Rc::clone(&node3));
  while let Some(node_rc) = current {
    let (data, prev) = {
      let node = node_rc.borrow();
      (node.data, node.prev.as_ref().and_then(Weak::upgrade))
    };

    println!("反向: {data}");
    current = prev;
  }
  // 输出: 3, 20, 1

  // 5. 证明弱引用不会阻止内存释放
  // 此时 node1, node2, node3 仍在作用域，强引用计数为：
  // node1: 自身变量(+1) + node2.prev(弱引用不影响) = 1
  // node2: 自身变量(+1) + node1.next(强+1) = 2
  // node3: 自身变量(+1) + node2.next(强+1) = 2
  // 但是如果我们 drop 掉 node1 和 node2，会发生什么？
  println!("\n=== 释放 node1 和 node2 ===");
  drop(node1);
  drop(node2);
  let node3_ref = node3.borrow();
  let node2_weak = node3_ref.prev.as_ref().unwrap();


  println!("node2 api 弱引用计数 {}", node2_weak.weak_count());
  let (raw_strong, raw_weak) = unsafe { raw_rc_counts(node2_weak) };
  println!("node2 内存中存储的  强计数：{} 弱计数：{}", raw_strong, raw_weak);

  // 此时 node3 还存在，所以：
  // node3 的强引用计数 = 自身变量(1) + node2.next 已不存在（node2被drop） = 1
  // node2 的强引用计数 = node1.next 已不存在 + node3.prev(弱引用) = 0 → 内存释放！
  // node1 的强引用计数 = node2.prev(弱引用) = 0 → 内存释放！
  // 注意：node3.prev 是弱引用，指向 node2，但 node2 已经释放，所以 upgrade 会返回 None
  let previous = {
    node3_ref.prev.as_ref().and_then(Weak::upgrade)
  };
  match previous {
    Some(_) => println!("错误：node2 竟然还活着"),
    None => println!("正确：node2 已被释放，弱引用失效"),
  }
}

unsafe fn raw_rc_counts<T>(weak: &Weak<T>) -> (usize, usize) {
  // 当前 RcInner<T>：
  // strong: Cell<usize>
  // weak:   Cell<usize>
  // value:  T
  let header_size = 2 * size_of::<usize>();
  let value_align = align_of::<T>();
  let value_offset = (header_size + value_align - 1) & !(value_align - 1);

  unsafe {
    // Weak::as_ptr() 指向 value；退回到 RcInner 的开头。
    let header = weak.as_ptr().cast::<u8>().sub(value_offset);

    let strong = (&*header.cast::<Cell<usize>>()).get();
    let weak = (&*header
      .add(size_of::<usize>())
      .cast::<Cell<usize>>())
    .get();

    (strong, weak)
  }
}

// ---- 6 Rc<RefCell<T>> 组合拳: 共享可变数据 ----
fn rc_06_shared_mut() {
  println!("\n---- 6) Rc<RefCell<T>>: 共享可变数据 ----");
  // JS 对象默认共享可写; Rust 最接近的写法: Rc(共享) + RefCell(可变)
  let shared = Rc::new(RefCell::new(42));
  let alias = Rc::clone(&shared);

  println!("初始值 = {}", shared.borrow());
  // 通过 alias 修改, shared 能看到 (同一份数据)
  *alias.borrow_mut() += 1;
  println!("alias 修改后 = {}", shared.borrow());
  println!("强计数 = {}", Rc::strong_count(&shared));

  // 坑 4: borrow 的 Ref 借用要释放后才能 borrow_mut
  {
    let _r = shared.borrow();
    println!("borrow 借用中: {}", *_r);
  } // _r 离开作用域, 借用释放
  *shared.borrow_mut() = 100;
  println!("重新赋值后 = {}", shared.borrow());
}
