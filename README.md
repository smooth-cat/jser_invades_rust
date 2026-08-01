# JSer 入侵 Rust
# 第 6 集 所有权、借用、生命周期

## 所有权

```rust
let a = 123; // a 是 123 的所有者
```

**每一个值有且仅有一个所有者，当所有者离开作用域时，该值就会被自动释放。**

### 变量什么时候离开？

1. 运行时 函数、代码块、闭包 执行完成
2. 编译时 **变量的最后一次使用**

## 所有权是 编译时 概念

所有权只是 rust 借用检查器在编译时的概念，**类比于 TS 的 类型限制**

TS 中 **目的：正确地处理数据**

1. `interface Foo { a: string }`  此时 `foo.b` 报错，实际上可通过 `foo['b']` 绕开类型限制，但得到的值可能是 `undefined`
2. `interface Foo { a ?: string }` 限制一定要处理 a 是空的情况, `foo.a.trim()` 会报错，必须 `foo.a?.trim?.()`

Rust 所有权、借用检查、生命周期标注, **目的：内存正确释放、无数据竞争**

## 所有权转移

### 两层含义

1. 运行时：拷贝栈内存 memcpy

2. 编译时：所有权变更，编译器认为 `a` 不可用

   ```rust
   let a = String::from("123"); // a 是 字符串的所有者
   let b = a;                   // String 没 Copy Trait(标志)，字符串所有权归 b， a 不可用
   
   let c = 21; // i32 有 Copy Trait(标志)
   let d = c;  // 编译器认为这里是 复制所有权语义
   ```

   > Copy Trait 只是编译时标志(无具体实现) 让编译器知道 该数据在赋值语句中是 **复制所有权 还是 所有权转移**
   >
   > **它只是一个标志，不改变任何运行时行为**
   >
   > 实现 Copy: Clone Trait 的数据在赋值语句并不会调用 clone 方法，而是 **仍然使用 memcpy 进行拷贝**

## 借用

### 两层含义

1. 运行时：就是指针，保存了数据所在的地址
2. 编译时：可以通过指针对数据做 读写，但不具备所有权

### 两种借用

1. 读指针（只读）
   ```rust
   let a = 123;
   let ptr = &a; // 可读取数字，但不能做赋值操作
   ```

2. 写指针（完全操作权，**同时只能存在一个**）
   ```rust
   let mut a = 123;
   let mut ptr = &mut a; // 完全操作权限
   ```

# 限制：读写互斥，写唯一

## 读写互斥

1. **创建指针是 读写互斥的**

   ```rust
     let mut num = 10;
     let mut write = &mut num;
     let read = &num;      // 报错：已经有写指针 不能创建读指针
     println!("{}", write) // write 存活到最后一次使用
   ```

2. **所有者读写操作 与 指针类型互斥**

   ```rust
     let mut num = 10;
     let mut write = &mut num;
     let num2 = num; // 报错：有写指针时 owner 不可读，也不可写(写唯一)
     *write = 30;    // write 存活到最后一次使用
   ```

## 生命周期(指针的生命周期)

### 什么时候需要？

1. 结构体、枚举，内部存在指针 -> **避免 悬垂指针**
2. 函数、方法、如果返回值是 指针
   生命周期标注 -> **函数/方法 返回的指针 要么来自参数，要么来自方法。需要准确告诉编译器具体是哪个指针**

### 生命周期的隐式转换

```rust
/* 
	表示 x,y,返回值 为 'a 生命周期
	'a 生命周期由参数决定
*/
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
  if x.len() > y.len() { x } else { y }
}

pub fn life_shrink() {
  let s1 = String::from("long string is long");
  let result;
  {
    let s2 = String::from("xyz");
    // 's1 > 's2 
    // 调用时会将 s1 的生命周期视为 's2 以满足入参生命周期相同的约束
    result = longest(s1.as_str(), s2.as_str());
  }
  // println!("The longest string is {}", result);
  println!("s1: {}", s1);
}
```

### 生命周期可以**类比于 TS 的 泛型**

1. TS 泛型

   ```typescript
   // 返回值类型由  入参 input 类型决定
   const noop = <T>(input: T): T => input
   ```

2. Rust 函数生命周期
   ```rust
   // 返回值生命周期由 入参 input 生命周期决定
   fn noop <'a>(input: &'a str) -> &'a str { 
     input
   }
   ```

3. Rust 结构体生命周期
   ```rust
   struct TempPerson<'a> {
     name: &'a str,
     age: i32,
   }
   
   let name = String::from("Jay");
   // 有 'a 后借用检查器 才知道 person 活不过 &name 指针
   let person = TempPerson{ name: &name, age: 18 };
   ```

4. Trait 实现时生命周期
   ```rust
   // 定义带生命周期的泛型
   trait Name<'a> {
     fn get_name (&self) -> &'a str;
   }
   // 实现时将泛型 'b 传给 trait 和 TempPerson 即可
   impl<'b> Name<'b> for TempPerson<'b> {
   	fn get_name (&self) -> &'b str {
       return &self.name;
     }
   }
   ```

5. 闭包生命周期

   1. 返回值来自参数 ， 表示闭包能处理任意生命周期，并返回相同生命周期
      ```rust
      // 实现了 Fn Trait 
      fn apply<F: for<'a> Fn(&'a str) -> &'a str>(f: F, s: &str) -> &str {
        f(s)
      }
      // 可以简写为
      fn apply(f: impl Fn(&str) -> &str, s: &str) -> &str {
        f(s)
      }
      let trimmed = apply(|s| s.trim(), "  hello  ");
      ```

   2. 返回值来自捕获数据
      ```rust
      fn apply<'a>(f: impl Fn() -> &'a str) -> &'a str {
        f(s)
      }
      let data = String::from("hello");
      let f = || data.as_str(); // 返回 &str 的生命周期绑定的是 data，不是参数
      ```

   3. struct 存储闭包
      ```rust
      struct Getter<'a> {
        // 闭包至少活 'a 这么长
        f: Box<dyn Fn(&'a str, &'a str) -> &'a str>,
      }
      pub fn closure_var() {
        let f: Box<dyn for<'a> Fn(&'a str, &'a str) -> &'a str> =
          Box::new(|a, b| {
            if a.len() > b.len() { a } else { b }
          });
        let getter = Getter { f: f };
        // 使用 getter.f 表示字段。这种写法与 impl 定义的方法分开来保证语义准确
        // 这里指调用 结构体中 f 字段存储的 闭包
        (getter.f)("hello", "world!");
      }
      ```


# 总结

这些概念，本质上是 Rust 把 **内存安全的编码规范** 写入了编译器之中。
