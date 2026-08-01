use std::mem;
/*----------------- 不可返回函数自己产生的指针 -----------------*/
fn no_return_self_ref() {
  let s = 32;
  let ptr = &s;
  // 错误，所有者 s 只能活到函数结束，ptr 就被释放
  // return ptr; 
}

/*----------------- 自动推导生命周期 -----------------*/
pub fn auto_infer(v: &str) -> &str {
  return v;
}

/*----------------- 手动标注 -----------------*/
/**
 * 为什么需要标注
 * 有些生命周期运行时才能确定， 但生命周期是编译时概念
 * 通过生命周期标注来 消除不确定性
 */
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
  if x.len() > y.len() { x } else { y }
}

pub fn life_shrink() {
  let s1 = String::from("long string is long");
  let result;
  {
    let s2 = String::from("xyz");
    /*
      https://doc.rust-lang.org/rust-by-example/scope/lifetime/lifetime_coercion.html#coercion
      把长生命周期 当做 短生命周期使用
      把 's1 生命周期当成 's2
      为什么可以？ 
      s1 实际在函数内可用，被当成 在块内可用
      longest 在块内调用，参数在块内活着就 OK
    */
    result = longest(s1.as_str(), s2.as_str());
  }
  // println!("The longest string is {}", result);
  println!("s1: {}", s1);
}

/*----------------- 标注生命周期大小 与实际有冲突时 -----------------*/
pub fn conflict_life_circle() {
  let s1 = String::from("long life");
  let result;
  {
    let s2 = String::from("xyz");
    // 任何获得更长 变量都可以 视为 活的更短的变量来使用
    result = shorter(s1.as_str(), s2.as_str());
  }
  // println!("The shorter life string is \"{}\"", result);
}

// 'a <= 'b
fn shorter<'a, 'b: 'a>(_a: &'a str, _b: &'b str) -> &'a str {
  println!("_b: {}", _b);
  return _a;
}

/*----------------- 结构体、Trait 生命周期 -----------------*/
pub fn trait_life_circle() {
  // 定义带生命周期的结构体
  struct TempPerson<'a> {
    name: &'a str,
  }

  // 定义带生命周期的泛型
  trait Name<'a> {
    fn get_name(&self) -> &'a str;
  }
  // 实现时将泛型 'b 传给 trait 和 TempPerson 即可
  impl<'b> Name<'b> for TempPerson<'b> {
    fn get_name(&self) -> &'b str {
      return &self.name;
    }
  }

  let person = TempPerson { name: "tom" };
  println!("{}", person.get_name());
}

/*----------------- 闭包生命周期 -----------------*/
/*----------------- 1. 返回值 与 入参相关 -----------------*/
fn from_param<F>(f: F, s: &str) -> &str
where
  F: Fn(&str) -> &str,
{
  f(s)
}
// 简化 for<'a>
fn from_param_sim(f: impl Fn(&str) -> &str, s: &str) -> &str {
  f(s)
}

pub fn closure_param() {
  let trimmed = from_param(|s| s.trim(), "  hello  ");
  let trimmed2 = from_param_sim(|s| s.trim(), "  hello  ");
}

/*----------------- 2. 返回值 与 捕获指针相关 -----------------*/
fn from_catch<'a>(f: impl Fn() -> &'a str) -> &'a str {
  f()
}

pub fn closure_return() {
  let data = String::from("hello");
  let result = from_catch(|| data.as_str());
  let mut a = 10;
  let mut b = &mut a;
}

/*----------------- 3. struct 存储 闭包 -----------------*/
struct Getter<'a> {
  // 闭包至少活 'a 这么长
  f: Box<dyn Fn(&'a str, &'a str) -> &'a str>,
}
pub fn closure_in_struct() {
  let f: Box<dyn for<'a> Fn(&'a str, &'a str) -> &'a str> =
    Box::new(|a, b| {
      if a.len() > b.len() { a } else { b }
    });
  let getter = Getter { f: f };
  // 使用 getter.f 表示字段。这种写法与 impl 定义的方法分开来保证语义准确，这里指调用 结构体中 f 字段存储的 闭包
  (getter.f)("hello", "world!");
}
