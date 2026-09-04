pub fn demo() {
  /*----------------- 计数器 -----------------*/
  // struct 只放数据
  struct Counter {
    count: i32,
  }

  // trait 只放方法
  trait SelfOperate {
    fn inc(&mut self) -> i32;
    fn dec(&mut self) -> i32;
    fn _greet() { println!("hello world") }
  }

  // 为 Counter 实现 SelfOperate
  impl SelfOperate for Counter {
    fn inc(&mut self) -> i32 {
      self.count += 1;
      self.count
    }
    fn dec(&mut self) -> i32 {
      self.count -= 1;
      self.count
    }
  }
  
  let mut c = Counter { count: 0 };
  println!("{}", c.inc() == 1);
  println!("{}", c.inc() == 2);
  println!("{}", c.dec() == 1);
}

#[test]
pub fn run() {
  demo();
}
