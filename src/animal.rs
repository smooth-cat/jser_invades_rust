// 抽象动物: Animal Trait + Dog / Cat 实现
// JS:  class Animal { bark() {} }  class Dog extends Animal { bark() { return '汪汪'; } }

pub trait Animal {
  fn name(&self) -> &str;
  fn bark(&self) -> &'static str;
}
#[derive(Debug)]
pub struct Dog {
  pub name: String,
}

impl Animal for Dog {
  fn name(&self) -> &str {
    &self.name
  }

  fn bark(&self) -> &'static str {
    "汪汪汪!"
  }
}

#[derive(Debug)]
pub struct Cat {
  pub name: String,
}

impl Animal for Cat {
  fn name(&self) -> &str {
    &self.name
  }

  fn bark(&self) -> &'static str {
    "喵喵喵!"
  }
}
