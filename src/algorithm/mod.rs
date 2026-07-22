use crate::util::calc;
// use super::util::calc;
pub fn fib(n: i32) -> i32 {
  if n < 2 {
    return n;
  }
  return calc::add(fib(n - 1), fib(n - 2));
}