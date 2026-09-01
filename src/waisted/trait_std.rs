/**
 * 常用 std trait：Rust 的"内置协议"
 * JS 的 toString / == / 默认值是语言隐式行为，Rust 里全是 trait，显式实现或 derive
 */
use std::fmt;
use std::ops::Add;

pub fn demo() {
  // derive 宏一行 = 编译器生成实现，TS/JS 没有这种能力
  #[derive(Debug, Clone, PartialEq, Default)]
  struct Hero {
    name: String,
    hp: i32,
  }
  // Display 无法 derive（怎么展示只有你知道），必须手写 = JS 的 toString()
  impl fmt::Display for Hero {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "{}({})", self.name, self.hp)
    }
  }

  println!("----------------- Debug / Display -----------------");
  let hero = Hero {
    name: "寒冰".into(),
    hp: 100,
  };
  println!("{:?}", hero); // Debug：调试格式 {:?}，derive 得来
  println!("{}", hero);   // Display：用户可读格式 {}，手写的

  println!("----------------- Clone / Copy -----------------");
  #[derive(Clone, Copy)]
  struct Point {
    x: i32,
    y: i32,
  }
  let p1 = Point { x: 1, y: 2 };
  let p2 = p1; // Copy 类型：赋值 = 按位复制，p1 还能用（纯栈数据才允许 Copy）
  println!("{} {} vs {} {}", p1.x, p1.y, p2.x, p2.y); // 1 2 vs 1 2
  // Hero 里有 String（堆内存），只能显式 Clone，不能 Copy
  let h2 = hero.clone();
  println!("{}", h2);

  println!("----------------- PartialEq / Default -----------------");
  // == 就是 PartialEq::eq，不实现连 == 都用不了
  println!("{}", hero == Hero { name: "寒冰".into(), hp: 100 }); // true
  // Default = JS 的初始对象/默认参数
  println!("{:?}", Hero::default()); // Hero { name: "", hp: 0 }

  println!("----------------- From / Into -----------------");
  struct Damage(i32);
  // 实现 From<i32> 后，Into<i32> 自动获得
  impl From<i32> for Damage {
    fn from(v: i32) -> Self {
      Damage(v.max(0)) // 顺手加规则：伤害不为负
    }
  }
  let d1 = Damage::from(80);
  let d2: Damage = 80.into(); // 同一件事的两种写法
  println!("{} {}", d1.0, d2.0); // 80 80

  println!("----------------- Drop：确定性析构 -----------------");
  // JS 靠 GC，没有"离开作用域立刻执行"的钩子；Rust 的 Drop 就是（文件/锁/连接靠它关闭）
  struct Session;
  impl Drop for Session {
    fn drop(&mut self) {
      println!("Session 已销毁");
    }
  }
  {
    let _s = Session; // _s 前缀：保持到作用域结束才 drop
    println!("Session 使用中");
  } // <-- 这一行立即执行 drop，不等 GC
  println!("Session 已离开作用域");

  println!("----------------- Add：运算符重载 -----------------");
  // a + b 就是 Add::add(a, b)，实现后自定义类型也能参与 +
  #[derive(Debug)]
  struct Mana(i32);
  impl Add for Mana {
    type Output = Mana;
    fn add(self, other: Mana) -> Mana {
      Mana(self.0 + other.0)
    }
  }
  println!("{:?}", Mana(30) + Mana(70)); // Mana(100)
}

#[test]
fn run() {
  demo();
}
