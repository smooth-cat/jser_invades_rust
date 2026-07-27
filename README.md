# JSer 入侵 Rust

## 第 3 集：Struct 和 Trait

```bash
cargo run
```

## 核心概念

- `struct` 保存数据，`impl` 为具体类型实现方法，`trait` 定义多个类型共享的行为。
- `&self` 是只读借用，`&mut self` / `&mut T` 是独占可变借用。
- Rust 没有类继承；共享数据和逻辑通常用组合，共享行为通常用 trait。

## 三种英雄建模

### Trait 直写（`moba_direct.rs`）

- `Support`、`Assassin` 各自保存数据并实现 `Hero`。
- `impl Hero` 是泛型静态分发：编译期确定具体类型并单态化。
- `dyn Hero` 是动态分发：运行时通过 vtable 调用，可用 `Vec<Box<dyn Hero>>` 保存不同具体类型。
- trait 的泛型方法默认不能进入 vtable；可改用 `dyn Hero` 参数，或用 `where Self: Sized` 排除该方法。
- 类型集合固定时，可用 enum 包装不同类型，再通过 `match` 做静态分发。

### 组合（`moba_composite.rs`）

- `Support`、`Assassin` 内嵌 `Hero`，复用生命值和普通攻击逻辑。
- 公共能力通过字段显式转发，角色专属能力留在各自的 `impl` 中。

### 泛型组合（`moba_generic.rs`）

- `Hero<Role>` 把公共数据与角色数据组合在一个类型中。
- `impl Hero<Support>`、`impl Hero<Assassin>` 只为对应角色开放专属方法。
- 不同 `Hero<Role>` 是不同类型；需要混合存储时可再用 enum 统一。

## ECS 与 Sparse Set（`main.rs`）

- Entity 只是 ID，Component 只保存数据，System 负责行为。
- 每种组件使用 `sparse + dense + entities`：稀疏索引负责定位，连续数组负责缓存友好的遍历。
- 插入、查询和 `swap-remove` 删除接近 `O(1)`；删除会改变 dense 顺序。
- 批量 System 先收集 Entity，再可变访问组件，可避免同时借用 `World` 的冲突。

## 选择建议

- 类型在编译期已知：优先泛型或 enum 静态分发。
- 类型只能在运行时确定，或需要开放扩展：使用 `dyn Trait`。
- 多个角色共享状态而行为差异有限：优先组合；数据量大且需要批量处理：考虑 ECS。
