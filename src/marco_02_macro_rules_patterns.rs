//! 声明宏可以有多个匹配分支，还能用 `$(...)+` 重复和递归调用自身。

macro_rules! ordered_map {
  () => {
    ::indexmap::IndexMap::new()
  };
  // `+` 表示重复一次或多次，末尾的 `$(,)?` 允许可选的尾逗号。
  ($($key:expr => $value:expr),+ $(,)?) => {{
    let mut map = ::indexmap::IndexMap::new();
    $(
      map.insert($key, $value);
    )*
    map
  }};
}

macro_rules! sum {
  // 递归出口必须写在递归分支前面或后面，但必须存在。
  () => { 0 };
  ($head:expr $(, $tail:expr)* $(,)?) => {
    $head + sum!($($tail),*)
  };
}

macro_rules! describe_value {
  (null) => {
    "匹配固定 token: null"
  };
  ([$($item:expr),* $(,)?]) => {
    concat!("匹配数组元素 token: ", stringify!($($item),*))
  };
  ($other:expr) => {
    concat!("匹配普通表达式: ", stringify!($other))
  };
}

pub fn demo() {
  let scores = ordered_map! {
    "Rust" => 100,
    "JavaScript" => 95,
  };
  println!("保持插入顺序: {scores:?}");

  let total = sum!(10, 20, 30, 40);
  println!("递归宏求和: {total}");
  assert_eq!(total, 10 + sum!(20, 30, 40));

  println!("{}", describe_value!(null));
  println!("{}", describe_value!([1, 2, 3]));
  println!("{}", describe_value!(scores.len()));
}

#[cfg(test)]
mod tests {
  #[test]
  fn repetition_accepts_empty_input_and_a_trailing_comma() {
    let empty: indexmap::IndexMap<&str, i32> = ordered_map! {};
    let one = ordered_map! { "one" => 1, };
    assert!(empty.is_empty());
    assert_eq!(one["one"], 1);
  }

  #[test]
  fn recursive_macro_reaches_its_empty_base_case() {
    assert_eq!(sum!(), 0);
    assert_eq!(sum!(1, 2, 3, 4), 10);
  }
}
