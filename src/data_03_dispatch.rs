#[test]
fn run() {
  demo()
}
#[allow(unused)]
pub fn demo() {
  trait Tail {
    fn shake_tail(&self) {
      println!("shake_tail!");
    }
  }
  impl Tail for Cat {}
  impl Tail for Dog {}
  
  /*----------------- 静态分发 消息队列 -----------------*/
  struct Cat {
    name: String,
  };
  struct Dog;
  enum Animal {
    Cat(Cat),
    Dog(Dog),
  }
  
  // 编译时确定： 每项 👉🏻 最大的子类型大小 + 枚举值(u8)  
  let animals = vec![
    Animal::Cat(Cat { name: String::from("梨花") }),
    Animal::Dog(Dog),
  ];

  for animal in animals {
    match animal {
      Animal::Cat(cat) => cat.shake_tail(),
      Animal::Dog(dog) => dog.shake_tail(),
    }
  }
  
  /*----------------- 链表 ~ Box 智能指针 -----------------*/
  // 1. Box 处理链表, 树 这种，不用 Box 会导致 rust 内存无限递归计算
  struct Node {
    value: i32,
    next: Option<Box<Node>>,
  }

  let head = Node {
    value: 1,
    next: Some(Box::new(Node {
      value: 2,
      next: None,
    })),
  };
  
  /*----------------- Box 动态分发 插件、中间件 -----------------*/
  let cat = Cat {
    name: String::from("小白"),
  };
  
  // dyn Tail：
  // 表示实现了 Tail 特征 的 某个结构体
  let smart_ptr: Box<dyn Tail> = Box::new(cat);

  let animals: Vec<Box<dyn Tail>> = vec![
    Box::new(Cat { name: String::from("梨花") }),
    Box::new(Dog),
  ];
  
  for animal in &animals {
    animal.shake_tail();
  }
}
