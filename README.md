# JSer 入侵 Rust
# 第 8 集 智能指针

目标：讲清 `Box`、`Rc`、`Weak`、`Arc`、`Cell`、`RefCell` 以及 `Deref`、`Drop`，理解不同所有权和内部可变性方案的使用场景。

## 智能指针实用知识点

### 1. 智能指针是什么

指针：存的是地址，而不是值本身（普通引用 `&T` 就是指针）。智能指针：带额外行为和元数据的"结构体指针"。

- **智能指针 vs 普通引用**：普通引用只借用（`&`），智能指针拥有数据（`Box<T>` 拥有堆数据的所有权）
- **两个核心 Trait**：`Deref`（决定 `*` 解引用行为和自动解引用）、`Drop`（决定离开作用域时如何清理）

### 2. Deref 与 Drop（智能指针的两大支柱）

- **Deref**：实现后 `*x` 拿到内部值；方法调用时自动解引用（`s.len()` 实际上 `(*s).len()`）
- **自动解引用链**：`&String` → `&str` → `str`，编译器按需逐层解引用找到方法（类似 JS 原型链查找，但是编译期完成）
- **`DerefMut`**：可变解引用，只有 `&mut` 场景才需要，要求数据本身可变借用
- **Drop**：离开作用域自动调用 `drop(self)` 释放资源（内存、文件、连接），所有权模型下不会重复释放（JS 的 `finally` 是手动的）
- **显式提前释放**：`drop(x)` 手动调用（等价于 `mem::drop`），提前结束借用再继续操作

### 3. Box<T>：单一所有权的堆分配

- **做什么**：把值从栈挪到堆，栈上只留一个指针；`Box::new(1)` 类比 JS `let x = new Number(1)`（堆对象 + 栈引用）
- **递归类型**：`enum List { Nil, Cons(i32, Box<List>) }`，没有 Box 编译器拒绝（无限大小），Box 让大小固定为指针大小
- **Trait 对象**：`Box<dyn Draw>` 把不确定大小的 trait 实现装箱，动态分发（类比 JS 多态）
- **大数据转移**：`Box::new(big)` 移动时只复制指针，不深拷贝整份数据
- **小知识**：`*boxed = 5` 可写入，`&*boxed` 得到内部引用；Rust 1.80 起 `box expr` 语法渐趋稳定（`box` 关键字）

### 4. Rc<T>：单线程共享所有权（引用计数）

- 做什么：多个变量共享同一份数据，计数归零才释放；`Rc::clone(&rc)` 增加计数（注意不是深拷贝）
- 强计数 `strong_count`：`Rc::strong_count(&rc)` 查看当前引用数
- ***只读共享***：**`Rc<T>` 里 `T` 不可变；要改数据必须配合 `RefCell`**
- 限制：不能跨线程（不是 `Send`/`Sync`），线程之间只能用 `Arc`

### 5. Weak<T>：弱引用，打破循环

- **做什么**：`Rc::downgrade(&rc)` 得到 `Weak<T>`，**不增加强计数**，不阻止释放
- **`upgrade()`**：`weak.upgrade()` 返回 `Option<Rc<T>>`，数据已释放则 `None`（类比 JS 弱引用 `WeakRef` + `deref()`）
- **循环引用泄漏**：两个 `Rc` 互指时强计数永不归零 → 内存泄漏；必须把其中一环换成 `Weak`（如树：父 `Weak` 指子，子 `Rc` 指父，或反之）
- **使用场景**：父节点引用子节点用 `Rc`，子节点回指父节点用 `Weak`；缓存对象可随时清空

### 6. RefCell<T>：内部可变性

- **RefCell<T>**： 适合非 Copy 类型；`borrow()` 返回 `Ref`，`borrow_mut()` 返回 `RefMut`，**运行时**检查借用规则
- **RefCell<T>**： 只让它直接包裹的 `T` 可变，不会自动让 `T` 内部指针指向的其他对象也可变。
- **RefCell<T>**： 不会把内部数据放堆上 
- **`Rc<RefCell<T>>` 组合拳**：可变的共享数据（JS 里任何对象默认如此；Rust 需要显式组合）

### 7. Cell<T> 比较少用

- 用于 number 或 bool 类型
- **Cell<T>**：只适合 `Copy` 类型；`cell.get()` / `cell.set(v)` / `cell.replace(v)`，无 borrow 检查，最快
- 通过 get set 实现取值和设置 当然都是拷贝

### 8. 使用场景速查表

| 需求 | 单线程 | 多线程 |
|---|---|---|
| 独占可变（所有权唯一） | `Box<T>` | `Box<T>`（move 进线程） |
| 共享只读 | `Rc<T>` | `Arc<T>` |
| 共享可变 | `Rc<RefCell<T>>` | `Arc<Mutex<T>>` |
| 回指/缓存（不参与存活） | `Weak<T>` | `Weak<T>`（`Arc::downgrade`） |
| 栈上小数据仅可变 | `Cell<T>`（Copy） | 不可共享，用 `Mutex` |
| 局部可变借用绕检查 | `RefCell<T>` | 不可共享，用 `Mutex` |

- **选型口诀**：先问"要不要共享所有权"→ 否用 `Box`；是→ 再问"要不要跨线程"→ 单线程 `Rc`、多线程 `Arc`；最后问"要不要变"→ 要变就包一层 `Cell`/`RefCell`（或 `Mutex`）
- **JS 习惯对照**：JS 万物共享可写 → Rust 最接近的写法是 `Rc<RefCell<T>>`；但多线程共享可变在 JS 是单线程天然安全，Rust 必须 `Arc<Mutex<T>>`

### 9. 常见坑

- **坑 1**：递归类型不加 `Box` 编译报错 `recursive type has infinite size`；用 `Box` 打破无限递归
- **坑 2**：`Rc<T>` 传线程报错 `Rc cannot be sent between threads safely`；换 `Arc`
- **坑 3**：两个 `Rc` 循环引用 → `strong_count` 永不归零 → 内存泄漏；`dbg!(Rc::strong_count)` 排查，用 `Weak` 破环
- **坑 4**：`RefCell` 同时 `borrow` 和 `borrow_mut` 运行时 `panic`，不是编译期错误；可用 `try_borrow_mut()` 返回 `Result` 代替硬崩溃
- **坑 5**：`weak.upgrade()` 返回 `None` 时解引用会 panic（`None.unwrap()` 炸）；先 `if let Some(rc) = weak.upgrade()`
- **坑 6**：`Cell` 只能放 `Copy` 类型；放 `String` 编译报错，换 `RefCell`
- **坑 7**：`Rc`/`Arc` 的 `clone` 是浅拷贝（共享数据），不要误以为深拷贝；要独立数据用 `T::clone`（deep）或新建
- **坑 8**：`RefCell` 的 `Ref` 借用未释放就继续 `borrow_mut` → panic；借用变量提前 `drop` 或用作用域包裹

### 10. Arc<T>：多线程共享所有权

- **做什么**：`Rc` 的线程安全版（原子操作计数），`Arc = Atomic Rc`
- **必须 `Send + Sync`**：`Arc<T>` 要求 `T: Send + Sync` 才能共享；`Arc<RefCell<T>>` 不能跨线程，要用 `Arc<Mutex<T>>`（锁，下一集细讲）
- **性能代价**：原子计数比普通计数慢（单线程用 `Rc` 更合适）
- **线程示例**：`Arc::clone` 后 `move` 进 `thread::spawn`，各线程持有一份强引用
