#[test]
fn run() {
  demo()
}
#[allow(unused)]
pub fn demo() {
  /*----------------- Box -----------------*/
  // 1. Box 处理链表, 树 这种，不用 Box 会导致 rust 内存无限递归计算，把下一个 node 放 heap 上
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

  /*----------------- Box 动态分发 插件、中间件 -----------------*/
  trait TailAction {
    fn shake_tail(&self) {
      println!("shake_tail!");
    }
  }

  struct Cat;
  impl TailAction for Cat {}

  struct Dog;
  impl TailAction for Dog {}

  let animals: Vec<Box<dyn TailAction>> = vec![Box::new(Cat), Box::new(Dog)];
  for ele in &animals {
    ele.shake_tail();
  }

  /*----------------- 静态分发 消息队列 -----------------*/
  enum Animal {
    Cat(Cat),
    Dog(Dog),
  }

  let animals = vec![Animal::Cat(Cat), Animal::Dog(Dog)];

  for ele in &animals {
    match ele {
      Animal::Cat(cat) => cat.shake_tail(),
      Animal::Dog(dog) => dog.shake_tail(),
    }
  }
}
