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
trait CanAttack {
  fn attack(&self, hp: &mut i32) {
    *hp -= 1;
  }
}
trait CanHeal {
  fn heal(&self, hp: &mut i32) {
    let new_hp = (*hp + 5).min(100);
    *hp = new_hp;
  }
}

trait CanKill {
  fn kill(&self, hp: &mut i32, can_kill: bool) {
    if can_kill {
      *hp = 0;
    }
  }
}

impl CanAttack for Support {}
impl CanHeal for Support {}

impl CanAttack for Assassin {}
impl CanKill for Assassin {}

pub fn demo() {
  let mut support = Support { hp: 100 };
  let mut assassin = Assassin {
    hp: 100,
    can_kill: false,
  };
  assassin.attack(&mut support.hp);

  // 不能直接 &mut hp ，因为 trait 签名是 &self 有读写冲突
  let mut hp = support.hp;
  support.heal(&mut hp);

  assassin.can_kill = true;
  assassin.kill(&mut support.hp, assassin.can_kill);
}

#[test]
fn run() {
  demo();
}
