pub fn demo() {
  /*----------------- 关联类型 -----------------*/
  struct Counter {
    v: u32,
  }

  trait Iterator {
    type Next;
    fn next(&mut self) -> Self::Next;
  }

  impl Iterator for Counter {
    // impl 实现 trait 时确定
    type Next = u32;
    fn next(&mut self) -> u32 {
      self.v += 1;
      self.v
    }
  }

  let mut counter = Counter { v: 10 };
  counter.next();



  /*----------------- 结构体泛型 -----------------*/
  struct B<T> {
    _foo: T,
  }
  // 创建结构体时确定 _foo 类型
  let _a = B { _foo: 10 };



  /* ----------------- 函数泛型  ----------------- */
  fn no_process<V>(a: V) -> V {
    a
  }
  // 调用时确定类型, 入参 定 返回值
  no_process(10);



  /*----------------- trait 泛型 -----------------*/
  // 要对同一个结构体实现多次 trait ？ 需要 → 泛型  不要 → 关联类型。
  trait Into<T> {
    fn to(&self) -> T;
  }

  struct Person {
    age: i32,
    is_good: bool,
  }

  impl Into<i32> for Person {
    fn to(&self) -> i32 {
      self.age
    }
  }

  impl Into<bool> for Person {
    fn to(&self) -> bool {
      self.is_good
    }
  }

  let person = Person {
    age: 10,
    is_good: true,
  };

  let _age: i32 = person.to();
  let _is_good: bool = person.to();



  /*----------------- 对结构体泛型 实现 trait -----------------*/
  struct Foo<T> {
    v: T,
  }

  trait Setter {
    type Value;
    fn set(&mut self, val: Self::Value);
  }

  impl<T> Setter for Foo<T> {
    type Value = T;
    fn set(&mut self, val: T) {
      self.v = val;
    }
  }
}

#[test]
fn run() {
  demo();
}
