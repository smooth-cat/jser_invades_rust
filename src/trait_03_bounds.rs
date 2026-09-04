/**
 * 泛型约束、trait 约束
 */
pub fn demo() {
  /*----------------- 泛型约束 -----------------*/
  struct A { value: i32 }
  struct B { value: i32, other: i32 }

  trait Access {
    fn get(&self) -> i32;
  }

  impl Access for A {
    fn get(&self) -> i32 {
      self.value
    }
  }

  impl Access for B {
    fn get(&self) -> i32 {
      self.value
    }
  }
  
  fn get_value<T: Access>(a: T) {
    println!("{}", a.get());
  }

  let a = A { value: 1 };
  let b = B { value: 1, other: 2 };

  get_value(a);
  get_value(b);

  // fn _get_value<T>(a: T)
  // where 
  //   T: Access,
  // {
  //   println!("{}", a.get());
  // }


  /*----------------- trait 约束，类似于 继承 -----------------*/
  // 要实现 Double 先 具有 Access
  trait Double: Access {
    // 不允许父子 trait 内方法重名
    // fn get(&self) -> i32;
    fn double(&self) -> i32 {
      self.get() * 2
    }
  }

  impl Double for A {}
  let x = A { value: 1 };
  println!("{}", x.double());
}

#[test]
fn run() {
  demo();
}
