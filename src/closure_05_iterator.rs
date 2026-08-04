// 5. 闭包 + 迭代器: map / filter / fold 等组合用法

pub fn demo() {
  let nums = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  // ---- map: 转换 (JS: nums.map(x => x * x)) ----
  let squares: Vec<i32> = nums.iter().map(|x| x * x).collect();
  println!("squares: {squares:?}");

  // ---- filter: 筛选 (JS: nums.filter(x => x % 2 === 0)) ----
  // 注意: iter() 迭代 &i32, filter 闭包收到 &&i32, 所以要 **x
  let evens: Vec<i32> = nums.iter().filter(|x| **x % 2 == 0).copied().collect();
  println!("evens: {evens:?}");

  // ---- filter_map: 筛选 + 转换 ----
  let parsed: Vec<i32> = ["1", "bad", "3"]
    .iter()
    .filter_map(|s| s.parse().ok())
    .collect();
  println!("parsed: {parsed:?}");

  // ---- for_each: 消费每个元素 ----
  let mut sum = 0;
  nums.iter().for_each(|x| sum += x);
  println!("for_each 累计 sum = {sum}");

  // ---- fold: 归约 (JS: nums.reduce((acc, x) => acc + x, 0)) ----
  let total = nums.iter().fold(0, |acc, x| acc + x);
  println!("fold sum = {total}");

  // ---- any / all ----
  println!("any > 9: {}", nums.iter().any(|x| *x > 9));
  println!("all > 0: {}", nums.iter().all(|x| *x > 0));

  // ---- sort_by_key: 需要 FnMut (内部会多次调用比较闭包) ----
  let mut words = vec!["aaa", "b", "ccccc", "dd"];
  words.sort_by_key(|w| w.len());
  println!("按长度排序: {words:?}");

  // ---- 捕获环境变量的闭包 (有捕获) ----
  let threshold = 5;
  let big: Vec<i32> = nums.iter().filter(|x| **x > threshold).copied().collect();
  println!("> {threshold}: {big:?}");

  // ---- 链式组合 ----
  let result: i32 = nums
    .iter()
    .filter(|x| **x % 2 == 1)
    .map(|x| x * x)
    .take(3)
    .sum();
  println!("奇数平方取前 3 个之和 = {result}");

  // ---- enumerate: 带下标遍历 (JS: nums.entries()) ----
  for (i, x) in nums.iter().enumerate() {
    if i >= 2 {
      break;
    }
    println!("nums[{i}] = {x}");
  }

  // ---- max_by: 自定义比较闭包 ----
  let people = vec![("a", 20), ("b", 35), ("c", 25)];
  let oldest = people.iter().max_by(|a, b| a.1.cmp(&b.1)).unwrap();
  println!("年龄最大: {:?}", oldest);
}
