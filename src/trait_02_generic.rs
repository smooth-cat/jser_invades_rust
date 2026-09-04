pub fn demo() {
  /*----------------- 结构体泛型 -----------------*/
  struct B<T> {
    _foo: T,
  }
  // 创建结构体时确定 _foo 类型
  let _a = B { _foo: 10 };

  /* ----------------- 函数泛型  ----------------- */
  fn no_process<V>(a: V) -> V {
    return a;
  }
  // 调用时确定类型, 入参 定 返回值
  no_process(10.0);

  /*----------------- 关联类型 -----------------*/
  // 1 trait 对多个结构体实现
  // 希望有 next -> u32
  struct Counter { v: u32 }
  // 希望有 next -> f64
  struct Counter2 { v: f64 }

  trait Iterator {
    type N;
    fn next(&mut self) -> Self::N;
  }

  impl Iterator for Counter {
    // impl 实现 trait 时确定
    type N = u32;
    fn next(&mut self) -> u32 {
      self.v += 1;
      self.v
    }
  }
  impl Iterator for Counter2 {
    // impl 实现 trait 时确定
    type N = f64;
    fn next(&mut self) -> f64 {
      self.v += 1.0;
      self.v
    }
  }

  let mut counter = Counter { v: 10 };
  let mut _counter = Counter2 { v: 10.0 };
  counter.next();

  /*----------------- trait 泛型 -----------------*/
  // 类型转换
  // 需要重载 ？ 需要 → 泛型  不要 → 关联类型
  trait Into<T> {
    fn to(&self) -> T;
  }

  // 分数类型
  struct Source {
    // 整数部分
    int_part: i32,
    // 小数部分
    fractional_part: f64,
  }

  impl Into<i32> for Source {
    fn to(&self) -> i32 {
      self.int_part
    }
  }

  impl Into<f64> for Source {
    fn to(&self) -> f64 {
      self.int_part as f64 + self.fractional_part
    }
  }

  let source = Source {
    int_part: 10,
    fractional_part: 0.5,
  };

  // 大概分数
  let _general: i32 = source.to();
  // 准确分数
  let _exact: f64 = source.to();

  /*----------------- 对泛型结构体 实现 trait -----------------*/
  struct Foo<T> {
    value: T, // D
  }

  trait Setter {
    type V;

    fn set(&mut self, data: Self::V);
  }

  impl<D> Setter for Foo<D> {
    type V = D;
    fn set(&mut self, data: D) {
      self.value = data;
    }
  }

  let mut _foo = Foo { value: 10 };
  _foo.set(20);
}

#[test]
fn run() {
  demo();
}
