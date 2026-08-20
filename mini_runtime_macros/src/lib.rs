//! `mini_runtime` 的两个教学属性宏。
//!
//! 它们展示了 `#[tokio::main]` / `#[tokio::test]` 的核心思路：把 async 函数体
//! 包进 async block，再由运行时的 `block_on` 驱动。为了保持示例简单，这里暂不
//! 支持 Tokio 的 `flavor`、`worker_threads` 等配置参数。

// 从编译器传入、并返回给编译器的过程宏 token 流类型。
use proc_macro::TokenStream;
// 用 `quote!` 根据已有语法树拼出新的 Rust 代码。
use quote::quote;
// `Error` 用于生成编译错误，`ItemFn` 表示一个函数，`parse_macro_input` 负责解析输入。
use syn::{Error, ItemFn, parse_macro_input};

/// 将 `async fn` 改写成创建 `MiniRuntime` 并 `block_on` 的同步函数。
#[proc_macro_attribute]
pub fn main(arguments: TokenStream, item: TokenStream) -> TokenStream {
  // 属性宏的第一个参数是 `#[mini_runtime::main(...)]` 中括号内的参数。
  // 本教学宏不支持任何参数，因此先统一检查并拒绝非空参数。
  if let Some(error) = reject_arguments(arguments, "main") {
    // 返回编译错误 token；返回后不会继续解析或展开原函数。
    return error;
  }

  // 把调用方传入的 token 解析成函数语法树，并交给通用展开逻辑。
  // `false` 表示这是运行时入口，而不是测试函数。
  expand_runtime_function(parse_macro_input!(item as ItemFn), false)
}

// 检查属性是否带有参数，并在带参数时构造一个可显示给用户的编译错误。
fn reject_arguments(arguments: TokenStream, macro_name: &str) -> Option<TokenStream> {
  // `TokenStream::is_empty` 表示属性括号内没有任何 token，即没有传参。
  if arguments.is_empty() {
    // 没有参数是合法情况，用 `None` 表示检查通过。
    None
  } else {
    // 有参数时，构造并返回错误 token，让编译器在调用位置报告问题。
    Some(
      // 使用调用点的 span，使错误尽量指向用户写属性的位置。
      Error::new(
        proc_macro2::Span::call_site(),
        // 根据宏名称生成对应的中文错误信息。
        format!("#[mini_runtime::{macro_name}] 不接受参数"),
      )
      // 将 `syn::Error` 转成编译器可以处理的 `compile_error!` token。
      .to_compile_error()
      // `proc_macro2::TokenStream` 与过程宏的 `TokenStream` 类型不同，这里转换回后者。
      .into(),
    )
  }
}

// 负责把一个函数改写成由 `MiniRuntime` 驱动的同步函数。
// `test` 为 true 时额外添加标准库的 `#[test]` 属性。
fn expand_runtime_function(function: ItemFn, test: bool) -> TokenStream {
  // 属性宏只接受 async 函数；普通同步函数没有可供运行时驱动的异步体。
  if function.sig.asyncness.is_none() {
    // `new_spanned` 把错误 span 绑定到原函数的 `fn` 关键字，方便定位。
    return Error::new_spanned(function.sig.fn_token, "#[mini_runtime] 属性只能用于 async fn")
      // 生成 `compile_error!`，并转换成过程宏需要返回的 token 流。
      .to_compile_error()
      .into();
  }

  // 为了让生成的入口函数保持简单，这里禁止函数参数。
  if !function.sig.inputs.is_empty() {
    // 将错误 span 绑定到参数列表，让用户直接看到不被支持的部分。
    return Error::new_spanned(&function.sig.inputs, "极简运行时入口函数不能接收参数")
      .to_compile_error()
      .into();
  }

  // 取出原函数上的所有属性，稍后原样放回生成的函数前面。
  let attributes = function.attrs;
  // 保存原函数的可见性（例如 `pub`）。
  let visibility = function.vis;
  // 取出函数签名并声明为可变，后面需要删除 `async` 标记。
  let mut signature = function.sig;
  // 生成函数必须是同步函数，因为它内部会主动调用 `block_on`。
  signature.asyncness = None;
  // 保存原函数体；它会被放进下面创建的 `async move` block 中。
  let body = function.block;

  // 测试宏需要标准 `#[test]` 属性；普通入口宏则不添加任何属性。
  let test_attribute = test.then(|| quote! { #[test] });

  // 使用 `quote!` 拼接展开后的 Rust 代码。
  quote! {
    // 保留用户原先写在函数上的属性（例如文档或条件编译属性）。
    #(#attributes)*
    // 仅在 `test == true` 时展开为 `#[test]`。
    #test_attribute
    // 复用原可见性和签名，但签名中的 `async` 已在上面移除。
    #visibility #signature {
      // 创建一个极简运行时，把原异步函数体包装成 `async move` 后交给 `block_on`。
      let mut even_loop = ::mini_runtime::runner::MyLoop::new();
      even_loop.add_async(Box::pin(async move #body), |_| {});
      even_loop.run();
    }
  }
  // 将 `quote!` 生成的 token 流转换成过程宏要求的返回类型。
  .into()
}

/// 将异步测试函数改写成标准 `#[test]`，并由 `MiniRuntime` 驱动。
#[proc_macro_attribute]
pub fn test(arguments: TokenStream, item: TokenStream) -> TokenStream {
  // 测试属性与入口属性一样，不允许任何括号参数。
  if let Some(error) = reject_arguments(arguments, "test") {
    // 参数非法时直接返回编译错误。
    return error;
  }

  // 解析原始函数，并将 `true` 传给展开器以附加 `#[test]`。
  expand_runtime_function(parse_macro_input!(item as ItemFn), true)
}
