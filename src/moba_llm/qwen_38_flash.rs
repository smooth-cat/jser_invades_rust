/// HP 上限
const MAX_HP: u32 = 100;

/// 英雄基础状态：封装 hp，所有读写必须经过方法，保证 hp 始终在 0..=100
#[derive(Debug)]
struct HeroStats {
  hp: u32,
}

impl HeroStats {
  fn new() -> Self {
    Self { hp: MAX_HP }
  }

  fn hp(&self) -> u32 {
    self.hp
  }

  /// 受到伤害，最低扣到 0
  fn take_damage(&mut self, amount: u32) {
    self.hp = self.hp.saturating_sub(amount);
  }

  /// 恢复血量，最高加到 MAX_HP
  fn heal(&mut self, amount: u32) {
    self.hp = (self.hp + amount).min(MAX_HP);
  }
}

/// 可被伤害的目标：普攻 / 刺杀可作用于任意实现了该 trait 的英雄
trait Damageable {
  fn take_damage(&mut self, amount: u32);
  fn hp(&self) -> u32;
}

/// 英雄统一接口：后续新增英雄只需实现本 trait 与所需的能力 trait
trait Hero: Damageable {
  fn name(&self) -> &str;
}

/// 能力：普通攻击，默认实现为对目标造成 1 点伤害
trait BasicAttack {
  fn basic_attack<T: Damageable + ?Sized>(&self, target: &mut T) {
    target.take_damage(1);
  }
}

/// 能力：恢复自身血量
trait SelfHeal {
  fn heal_self(&mut self);
}

/// 能力：刺杀，can_kill 为 true 时将目标 hp 置为 0
trait Execution {
  fn can_kill(&self) -> bool;

  fn execute<T: Damageable + ?Sized>(&mut self, target: &mut T) {
    if self.can_kill() {
      target.take_damage(u32::MAX);
    }
  }
}

/// 辅助
#[derive(Debug)]
struct Support {
  stats: HeroStats,
}

impl Support {
  fn new() -> Self {
    Self {
      stats: HeroStats::new(),
    }
  }
}

impl Damageable for Support {
  fn take_damage(&mut self, amount: u32) {
    self.stats.take_damage(amount);
  }

  fn hp(&self) -> u32 {
    self.stats.hp()
  }
}

impl Hero for Support {
  fn name(&self) -> &str {
    "Support"
  }
}

impl BasicAttack for Support {}

impl SelfHeal for Support {
  /// 每次恢复 5 点血量（受 MAX_HP 封顶）
  fn heal_self(&mut self) {
    self.stats.heal(5);
  }
}

/// 刺客
#[derive(Debug)]
struct Assassin {
  stats: HeroStats,
  can_kill: bool,
}

impl Assassin {
  fn new() -> Self {
    Self {
      stats: HeroStats::new(),
      can_kill: false,
    }
  }

  /// 外部只能通过 setter 修改 can_kill，保证状态可控
  fn set_can_kill(&mut self, value: bool) {
    self.can_kill = value;
  }
}

impl Damageable for Assassin {
  fn take_damage(&mut self, amount: u32) {
    self.stats.take_damage(amount);
  }

  fn hp(&self) -> u32 {
    self.stats.hp()
  }
}

impl Hero for Assassin {
  fn name(&self) -> &str {
    "Assassin"
  }
}

impl BasicAttack for Assassin {}

impl Execution for Assassin {
  fn can_kill(&self) -> bool {
    self.can_kill
  }
}

pub fn demo() {
  let mut support = Support::new();
  let mut assassin = Assassin::new();

  println!(
    "初始状态: {} hp={}, {} hp={}",
    support.name(),
    support.hp(),
    assassin.name(),
    assassin.hp()
  );

  // 1. 刺客对辅助进行一次普通攻击：辅助掉 1 滴血
  assassin.basic_attack(&mut support);
  println!("普攻后:   {} hp={}", support.name(), support.hp());

  // 2. 辅助回血：hp +5（不超过上限 100）
  support.heal_self();
  println!("回血后:   {} hp={}", support.name(), support.hp());

  // 3. 刺客将 can_kill 设置为 true 并刺杀辅助：辅助 hp 置为 0
  assassin.set_can_kill(true);
  assassin.execute(&mut support);
  println!(
    "刺杀后:   {} hp={}, {} hp={}",
    support.name(),
    support.hp(),
    assassin.name(),
    assassin.hp()
  );
}
