pub fn demo() {


  enum Drink {
    Coffee(String),
    Cola,
    Water { boiled: bool },
    Soda(String),
  }

  let coffee = Drink::Coffee(String::from("拿铁"));

  /*----------------- if let -------- 类比 ts if(xxx === Drink.Coffee) {} -----------------*/
  if let Drink::Coffee(name) = &coffee {
    println!("是 {name} 咖啡！");
  }

  /*----------------- while let ----- 类比 ts while(xxx === Drink.Cola) {} -----------------*/
  let mut stack = vec![Drink::Cola, Drink::Cola];
  let mut count = 0;
  while let Some(Drink::Cola) = stack.pop() {
    count += 1;
  }
  println!("弹出了 {count} 瓶 可乐");

  /*----------------- let else ---- 类比 js if(xxx == null) return -----------------*/
  fn find_water(c: &Drink) -> Option<bool> {
    let Drink::Water { boiled } = c else {
      return None;
    };
    Some(*boiled)
  }
  println!("find_water: {:?}", find_water(&coffee));

  let water = Drink::Water { boiled: false };
  println!("find_water: {:?}", find_water(&water));

  /*----------------- Option / Result + ? ：None 自动提前返回 -----------------*/
  fn half(x: Option<u32>) -> Option<u32> {
    // x? 如果有值，解构出 u32，继续执行
    //    如果无值，函数直接 返回 None
    let num = x? / 2;
    Some(num)
  }
  println!(
    "half(Some(10)) = {:?}, unwrap_or: {}",
    half(Some(10)),
    half(Some(10)).unwrap_or(0)
  );

  println!("half(None) = {:?}", half(None));
}
