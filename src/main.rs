use demo::{
  enum_01_base, enum_01_option, enum_02_sugar, enum_04_msg,
};

fn main() {
  enum_01_base::demo_wrap_value();
  println!("\n------ enum_01_option: 模仿官方 Option ------\n");
  enum_01_option::demo();

  println!("\n------ enum_02_coin: match / if let / while let / let else / ? ------\n");
  enum_02_sugar::demo();

  println!("\n------ enum_03_state_machine: 状态机 ------\n");

  println!("\n------ enum_04_msg: 消息类型 ------\n");
  enum_04_msg::demo();
}
