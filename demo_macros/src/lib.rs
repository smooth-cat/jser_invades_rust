//! 第 11 集使用的过程宏。
//! 过程宏必须位于 `proc-macro = true` 的独立 crate 中，并在编译调用方时运行。

// 编译器通过这个标准库类型把调用方的 token 传给过程宏，再接收宏的输出。
use proc_macro::TokenStream;
// `quote!` 把接近普通 Rust 的代码模板转换成 `proc_macro2::TokenStream`。
use quote::quote;
// `syn` 负责把无结构的 token 解析为 Rust AST；这里只导入本文件需要的节点和解析宏。
use syn::{Data, DeriveInput, Fields, ItemFn, LitStr, parse_macro_input, parse_quote};

// 注册名为 `Describe` 的派生宏。
// `attributes(describe)` 允许被派生的类型使用 `#[describe(...)]` 辅助属性。
#[proc_macro_derive(Describe, attributes(describe))]
// 派生宏入口必须接收并返回编译器提供的 `proc_macro::TokenStream`。
pub fn derive_describe(input: TokenStream) -> TokenStream {
  // 把输入解析成 `DeriveInput`，它可以表示带 derive 的 struct、enum 或 union。
  // 解析失败时，`parse_macro_input!` 会提前返回带源码位置的 `compile_error!`。
  let input = parse_macro_input!(input as DeriveInput);
  // 把实际展开工作交给返回 `syn::Result` 的普通函数，方便统一处理错误。
  match expand_describe(input) {
    // `quote!` 产生的是 proc_macro2 类型；`.into()` 将其转成编译器要求的类型。
    Ok(tokens) => tokens.into(),
    // 将语义校验错误转换为调用方能看到、且带正确 span 的编译错误。
    Err(error) => error.to_compile_error().into(),
  }
}

// 输入已经是 AST，输出是待插回调用方源码的 token；失败时返回带 span 的 `syn::Error`。
fn expand_describe(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
  // 取出类型名，例如 `struct User` 中的 `User`；之后用于生成 `impl Describe for User`。
  let type_name = input.ident;
  // 默认展示标签就是类型名；辅助属性可以在后面覆盖它。
  let mut label = type_name.to_string();

  // 遍历类型上的全部属性，例如 `derive`、`describe`、`allow`。
  for attribute in &input.attrs {
    // 只处理路径恰好为 `describe` 的辅助属性，其他属性保持无关。
    if attribute.path().is_ident("describe") {
      // 按 Rust 属性的嵌套元数据语法解析 `#[describe(...)]` 中的每一项。
      attribute.parse_nested_meta(|meta| {
        // 当前宏只支持 `label` 这一项。
        if meta.path.is_ident("label") {
          // `meta.value()` 消费等号，`parse()` 再读取等号后的字符串字面量。
          let value: LitStr = meta.value()?.parse()?;
          // 将 `LitStr` 转成普通 String，作为生成代码里的展示标签。
          label = value.value();
          // 返回成功，允许解析器继续读取下一项元数据。
          Ok(())
        } else {
          // 未知配置项会在对应属性位置产生明确的编译错误。
          Err(meta.error("expected `label = \"...\"`"))
        }
        // `?` 把属性解析错误向上传给 `expand_describe`。
      })?;
    }
  }

  // 解构 AST，限定该学习宏只支持 struct；enum 和 union 会进入 `else`。
  let Data::Struct(data) = input.data else {
    // `new_spanned` 把错误定位到用户写下的类型名，而不是宏实现内部。
    return Err(syn::Error::new_spanned(
      // 作为报错位置的语法节点。
      type_name,
      // 展示给宏调用者的错误消息。
      "Describe only supports structs",
    ));
  };
  // 继续限定为具名字段 struct，例如 `{ id: u64 }`，不接受元组或单元 struct。
  let Fields::Named(fields) = data.fields else {
    // 同样把输入不符合要求的问题变成调用方的编译错误。
    return Err(syn::Error::new_spanned(
      // 仍将错误标在类型名上，方便调用者定位。
      type_name,
      // 说明本宏需要具名字段。
      "Describe requires named fields",
    ));
  };

  // 保留原类型的生命周期、类型参数、const 参数以及已有 where 条件。
  let mut generics = input.generics;
  // 生成代码会用 `{:?}` 格式化每个字段，因此逐个为字段类型增加 Debug 约束。
  for field in &fields.named {
    // 取出字段的类型 AST，例如 `name: String` 中的 `String`。
    let field_type = &field.ty;
    // 如果输入没有 where 子句就创建一个，如果已有则复用。
    generics
      .make_where_clause()
      // 取得 where 子句中的条件列表。
      .predicates
      // `parse_quote!` 将模板解析成 WherePredicate 后加入列表。
      .push(parse_quote!(#field_type: std::fmt::Debug));
  }
  // 将泛型拆成 impl、类型使用和 where 三部分，以放到正确的语法位置。
  let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

  // 为每个字段构造一条“字段名 + Debug 值”的代码；map 此时返回惰性迭代器。
  let describe_fields = fields.named.iter().map(|field| {
    // Named 字段按 syn 的不变量一定有 ident；expect 只用于声明这个不变量。
    let field_name = field.ident.as_ref().expect("named fields have identifiers");
    // 把字段标识符转成运行时展示的普通字符串，例如 `id` 变为 `"id"`。
    let field_label = field_name.to_string();
    // 为当前字段生成一段待插入 `describe` 方法体的 Rust token。
    quote! {
      // `#field_label` 插入字符串，`#field_name` 插入字段标识符。
      fields.push(format!("{}: {:?}", #field_label, &self.#field_name));
    }
  });

  // 用一个代码模板组装最终 trait 实现，并用 Ok 表示展开成功。
  Ok(quote! {
    // 三段泛型分别落在 impl、类型名和末尾 where 子句的位置。
    // `Describe` 从宏调用处解析，所以调用模块必须能访问同名 trait。
    impl #impl_generics Describe for #type_name #type_generics #where_clause {
      // 生成主库 `Describe` trait 要求的方法。
      fn describe(&self) -> String {
        // 收集每个字段格式化后的文本。
        let mut fields = Vec::new();
        // quote 的重复插值：为迭代器中的每个元素插入一次字段处理语句。
        #(#describe_fields)*
        // 将标签和全部字段拼成 `用户 { id: 7, name: "Lin" }`。
        format!("{} {{ {} }}", #label, fields.join(", "))
      }
    }
  })
}

// 注册名为 `trace` 的属性宏；它可以写在函数上作为 `#[trace]`。
#[proc_macro_attribute]
// 属性宏收到两个输入：属性括号里的参数，以及被属性标记的完整代码项。
pub fn trace(arguments: TokenStream, item: TokenStream) -> TokenStream {
  // 本 demo 的 `#[trace]` 不接受参数，所以先拒绝 `#[trace(...)]`。
  if !arguments.is_empty() {
    // 构造一个会显示在宏调用位置的 syn 错误。
    return syn::Error::new(
      // `call_site` 表示当前宏调用处；这里没有更精确的参数 AST span 可用。
      proc_macro2::Span::call_site(),
      // 告知调用者当前属性的合法写法只能是 `#[trace]`。
      "trace does not accept arguments",
    )
    // 把 syn 错误转换成 `compile_error!(...)` token。
    .to_compile_error()
    // 再把 proc_macro2 token 转成编译器 TokenStream 并提前返回。
    .into();
  }

  // 把被标记的代码解析成自由函数 AST；用于其他代码项时会产生编译错误。
  let function = parse_macro_input!(item as ItemFn);
  // 保存函数原有属性，以便输出时原样放回，例如 `#[allow(...)]`。
  let attributes = function.attrs;
  // 保存函数可见性，例如空、`pub` 或 `pub(crate)`。
  let visibility = function.vis;
  // 保存完整签名，包括函数名、参数、返回值、泛型和 async 等修饰符。
  let signature = function.sig;
  // 单独克隆函数名，因为它既要留在签名中，也要插入日志文本。
  let function_name = signature.ident.clone();
  // 取出原函数体，稍后包进闭包或异步块。
  let body = function.block;

  // async 函数和同步函数需要不同的包裹方式，但目标都是隔离原函数体中的 return。
  let invoke_body = if signature.asyncness.is_some() {
    // 外层仍是 async fn；创建并 await 一个异步块，之后才能继续打印 exit 日志。
    quote! { (async move #body).await }
  } else {
    // 闭包使原函数体中的 `return value` 只返回闭包结果，不会跳过 exit 日志。
    quote! { (|| #body)() }
  };

  // 生成一个签名不变、但函数体前后增加日志的新函数。
  quote! {
    // 重复插值恢复原函数的所有属性；即使没有属性也能正常展开为零次。
    #(#attributes)*
    // 恢复原可见性与完整签名，只替换花括号中的函数体。
    #visibility #signature {
      // `stringify!` 在编译期把插入的函数标识符变成字符串，不执行函数名表达式。
      println!("[trace] enter {}", stringify!(#function_name));
      // 执行前面生成的闭包调用或异步块，并保留原函数返回值。
      let __trace_result = #invoke_body;
      // 原函数体正常完成后打印退出日志。
      println!("[trace] exit {}", stringify!(#function_name));
      // 将原函数结果作为新函数的最终表达式返回，保持外部行为不变。
      __trace_result
    }
  }
  // 属性宏边界要求返回 proc_macro 类型，因此在最后完成类型转换。
  .into()
}
