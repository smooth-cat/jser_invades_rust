/**
 * 裸写
 * 1. 英雄实现 hp、attack 基础方法
 * 2. 辅助实现 heal 治疗
 * 3. 刺客实现 can_kill、kill 方法
 */
pub fn direct() {
  // 共享行为(接口)：所有"英雄"都能做的事， trait 不能定义任何数据
  trait Hero {
    fn get_hp(&self) -> i32;
    fn set_hp(&mut self, hp: i32);
    // 默认方法 = JS 基类里写好的 attack
    // fn attack(&mut self, target: &mut impl Hero) {
    fn attack(&mut self, target: &mut dyn Hero) {
      target.set_hp(target.get_hp() - 1);
    }
  }

  macro_rules! impl_hero {
    ($struct_name:ident) => {
      impl Hero for $struct_name {
        fn get_hp(&self) -> i32 {
          self.hp
        }
        fn set_hp(&mut self, hp: i32) {
          self.hp = hp;
        }
      }
    };
  }

  /** 辅助 */
  struct Support {
    hp: i32,
  }
  impl_hero!(Support);
  /** 刺客 */
  struct Assassin {
    hp: i32,
    can_kill: bool,
  }
  impl_hero!(Assassin);

  // 各自的专属能力
  impl Support {
    fn heal(&mut self) {
      self.set_hp((self.get_hp() + 5).min(100));
    }
  }
  impl Assassin {
    fn kill(&self, target: &mut impl Hero) {
      if self.can_kill {
        target.set_hp(0);
      }
    }
  }
  let mut support = Support { hp: 100 };
  let mut assassin = Assassin {
    hp: 100,
    can_kill: false,
  };
  println!("----------------- 静态分发 -----------------");
  assassin.attack(&mut support);
  println!("{}", support.hp); // 99

  support.heal();
  println!("{}", support.hp); // 100

  assassin.can_kill = true;
  assassin.kill(&mut support);
  println!("{}", support.hp); // 0

  println!(
    "----------------- 基于 trait 动态分发，根据运行时类型查表执行对应方法 -----------------"
  );
  let mut normal_list: Vec<Box<dyn Hero>> = vec![
    Box::new(Support { hp: 100 }),
    Box::new(Assassin {
      hp: 100,
      can_kill: false,
    }),
  ];
  for (i, item) in normal_list.iter_mut().enumerate() {
    let hp = item.get_hp();
    println!("第 {i} hp 是：{hp}");
  }

  println!("----------------- 基于 Enum 静态分发 -----------------");
  enum HeroDispatch {
    Support(Support),
    Assassin(Assassin),
  }

  macro_rules! dispatch_hero {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            HeroDispatch::Support(hero) => hero.$method($($arg),*),
            HeroDispatch::Assassin(hero) => hero.$method($($arg),*),
            // 如果以后新增 Tank，只需在这里加一行：
            // HeroDispatch::Tank(hero) => hero.$method($($arg),*),
        }
    };
}

  impl Hero for HeroDispatch {
    fn get_hp(&self) -> i32 {
      dispatch_hero!(self, get_hp)
    }
    fn set_hp(&mut self, hp: i32) {
      dispatch_hero!(self, set_hp, hp)
    }
  }

  let mut hero_list = vec![
    HeroDispatch::Support(Support { hp: 100 }),
    HeroDispatch::Assassin(Assassin {
      hp: 100,
      can_kill: false,
    }),
  ];
  for (i, one) in hero_list.iter_mut().enumerate() {
    let hp = one.get_hp();
    println!("第 {i} hp 是：{hp}");
    match one {
      HeroDispatch::Support(support) => {
        // support 的类型是 &mut Support
        support.heal();
      }

      HeroDispatch::Assassin(assassin) => {}
    }
  }
  // TODO: 补充函数使用时 基于 trait 的泛型约束
}
