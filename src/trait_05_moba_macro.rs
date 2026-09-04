use duck_trait::{props};
use crate::_fields::_Hp;
/*
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
#[props]
struct Support {
  hp: i32,
}

/*----------------- 刺客 -----------------*/
#[props]
struct Assassin {
  hp: i32,
  can_kill: bool,
}
/*----------------- 攻击模组 -----------------*/
trait CanAttack {
  fn attack<T: _Hp<i32>>(&self, target: &mut T) {
    target.hp_set(target.hp() - 1);
  }
}

/*----------------- 治疗模组 -----------------*/
#[props(hp: i32)]
trait CanHeal {
  fn heal(&mut self) {
    self.hp_set((self.hp() + 5).min(100));
  }
}

/*----------------- 刺杀模组 -----------------*/
#[props(can_kill: bool)]
trait CanKill {
  fn kill<T: _Hp<i32>>(&self, target: &mut T) {
    if *self.can_kill() {
      target.hp_set(0);
    }
  }
}

impl CanAttack for Support {}
impl CanHeal for Support {}

impl CanAttack for Assassin {}
impl CanKill for Assassin {}

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
