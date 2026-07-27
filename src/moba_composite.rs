/**
 * 组合式
 * 1. 英雄实现 hp、attack 基础方法
 * 2. 辅助实现 heal 治疗
 * 3. 刺客实现 can_kill、kill 方法
 */
pub fn composite() {
  // 核心数据与通用逻辑
  struct Hero {
    hp: i32,
  }

  impl Hero {
    fn new(hp: i32) -> Self {
      Self { hp }
    }

    fn attack(&mut self, target: &mut Hero) {
      target.hp -= 1;
    }
  }

  /** 辅助 */
  struct Support {
    hero: Hero,
  }

  impl Support {
    fn new(hp: i32) -> Self {
      Self {
        hero: Hero::new(hp),
      }
    }

    // Support 自己的专属技能
    fn heal(&mut self) {
      self.hero.hp = (self.hero.hp + 5).min(100);
    }
  }

  /** 刺客 */
  struct Assassin {
    hero: Hero,
    can_kill: bool,
  }

  impl Assassin {
    fn new(hp: i32, can_kill: bool) -> Self {
      Self {
        hero: Hero::new(hp),
        can_kill,
      }
    }

    // Assassin 自己的专属技能，传入的是 &mut Hero 或者是带有 Hero 的其他角色
    fn kill(&self, target: &mut Hero) {
      if self.can_kill {
        target.hp = 0;
      }
    }
  }
  println!("----------------- 组合式 -----------------");
  let mut support = Support::new(100);
  let mut assassin = Assassin::new(100, false);

  // 攻击：通过内部的 hero 实例发起攻击
  assassin.hero.attack(&mut support.hero);
  println!("{}", support.hero.hp); // 99

  // 治疗：直接调用 Support 的方法
  support.heal();
  println!("{}", support.hero.hp); // 100

  // 斩杀
  assassin.can_kill = true;
  assassin.kill(&mut support.hero);
  println!("{}", support.hero.hp); // 0
}
