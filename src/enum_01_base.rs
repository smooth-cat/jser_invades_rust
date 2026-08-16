use std::mem;

/*----------------- 基础用法 类似 switch -----------------*/
pub fn demo_base() {
  enum Drink {
    Coffee,
    Cola,
    Water,
  }
  let drink = Drink::Coffee;

  match drink {
    Drink::Coffee => {
      println!("咖啡");
    }
    Drink::Cola => println!("可乐"),
    Drink::Water => println!("水"),
  }

  // 类似于 default: 匹配剩余类型
  match drink {
    Drink::Coffee => println!("咖啡"),
    _ => println!("其他"),
  }
}

/*----------------- 值包装用法 -----------------*/
pub fn demo_wrap_value() {
  enum Drink2 {
    Coffee(String),
    Cola,
    Water { boiled: bool },
    Soda(String),
  }

  let coffee = Drink2::Coffee(String::from("拿铁"));
  println!("coffee大小: {}", mem::size_of_val(&coffee));

  let cola = Drink2::Cola;
  println!("cola大小: {}", mem::size_of_val(&cola));

  let water = Drink2::Water { boiled: true };
  println!("cola大小: {}", mem::size_of_val(&water));

  match coffee {
    Drink2::Coffee(name) => {
      println!("咖啡: {name}");
    }
    Drink2::Cola => println!("可乐"),
    Drink2::Water { boiled } => println!("水: {boiled}"),
    Drink2::Soda(name) => println!("苏打: {name}"),
  }
}
