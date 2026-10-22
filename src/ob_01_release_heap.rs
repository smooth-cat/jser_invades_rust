use std::mem;
use crate::data::Person;

#[allow(unused)]
pub fn demo() {
  let a = Person {
    name: String::from("米龙"),
    age: 18
  };

  let ptr = &a;

  // 根据可用范围生成释放堆内存代码
  mem::drop(a);
}