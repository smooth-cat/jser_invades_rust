# JSer 入侵 Rust
# 第 8 集 枚举与模式匹配

目标：学习 `enum`、`match`、`if let`、`while let`、`let else` 和 `Option`，看懂 Rust 项目中常见的状态建模方式。

## 学习路线

### 1. enum：带数据的"类型级"联合

JS 里"一个值可能是 A 或 B 或 C"要靠手写约定（字符串常量、`typeof` 判断），编译器不帮你兜底。Rust 的 `enum` 是类型系统的一部分，每个变体（variant）还能携带数据：

```rust
enum Coin {
    Penny,
    Quarter(u8),          // 变体带数据：25 美分对应的年份
    Paper { amount: u32 }, // 也可以像 struct 一样带命名字段
}
```

对照 JS：

```js
// JS：没有类型约束，写错只能运行时才知道
const coin = { kind: "Quarter", data: 2000 };
```

### 2. match：穷尽所有可能

`match` 必须覆盖所有变体，编译器强制你处理每一种情况（穷尽性检查），这是 JS 的 `switch` 给不了的保障：

```rust
fn describe(c: &Coin) -> String {
    match c {
        Coin::Penny => "一分钱".to_string(),
        Coin::Quarter(y) => format!("{y} 年的 25 美分"),
        Coin::Paper { amount } => format!("{amount} 元纸币"),
    }
}
```

可以同时使用解构、守卫（guard）、通配符 `_` 和绑定值：

```rust
match c {
    Coin::Quarter(y) if *y > 2000 => "新版硬币",
    Coin::Quarter(_) => "旧版硬币",
    _ => "其他",
}
```

### 3. if let / while let：只关心一种情况

当只关心一个变体、不想为其他变体写分支时，用 `if let` 更简洁：

```rust
if let Coin::Quarter(y) = coin {
    println!("拿到了 {y} 年的硬币");
}
```

JS 对照（用条件守卫数据）：

```js
if (coin.kind === "Quarter") {
  console.log(`拿到了 ${coin.data} 年的硬币`);
}
```

`while let` 则适合"只要还能匹配就继续循环"的场景，比如手写一个从 `Vec` 弹出元素的循环。

### 4. let else：匹配失败提前退出

Rust 2024 常用写法：如果匹配不成功就提前 `return`（或 `break`/`continue`），成功则绑定变量，让主逻辑不用层层缩进：

```rust
fn find_quarter(c: &Coin) -> Option<u8> {
    let Coin::Quarter(y) = c else {
        return None;
    };
    Some(*y)
}
```

### 5. Option<T>：Rust 版"可能有也可能没有"

`Option<T>` 就是标准库里的一个 enum：

```rust
enum Option<T> {
    Some(T),
    None,
}
```

对应 JS 的 `value ?? undefined`，但区别是 Rust 编译器强制你处理 `None` 的情况，不能像 JS 那样悄悄 `undefined` 一路传递。常用方法：`unwrap()`（会 panic，慎用）、`unwrap_or` / `unwrap_or_else`（给默认值）、`map` / `and_then`（链式处理）、`?`（传播 None，见下文示例）。

### 6. ? 运算符：让 None 自动提前返回

```rust
fn half(x: Option<u32>) -> Option<u32> {
    let y = x?;         // None 直接 return None，Some 解出值
    Some(y / 2)
}
```

### 7. 状态建模实战：用 enum 表示游戏/流程状态

Rust 项目最常见的建模方式：用一个 enum 穷举所有状态，配合 `match` 做状态迁移，非法状态根本无法被构造出来——这正是 JS 里"状态 + 布尔标志"组合容易出错的地方：

```rust
enum ProcessState {
    Idle,
    Running { started_at: u64, task: String },
    Paused { resume_at: u64 },
    Done { result: String },
    Failed(String),
}

impl ProcessState {
    fn start(self, task: String) -> ProcessState {
        match self {
            ProcessState::Idle => ProcessState::Running { started_at: now(), task },
            // 非 Idle 状态调用 start 是非法操作 → 直接编译期禁止分支
            other => other,
        }
    }
}
```

要点：

- 每个状态需要什么数据，就写在对应变体里（如 `Paused` 需要 `resume_at`）；
- `match` 的穷尽性保证迁移时所有状态都被考虑；
- 状态机是"非法状态不可表示"（make illegal states unrepresentable）的最佳实践。
