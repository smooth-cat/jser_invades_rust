use ::core::marker::Copy;
fn demo() {
  let num = 5;
  num.clone();  // i32 实现了 Copy 、Clone
  let name = String::from("hello");
  name.clone(); // String 实现了 Clone
}