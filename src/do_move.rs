use crate::data::Person;
use std::ptr;
pub fn do_move() {
  /*----------------- 所有权转移 -----------------*/
  // person 拥有栈内存的所有权，
  let person = Person {
    name: "John".to_string(),
    age: 30,
  };

  // name 没有 Copy Trait，所以 “=” 表示所有权转移，此时 person 无效
  let person_2 = person;

  // Err: borrow of moved value: `person`value borrowed here after move
  // println!("旧 person {:?}", person);
  println!("person_2 {:?}", person_2);
}

pub fn force_read() {
  let person = Person {
    name: "John".to_string(),
    age: 30,
  };

  // 裸指针
  let person_raw_ptr = &person as *const Person;

  let mut person_2 = person;
  person_2.name = "Tom".to_string();
  person_2.age = 1;

  unsafe {
    // 读取 x 所在栈位置保存的 String 结构（包含 pointer, capacity, len）
    let old_person: Person = ptr::read(person_raw_ptr);

    // 理论上能读出原数据，但千万不要让 Rust 在这里释放它！
    println!("旧内存中的 person: {:?}", old_person);

    // person_2.name = "Tom".to_string(); 已经释放了旧 name 堆内存，所以要 阻止 old_person 触发二次释放导致报错
    // person_2.age 不会导致 二次释放，因为它没有实现 Drop Trait
    std::mem::forget(old_person); 
  }

  println!("person_2 {:?}", person_2);
}
