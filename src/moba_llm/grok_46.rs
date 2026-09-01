pub fn demo() {
  const HP_MIN: u32 = 0;
  const HP_MAX: u32 = 100;
  const BASIC_ATK: u32 = 1;
  const HEAL_AMOUNT: u32 = 5;

  struct Hp {
    current: u32,
  }

  impl Hp {
    fn full() -> Self {
      Self { current: HP_MAX }
    }

    fn get(&self) -> u32 {
      self.current
    }

    fn damage(&mut self, amount: u32) {
      self.current = self.current.saturating_sub(amount).max(HP_MIN);
    }

    fn heal(&mut self, amount: u32) {
      self.current = self.current.saturating_add(amount).min(HP_MAX);
    }
  }

  trait Hero {
    fn hp(&self) -> u32;
    fn receive_damage(&mut self, amount: u32);

    fn basic_attack(&self, target: &mut dyn Hero) {
      target.receive_damage(BASIC_ATK);
    }
  }

  trait Healer: Hero {
    fn heal_self(&mut self);
  }

  trait AssassinSkill: Hero {
    fn can_kill(&self) -> bool;
    fn set_can_kill(&mut self, value: bool);
    fn assassinate(&self, target: &mut dyn Hero) {
      if self.can_kill() {
        let lethal = target.hp();
        target.receive_damage(lethal);
      }
    }
  }

  struct Support {
    hp: Hp,
  }

  impl Support {
    fn new() -> Self {
      Self { hp: Hp::full() }
    }
  }

  impl Hero for Support {
    fn hp(&self) -> u32 {
      self.hp.get()
    }

    fn receive_damage(&mut self, amount: u32) {
      self.hp.damage(amount);
    }
  }

  impl Healer for Support {
    fn heal_self(&mut self) {
      self.hp.heal(HEAL_AMOUNT);
    }
  }

  struct Assassin {
    hp: Hp,
    can_kill: bool,
  }

  impl Assassin {
    fn new() -> Self {
      Self {
        hp: Hp::full(),
        can_kill: false,
      }
    }
  }

  impl Hero for Assassin {
    fn hp(&self) -> u32 {
      self.hp.get()
    }

    fn receive_damage(&mut self, amount: u32) {
      self.hp.damage(amount);
    }
  }

  impl AssassinSkill for Assassin {
    fn can_kill(&self) -> bool {
      self.can_kill
    }

    fn set_can_kill(&mut self, value: bool) {
      self.can_kill = value;
    }
  }

  let mut support = Support::new();
  let mut assassin = Assassin::new();
  assassin.basic_attack(&mut support);
  support.heal_self();
  assassin.set_can_kill(true);
  assassin.assassinate(&mut support);
}
