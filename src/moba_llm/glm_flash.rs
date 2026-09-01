const MAX_HP: u32 = 100;
const MIN_HP: u32 = 0;
const HEAL_AMOUNT: u32 = 5;

#[derive(Debug)]
struct HeroCore {
  name: String,
  hp: u32,
}

impl HeroCore {
  fn new(name: &str) -> Self {
    Self {
      name: name.to_string(),
      hp: MAX_HP,
    }
  }
}

trait Hero {
  fn core(&self) -> &HeroCore;
  fn core_mut(&mut self) -> &mut HeroCore;

  fn name(&self) -> &str {
    &self.core().name
  }

  fn hp(&self) -> u32 {
    self.core().hp
  }

  fn take_damage(&mut self, amount: u32) {
    let core = self.core_mut();
    core.hp = core.hp.saturating_sub(amount).max(MIN_HP);
  }

  fn heal(&mut self, amount: u32) {
    let core = self.core_mut();
    core.hp = core.hp.saturating_add(amount).min(MAX_HP);
  }

  fn kill(&mut self) {
    self.core_mut().hp = MIN_HP;
  }

  fn attack(&self, target: &mut dyn Hero) {
    target.take_damage(1);
  }
}

struct Support {
  core: HeroCore,
}

impl Support {
  fn new(name: &str) -> Self {
    Self {
      core: HeroCore::new(name),
    }
  }

  fn heal_self(&mut self) {
    self.heal(HEAL_AMOUNT);
  }
}

impl Hero for Support {
  fn core(&self) -> &HeroCore {
    &self.core
  }

  fn core_mut(&mut self) -> &mut HeroCore {
    &mut self.core
  }
}

struct Assassin {
  core: HeroCore,
  can_kill: bool,
}

impl Assassin {
  fn new(name: &str) -> Self {
    Self {
      core: HeroCore::new(name),
      can_kill: false,
    }
  }

  fn assassinate(&self, target: &mut dyn Hero) {
    if self.can_kill {
      target.kill();
    }
  }
}

impl Hero for Assassin {
  fn core(&self) -> &HeroCore {
    &self.core
  }

  fn core_mut(&mut self) -> &mut HeroCore {
    &mut self.core
  }
}

pub fn demo() {
  let mut support = Support::new("辅助");
  let mut assassin = Assassin::new("刺客");

  println!(
    "[初始] {} hp={}，{} hp={}",
    assassin.name(),
    assassin.hp(),
    support.name(),
    support.hp()
  );

  assassin.attack(&mut support);
  println!(
    "[普攻] {} 普通攻击 {}，{} hp={}",
    assassin.name(),
    support.name(),
    support.name(),
    support.hp()
  );

  support.heal_self();
  println!("[回血] {} 恢复血量，hp={}", support.name(), support.hp());

  assassin.can_kill = true;
  assassin.assassinate(&mut support);
  println!(
    "[刺杀] {} 刺杀 {}，{} hp={}",
    assassin.name(),
    support.name(),
    support.name(),
    support.hp()
  );
}
