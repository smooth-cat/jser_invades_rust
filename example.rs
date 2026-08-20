//! 用于观察 async/await 如何被 Rust 编译器降低为 Future 状态机的最小示例。

/// 返回一个立即完成的异步计算。
pub async fn foo() -> i32 {
    10
}

/// 使用前一个异步计算的结果，再返回一个立即完成的异步计算。
pub async fn bar(a: i32) -> i32 {
    a + 1
}

/// 包含两个 await 点，便于在 MIR 中观察状态切换。
pub async fn example(x: i32) -> i32 {
    let a = foo().await;
    let b = bar(a).await;
    b + x
}
