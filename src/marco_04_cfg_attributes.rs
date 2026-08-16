//! `cfg` 是编译期开关：条件为假时，标记的代码不会进入后续编译阶段。

#[cfg(target_os = "macos")]
fn platform_name() -> &'static str {
  "macOS"
}

#[cfg(target_os = "linux")]
fn platform_name() -> &'static str {
  "Linux"
}

#[cfg(target_os = "windows")]
fn platform_name() -> &'static str {
  "Windows"
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_name() -> &'static str {
  "其他平台"
}

#[cfg(feature = "verbose")]
fn feature_message() -> &'static str {
  "verbose feature 已开启"
}

#[cfg(not(feature = "verbose"))]
fn feature_message() -> &'static str {
  "verbose feature 未开启"
}

// `cfg_attr` 根据条件决定是否添加另一个属性。
#[cfg_attr(feature = "verbose", derive(Debug))]
struct BuildInfo {
  mode: &'static str,
}

// `any()` 永远为假。函数体中的名字不存在，但整项会先被 cfg 删除。
#[cfg(any())]
fn code_removed_before_name_resolution() {
  this_function_does_not_exist();
}

pub fn demo() {
  println!("本次编译目标: {}", platform_name());
  println!("{}", feature_message());
  println!("cfg! 产生布尔常量: unix = {}", cfg!(unix));

  let info = BuildInfo { mode: "debug" };
  println!("BuildInfo.mode = {}", info.mode);

  #[cfg(feature = "verbose")]
  println!("cfg_attr 生成的 Debug: {info:?}");

  // 对照：`cfg!` 只返回 bool，不删除所在分支；两边仍必须能通过编译。
  if cfg!(target_pointer_width = "64") {
    println!("当前目标是 64 位");
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn exactly_one_platform_implementation_is_compiled() {
    assert!(!platform_name().is_empty());
  }

  #[cfg(feature = "verbose")]
  #[test]
  fn cfg_attr_adds_debug_when_feature_is_enabled() {
    let info = BuildInfo { mode: "test" };
    assert_eq!(format!("{info:?}"), "BuildInfo { mode: \"test\" }");
  }
}
