pub fn demo() {
  const HP_MIN: i32 = 0;
  const HP_MAX: i32 = 100;

  trait Hero {
    fn name(&self) -> &'static str;
    fn hp(&self) -> i32;
    fn hp_mut(&mut self) -> &mut i32;

    fn set_hp(&mut self, hp: i32) {
      *self.hp_mut() = hp.clamp(HP_MIN, HP_MAX);
    }

    fn take_damage(&mut self, amount: i32) {
      self.set_hp(self.hp() - amount);
    }

    fn heal(&mut self, amount: i32) {
      self.set_hp(self.hp() + amount);
    }

    fn is_alive(&self) -> bool {
      self.hp() > HP_MIN
    }
  }

  trait Attacker {
    fn attack(&self, target: &mut dyn Hero) {
      target.take_damage(1);
    }
  }

  struct Support {
    hp: i32,
  }

  impl Support {
    fn new() -> Self {
      Self { hp: HP_MAX }
    }

    fn heal_self(&mut self) {
      self.heal(5);
    }
  }

  impl Hero for Support {
    fn name(&self) -> &'static str {
      "Support"
    }

    fn hp(&self) -> i32 {
      self.hp
    }

    fn hp_mut(&mut self) -> &mut i32 {
      &mut self.hp
    }
  }

  impl Attacker for Support {}

  struct Assassin {
    hp: i32,
    can_kill: bool,
  }

  impl Assassin {
    fn new() -> Self {
      Self {
        hp: HP_MAX,
        can_kill: false,
      }
    }

    fn set_can_kill(&mut self, can_kill: bool) {
      self.can_kill = can_kill;
    }

    fn assassinate(&self, target: &mut dyn Hero) {
      if self.can_kill {
        target.take_damage(target.hp());
      }
    }
  }

  impl Hero for Assassin {
    fn name(&self) -> &'static str {
      "Assassin"
    }

    fn hp(&self) -> i32 {
      self.hp
    }

    fn hp_mut(&mut self) -> &mut i32 {
      &mut self.hp
    }
  }

  impl Attacker for Assassin {}

  let mut support = Support::new();
  let mut assassin = Assassin::new();

  assassin.attack(&mut support);
  assert_eq!(support.hp(), 99);

  support.heal_self();
  assert_eq!(support.hp(), 100);

  assassin.set_can_kill(true);
  assassin.assassinate(&mut support);
  assert_eq!(support.hp(), 0);
}
