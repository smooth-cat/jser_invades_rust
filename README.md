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

## HashMap 细节

1. 扩容时需要重新计算每个元素的 hash 以及新的分配位置

```rust
unsafe fn resize_inner<A>(
        &mut self,
        alloc: &A,
        capacity: usize,
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        fallibility: Fallibility,
        layout: TableLayout,
    ) -> Result<(), TryReserveError>
    where
        A: Allocator,
    {
        
        let mut new_table = self.prepare_resize(alloc, layout, capacity, fallibility)?;
        unsafe {
            // 遍历已存在的数据
            for full_byte_index in self.full_buckets_indices() {
                // 重算 hash
                let hash = hasher(self, full_byte_index);
              	// 重新定位
                let (new_index, _) = new_table.prepare_insert_index(hash);
                ptr::copy_nonoverlapping(
                    self.bucket_ptr(full_byte_index, layout.size),
                    new_table.bucket_ptr(new_index, layout.size),
                    layout.size,
                );
            }
        }

        
        new_table.growth_left -= self.items;
        new_table.items = self.items;
        mem::swap(self, &mut new_table);

        Ok(())
    }
```

2. 容量计算，最小从 4 开始分配

```rust
fn capacity_to_buckets(cap: usize, table_layout: TableLayout) -> Option<usize> {
    debug_assert_ne!(cap, 0);

    // For small tables we require at least 1 empty bucket so that lookups are
    // guaranteed to terminate if an element doesn't exist in the table.
    if cap < 15 {
        // Consider a small TableLayout like { size: 1, ctrl_align: 16 } on a
        // platform with Group::WIDTH of 16 (like x86_64 with SSE2). For small
        // bucket sizes, this ends up wasting quite a few bytes just to pad to
        // the relatively larger ctrl_align:
        //
        // | capacity | buckets | bytes allocated | bytes per item |
        // | -------- | ------- | --------------- | -------------- |
        // |        3 |       4 |              36 | (Yikes!)  12.0 |
        // |        7 |       8 |              40 | (Poor)     5.7 |
        // |       14 |      16 |              48 |            3.4 |
        // |       28 |      32 |              80 |            3.3 |
        //
        // In general, buckets * table_layout.size >= table_layout.ctrl_align
        // must be true to avoid these edges. This is implemented by adjusting
        // the minimum capacity upwards for small items. This code only needs
        // to handle ctrl_align which are less than or equal to Group::WIDTH,
        // because valid layout sizes are always a multiple of the alignment,
        // so anything with alignment over the Group::WIDTH won't hit this edge
        // case.

        // This is brittle, e.g. if we ever add 32 byte groups, it will select
        // 3 regardless of the table_layout.size.
        let min_cap = match (Group::WIDTH, table_layout.size) {
            (16, 0..=1) => 14,
            (16, 2..=3) | (8, 0..=1) => 7,
            _ => 3,
        };
        let cap = min_cap.max(cap);
        // We don't bother with a table size of 2 buckets since that can only
        // hold a single element. Instead, we skip directly to a 4 bucket table
        // which can hold 3 elements.
        let buckets = if cap < 4 {
            4
        } else if cap < 8 {
            8
        } else {
            16
        };
        ensure_bucket_bytes_at_least_ctrl_align(table_layout, buckets);
        return Some(buckets);
    }

    // Otherwise require 1/8 buckets to be empty (87.5% load)
    //
    // Be careful when modifying this, calculate_layout relies on the
    // overflow check here.
    let adjusted_cap = cap.checked_mul(8)? / 7;

    // Any overflows will have been caught by the checked_mul. Also, any
    // rounding errors from the division above will be cleaned up by
    // next_power_of_two (which can't overflow because of the previous division).
    let buckets = adjusted_cap.next_power_of_two();
    ensure_bucket_bytes_at_least_ctrl_align(table_layout, buckets);
    Some(buckets)
}
```

