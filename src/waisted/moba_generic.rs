/**
 * 泛型组合式 基础类型 包裹 派生 Role
 * 1. 英雄实现 hp、attack 基础方法
 * 2. 辅助实现 heal 治疗
 * 3. 刺客实现 can_kill、kill 方法
 */
pub fn demo() {
  struct Hero<Role> {
    hp: i32,
    role: Role,
  }
  trait AttackAble {
    fn attack<TargetRole>(&self, target: &mut Hero<TargetRole>);
  }

  impl<Role> Hero<Role> {
    fn new(role: Role) -> Self {
      Self { hp: 100, role }
    }
  }
  impl<Role> AttackAble for Hero<Role> {
    /// 普通攻击
    fn attack<TargetRole>(&self, target: &mut Hero<TargetRole>) {
      target.hp -= 1;
    }
  }

  struct Support;

  impl Hero<Support> {
    /// 回血
    fn heal(&mut self) {
      self.hp = (self.hp + 5).min(100);
    }
  }

  struct Assassin {
    can_kill: bool,
  }

  impl Hero<Assassin> {
    /// 击杀
    fn kill<TargetRole>(&self, target: &mut Hero<TargetRole>) {
      if self.role.can_kill {
        target.hp = 0;
      }
    }
  }

  println!("----------------- 泛型组合式-静态分发 -----------------");
  let mut support = Hero::new(Support {});
  let mut assassin = Hero::new(Assassin { can_kill: false });

  assassin.attack(&mut support);
  println!("{}", support.hp); // 99

  support.heal();
  println!("{}", support.hp); // 100

  assassin.role.can_kill = true;
  assassin.kill(&mut support);
  println!("{}", support.hp); // 0

  enum AnyHero {
    Support(Hero<Support>),
    Assassin(Hero<Assassin>),
  }

  println!("----------------- 泛型组合式-枚举静态分发 -----------------");
  let mut heroes = vec![
    AnyHero::Support(Hero::new(Support)),
    AnyHero::Assassin(Hero::new(Assassin { can_kill: false })),
  ];

  for (i, hero) in &mut heroes.iter_mut().enumerate() {
    match hero {
      AnyHero::Support(support) => {
        // support 的类型是 &mut Hero<Support>
        support.heal();
        println!("第 {i} 是辅助， hp 是 {}", support.hp);
      }

      AnyHero::Assassin(assassin) => {
        println!("第 {i} 是刺客， hp 是 {}", assassin.hp);
      }
    }
  }
}

#[test]
fn run() {
  demo();
}