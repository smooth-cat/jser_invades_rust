# JSer 入侵 Rust
# 第 13 集 异步编程

目标：理解 `Future`、`async/await`、运行时、task、`select!`、异步 channel、取消和 `Pin`，能够跟踪异步项目的执行流程。应用 tokio, 最后要实现一个极简的 tokio。



## Unpin

1. Unpin 是自动实现 Trait，大多数 Rust 数据都自动实现了 Unpin。
2. Unpin **默认** 和 Copy Trait 一样是传染性的：所有的字段都是 Unpin 整个数据才是 Unpin
3. 手动实现覆盖默认实现  `impl<T> Unpin for Ready<T> {}` ，Ready 被强制指定为 Unpin 类型不论内部是否全是 Unpin

## 生成 async/await 的 MIR

项目根目录的 `example.rs` 是一个最小 async/await 示例。使用下面的命令，可以将它单独编译并生成 `example.mir`：

```bash
rustc \
  --edition=2024 \
  --crate-type=lib \
  --emit=mir \
  example.rs \
  -o example.mir

rustc \
  --edition=2024 \
  --crate-type=lib \
  --emit=hir \
  example.rs \
  -o example.hir
```

生成后可在 `example.mir` 中搜索 `example::{closure#0}`、`Poll::Pending`、`Poll::Ready` 和 `discriminant`，观察 async 函数对应的 Future 状态机。

```
源代码
  ↓ 词法分析
TokenStream
  ↓ 过程宏/声明宏展开
展开后的 Rust 代码
  ↓
AST / HIR
  ↓
类型检查
  ↓
MIR
  ↓
机器码


TokenStream
    ↓ syn
结构化语法树 ItemFn
    ↓ 修改字段
新的语法结构
    ↓ quote
新的 TokenStream
    ↓ rustc
HIR / MIR / 机器码
```

