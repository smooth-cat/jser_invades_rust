use std::ptr;
use crate::data::Person;

pub fn demo() {
  let s1 = String::from("hello");

  println!("s1 内存地址 {:p}", &s1);

  let s2 = do_move(s1);
  
  println!("{}", s2);
}

fn do_move(mut s: String) -> String {
  println!("s 内存地址 {:p}", &s);
  s.push_str(" world!");
  s
}

#[test]
fn run() { can_read() }
pub fn can_read() {
  let person = Person {
    name: String::from("Jerry"),
    age: 30,
  };

  // 裸指针
  let person_ptr = &person as *const Person;

  // 所有权转移
  let mut person_2 = person;

  // mem::drop(person_2.name);
  person_2.name = String::from("Tom");
  person_2.age = 1;

  unsafe {
    // 通过裸指针读出 person
    let old_person: Person = ptr::read(person_ptr);

    // 理论上能读出原数据
    println!("old_person: {:?}", old_person);

    std::mem::forget(old_person); 
  }

  println!("person_2: {:?}", person_2);
}