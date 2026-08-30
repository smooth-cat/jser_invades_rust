pub fn demo() {
  /*----------------- 计数器 -----------------*/
  /** struct 只能放数据 */
  pub struct Counter {
    count: i32,
  }
  /** trait 定义方法签名 */
  pub trait SelfOperate {
    fn inc(&mut self) -> i32;
    fn dec(&mut self) -> i32;
  }

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
