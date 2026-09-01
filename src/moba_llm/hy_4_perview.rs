use std::error::Error;
use std::fmt;

pub const MIN_HP: u8 = 0;
pub const MAX_HP: u8 = 100;
pub const BASIC_ATTACK_DAMAGE: u8 = 1;
pub const HEAL_AMOUNT: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hp(u8);

impl Hp {
  pub const MIN: u8 = MIN_HP;
  pub const MAX: u8 = MAX_HP;

  pub fn new(value: u8) -> Self {
    Self(value.clamp(Self::MIN, Self::MAX))
  }

  pub fn full() -> Self {
    Self(Self::MAX)
  }

  pub fn empty() -> Self {
    Self(Self::MIN)
  }

  pub fn value(self) -> u8 {
    self.0
  }

  pub fn is_alive(self) -> bool {
    self.0 > Self::MIN
  }

  pub fn is_dead(self) -> bool {
    !self.is_alive()
  }

  pub fn damaged(self, amount: u8) -> Self {
    Self::new(self.0.saturating_sub(amount))
  }

  pub fn restored(self, amount: u8) -> Self {
    Self::new(self.0.saturating_add(amount))
  }
}

impl fmt::Display for Hp {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}/{}", self.0, Self::MAX)
  }
}

pub trait Hero: fmt::Debug {
  fn name(&self) -> &str;

  fn hp(&self) -> Hp;

  fn set_hp(&mut self, hp: Hp);

  fn is_alive(&self) -> bool {
    self.hp().is_alive()
  }

  fn is_dead(&self) -> bool {
    self.hp().is_dead()
  }

  fn take_damage(&mut self, amount: u8) -> Hp {
    let hp = self.hp().damaged(amount);
    self.set_hp(hp);
    hp
  }

  fn restore_hp(&mut self, amount: u8) -> Hp {
    let hp = self.hp().restored(amount);
    self.set_hp(hp);
    hp
  }
}

pub trait BasicAttack: Hero {
  fn basic_attack(&self, target: &mut dyn Hero) -> Hp {
    target.take_damage(BASIC_ATTACK_DAMAGE)
  }
}

impl<T: Hero> BasicAttack for T {}

pub trait Healer: Hero {
  fn heal(&mut self) -> Hp {
    self.restore_hp(HEAL_AMOUNT)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssassinationError {
  NotReady,
  TargetAlreadyDead,
}

impl fmt::Display for AssassinationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotReady => write!(f, "assassination is not ready: can_kill is false"),
      Self::TargetAlreadyDead => write!(f, "target is already dead"),
    }
  }
}

impl Error for AssassinationError {}

pub trait Assassination: Hero {
  fn can_kill(&self) -> bool;

  fn set_can_kill(&mut self, value: bool);

  fn assassinate(&mut self, target: &mut dyn Hero) -> Result<Hp, AssassinationError> {
    if !self.can_kill() {
      return Err(AssassinationError::NotReady);
    }
    if target.is_dead() {
      return Err(AssassinationError::TargetAlreadyDead);
    }
    Ok(target.take_damage(MAX_HP))
  }
}

#[derive(Debug, Clone)]
pub struct Support {
  name: String,
  hp: Hp,
}

impl Support {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      hp: Hp::full(),
    }
  }
}

impl Hero for Support {
  fn name(&self) -> &str {
    &self.name
  }

  fn hp(&self) -> Hp {
    self.hp
  }

  fn set_hp(&mut self, hp: Hp) {
    self.hp = hp;
  }
}

impl Healer for Support {}

#[derive(Debug, Clone)]
pub struct Assassin {
  name: String,
  hp: Hp,
  can_kill: bool,
}

impl Assassin {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      hp: Hp::full(),
      can_kill: false,
    }
  }
}

impl Hero for Assassin {
  fn name(&self) -> &str {
    &self.name
  }

  fn hp(&self) -> Hp {
    self.hp
  }

  fn set_hp(&mut self, hp: Hp) {
    self.hp = hp;
  }
}

impl Assassination for Assassin {
  fn can_kill(&self) -> bool {
    self.can_kill
  }

  fn set_can_kill(&mut self, value: bool) {
    self.can_kill = value;
  }
}

pub fn demo() {
  let mut support = Support::new("Soraka");
  let mut assassin = Assassin::new("Zed");

  println!("[init] {} hp = {}", support.name(), support.hp());
  println!("[init] {} hp = {}", assassin.name(), assassin.hp());

  let hp = assassin.basic_attack(&mut support);
  println!(
    "[attack] {} -> {} : hp = {} (alive: {})",
    assassin.name(),
    support.name(),
    hp,
    support.is_alive()
  );

  let hp = support.heal();
  println!("[heal] {} heals itself : hp = {}", support.name(), hp);

  assassin.set_can_kill(true);
  match assassin.assassinate(&mut support) {
    Ok(hp) => println!(
      "[assassinate] {} kills {} : hp = {} (dead: {})",
      assassin.name(),
      support.name(),
      hp,
      support.is_dead()
    ),
    Err(e) => println!("[assassinate] failed: {e}"),
  }
}
