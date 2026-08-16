pub fn demo() {
  // ===== 7. MyOption：模仿官方 Option，实现 unwrap / unwrap_or / map =====
  #[derive(Debug)]
  enum MyOption<T> {
    Some(T),
    None,
  }

  impl<T> MyOption<T> {
    fn unwrap(self) -> T {
      match self {
        MyOption::Some(v) => v,
        MyOption::None => panic!("called `unwrap` on a `None` value"),
      }
    }

    fn is_none(&self) -> bool {
      match self {
        MyOption::Some(_) => false,
        MyOption::None => true,
      }
    }

    fn map<U>(self, f: impl FnOnce(T) -> U) -> MyOption<U> {
      match self {
        MyOption::Some(v) => MyOption::Some(f(v)),
        MyOption::None => MyOption::None,
      }
    }
  }
  use MyOption::{None, Some};

  let maybe_num = Some(10);
  let num = maybe_num.unwrap();
  println!("num: {}", num);
  // borrow of moved value: `maybe_num`
  // println!("maybe_num: {:?}", maybe_num);

  let maybe_num2 = Some(10);
  println!("map: {:?}", maybe_num2.map(|x| x * 2));

  let b: MyOption<i32> = None; // 虽然是空，但是要告知有值时是什么类型
  println!("is_none: {}", b.is_none());
}
