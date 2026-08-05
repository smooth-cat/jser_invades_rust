mod animal;
mod ptr_01_box;
mod ptr_02_rc;

fn main() {
  let demos: &[(&str, fn())] = &[
    ("1. Box 智能指针", ptr_01_box::demo),
    ("2. Rc / Weak 智能指针", ptr_02_rc::demo),
  ];

  for (title, demo) in demos {
    println!("\n{:=^60}", format!(" {title} "));
    demo();
  }
}
