# JSer 入侵 Rust
# 第 5 集 非 Copy 类型

## 非 Copy 类型 有哪些

`Copy` 表示一个值可以通过简单的按位复制得到独立副本。非 `Copy` 类型在赋值或传参时，默认会转移所有权。

常见的非 `Copy` 类型：

| 类型 | 非 `Copy` 的原因 |
| --- | --- |
| `String` | 拥有堆上的 UTF-8 缓冲区 |
| `Vec<T>`、`VecDeque<T>` | 拥有动态分配的堆内存 |
| `HashMap<K, V>`、`HashSet<T>` | 拥有哈希表的堆内存 |
| `Box<T>` | 独占地拥有一块堆内存 |
| `Rc<T>`、`Arc<T>` | 拥有引用计数状态，复制需要显式调用 `clone` |
| `File`、`TcpStream`、`MutexGuard` | 管理文件、网络或锁等独占资源 |
| `&mut T` | 同一时刻只允许存在一个可变引用 |

判断规则：

- 所有字段都实现了 `Copy`，类型才能实现 `Copy`。
- 实现了 `Drop` 的类型不能实现 `Copy`。
- `&T` 是 `Copy`，复制的只是共享引用，不是 `T` 本身。
- `[T; N]`、`Option<T>`、`Result<T, E>` 等组合类型，只有在内部类型全部为 `Copy` 时才是 `Copy`。

## 非 Copy 类型 在 “=” 赋值的含义

对非 `Copy` 类型来说，`=` 默认表示 **move（所有权转移）**。会拷贝栈上的数据，同时在编译时具有所有权转移的含义。

```rust
let a = String::from("hello");
let b = a; // String 的所有权从 a 移动到 b

println!("{b}");
// println!("{a}"); // 编译错误：a 已经被移动
```

`String` 在栈上保存 `ptr + len + capacity`，字符内容保存在堆上。执行 `let b = a` 时：

1. `ptr + len + capacity` 的值转移给 `b` **依然会对栈中的数据做内存拷贝**。
2. 堆上的字符数据不会被拷贝。
3. `a` 不再可以使用。
4. `b` 离开作用域时，只由 `b` 负责释放堆内存。
