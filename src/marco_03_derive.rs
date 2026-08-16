//! `#[derive(...)]` 读取类型定义，并为类型生成 trait 实现。

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct User {
  id: u64,
  name: String,
}

// 这个类型手写了上面 derive 宏生成的核心代码，便于对照阅读。
struct ManualUser {
  id: u64,
  name: String,
}

impl std::fmt::Debug for ManualUser {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ManualUser")
      .field("id", &self.id)
      .field("name", &self.name)
      .finish()
  }
}

impl Clone for ManualUser {
  fn clone(&self) -> Self {
    Self {
      id: self.id,
      name: self.name.clone(),
    }
  }
}

impl PartialEq for ManualUser {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.name == other.name
  }
}

impl Eq for ManualUser {}

// 本段特意手写本可 derive 的实现，以展示宏隐藏的代码。
#[allow(clippy::derivable_impls)]
impl Default for ManualUser {
  fn default() -> Self {
    Self {
      id: Default::default(),
      name: Default::default(),
    }
  }
}

pub fn demo() {
  let derived = User {
    id: 1,
    name: "Ada".to_owned(),
  };
  let cloned = derived.clone();
  println!("derive 生成 Debug::fmt: {derived:?}");
  assert_eq!(derived, cloned);
  assert_eq!(User::default().name, "");

  let manual = ManualUser {
    id: 1,
    name: "Ada".to_owned(),
  };
  println!("手写 trait 实现: {manual:?}");
  assert_eq!(manual, manual.clone());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn derived_and_manual_implementations_have_the_same_behavior() {
    let derived = User::default();
    let manual = ManualUser::default();
    assert_eq!(derived.id, manual.id);
    assert_eq!(derived.name, manual.name);
  }
}
