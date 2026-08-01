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
