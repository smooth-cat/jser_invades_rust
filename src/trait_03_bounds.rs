/**
 * trait 的"继承"与边界，呼应 js/extends-mess.js
 * 1. supertrait：只叠加"要求"，不继承实现，没有 super 调用链
 * 2. 对象安全：为什么有的 trait 不能 dyn
 * 3. extension trait + 孤儿规则：给外部类型加方法 = 安全版猴子补丁
 */
pub fn demo() {
  println!("----------------- super trait：trait 的继承 -----------------");
  trait HasHp {
    fn hp(&self) -> i32;
  }
  // : HasHp = TS 的 interface Hero extends HasHp
  // 只是"要求"：想实现 Hero 必须先把 HasHp 实现了，编译器强制
  trait Hero: HasHp {
    fn name(&self) -> String;
    // 默认方法可以直接调用 supertrait 的方法
    fn status(&self) -> String {
      format!("{} ({})", self.name(), self.hp())
    }
  }

  struct Assassin {
    name: String,
    hp: i32,
  }
  impl HasHp for Assassin {
    fn hp(&self) -> i32 {
      self.hp
    }
  }
  impl Hero for Assassin {
    fn name(&self) -> String {
      self.name.clone()
    }
  }

  let a = Assassin {
    name: "劫".into(),
    hp: 100,
  };
  println!("{}", a.status()); // 劫 (100)
  // 和 JS 继承的本质区别：没有 super，重写默认方法只能整体替换，
  // 不存在 js/extends-mess.js 里 super 链条打乱执行顺序的问题

  println!("----------------- 对象安全：为什么有的 trait 不能 dyn -----------------");
  trait Factory {
    // 没有 self：调用时根本不知道是"谁"在 create，dyn 的 vtable 没地方放它
    fn create() -> Self;
  }
  struct Scroll;
  impl Factory for Scroll {
    fn create() -> Self {
      Scroll
    }
  }
  let _scroll = Scroll::create(); // 只能静态调用
  // let f: Box<dyn Factory> = ...;
  // ^ 编译错误：Factory 不是对象安全的（返回 Self / 无 self / 泛型方法都不行）
  // 速记：需要"编译期知道具体类型"的成员，都不能 dyn

  println!("----------------- supertrait + dyn：向上转型 -----------------");
  // dyn Hero 的 vtable 里包含 HasHp 的方法，所以能直接向上转型（1.86+ 稳定）
  let hero: Box<dyn Hero> = Box::new(a);
  let has_hp: &dyn HasHp = &*hero; // 不需要重新打包，复用同一份 vtable
  println!("血量 {}", has_hp.hp()); // 血量 100

  println!("----------------- extension trait + 孤儿规则 -----------------");
  // 孤儿规则：impl 必须写在本 crate，且 trait 或类型至少有一个是本地的
  // JS 想给 number 加方法只能猴子补丁 Number.prototype（全局污染、可能冲突）；
  // Rust 定义自己的 trait 再 impl 给 f64：方法只在引入 trait 后可见，不会污染别人
  trait ToMoney {
    fn to_money(&self) -> String;
  }
  impl ToMoney for f64 {
    fn to_money(&self) -> String {
      format!("{:.2} 元", self)
    }
  }
  println!("{}", 99.5.to_money()); // 99.50 元
  // 同理可以给 String、Vec 等 std 类型"扩展"方法，但只在本 crate 生效
}

#[test]
fn run() {
  demo();
}
