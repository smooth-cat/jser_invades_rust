/**
 * 有两个英雄：
 * 1. 辅助
 *   - 普通攻击
 *   - 治疗
 *
 * 2. 刺客
 *   - 普通攻击
 *   - 刺杀
 */
/*----------------- 辅助 -----------------*/
struct Support {
  hp: i32,
}

/*----------------- 刺客 -----------------*/
struct Assassin {
  hp: i32,
  can_kill: bool,
}

/*----------------- 定义 血量模组 -----------------*/
trait HasHp {
  fn hp(&self) -> i32;
  fn hp_set(&mut self, hp: i32);
}

/*----------------- 攻击模组 -----------------*/
trait CanAttack {
  fn attack<T: HasHp>(&self, target: &mut T) {
    target.hp_set(target.hp() - 1);
  }
}

/*----------------- 治疗模组 -----------------*/
trait CanHeal: HasHp {
  fn heal(&mut self) {
    self.hp_set((self.hp() + 5).min(100));
  }
}

/*----------------- 刺杀模组 -----------------*/
trait CanKill {
  fn can_kill(&self) -> bool;
  fn kill<T: HasHp>(&self, target: &mut T) {
    if self.can_kill() {
      target.hp_set(0);
    }
  }
}

impl HasHp for Support {
  fn hp(&self) -> i32 {
    self.hp
  }
  fn hp_set(&mut self, hp: i32) {
    self.hp = hp;
  }
}
impl CanAttack for Support {} // 拼上 attack
impl CanHeal for Support {} // 拼上 heal

impl HasHp for Assassin {
  fn hp(&self) -> i32 {
    self.hp
  }
  fn hp_set(&mut self, hp: i32) {
    self.hp = hp;
  }
}
impl CanAttack for Assassin {}
impl CanKill for Assassin {
  fn can_kill(&self) -> bool {
    self.can_kill
  }
}
pub fn demo() {
  let mut support = Support { hp: 100 }; // JS 工厂里的默认值由调用处给
  let mut assassin = Assassin {
    hp: 100,
    can_kill: false,
  };
  assassin.attack(&mut support);
  println!("{}", support.hp()); // 99

  support.heal(); // 拼了 CanHeal 才有这个方法
  println!("{}", support.hp()); // 100

  assassin.can_kill = true;
  assassin.kill(&mut support);
  println!("{}", support.hp()); // 0
}

#[test]
fn run() {
  demo();
}
