//! 08 - `Pin`：保证值在内存中的地址稳定，让自引用 future 可以安全地被 poll。
//!
//! 大多数类型实现 `Unpin`，可以像普通值一样移动；包含 `PhantomPinned` 的类型
//! 则不能移动。执行器通常把 future 放到 `Pin<Box<_>>`，之后只通过 `poll` 使用它。

use std::marker::PhantomPinned;
use std::pin::{Pin, pin};
use std::{mem, ptr};

#[tokio::main(flavor = "current_thread")]
pub async fn demo() {
  crate::section("08. Pin");

  /*----------------- 禁止移动，是禁哪些操作 -----------------*/
  // // 数据 替换【所有权】
  // mem::replace
  // // 内存取出 【所有权】
  // mem::take
  // // 数据 交换， 两个地址存的数据交换
  // mem::swap
  // // 将裸指针指向内存区域以 X 类型读到栈上，禁不禁都一样，因为 ptr::read 只能在 unsafe 里调用
  // ptr::read<X>(target.as_ptr())

  crate::section("08.1. Unpin");
  // rust 默认 实现了 Unpin
  let _foo: Box<dyn Unpin> = Box::new(10);
  // 不合理，非固定类型无法转换成 固定类型
  // let _foo2: Pin<Box<i32>> = _foo.into();

  // 固定类型，因为 _pin 是固定类型
  #[derive(Debug)]
  struct Fixed {
    num: String,         // Unpin
    _pin: PhantomPinned, // 固定类型，未实现 Unpin
  }
  // 不是 Unpin 报错
  // let _x: Box<dyn Unpin> = Box::new(PhantomPinned {});

  /*----------------- 只声明 PhantomPinned 不够 -----------------*/
  let mut x = Fixed {
    num: "10".into(),
    _pin: PhantomPinned,
  };
  let mut y = Fixed {
    num: "20".into(),
    _pin: PhantomPinned,
  };

  x.num = "30".into();
  mem::swap(&mut x, &mut y);

  /*----------------- 只有使用 Pin + PhantomPinned -----------------*/
  let mut _x = unsafe { Pin::new_unchecked(&mut x) };

  let mut _y = unsafe { Pin::new_unchecked(&mut y) };

  // 此时借用 &mut x 存在于 _x 中，不能再移动
  // let g = x;

  // 交换 pin ✅
  mem::swap(&mut _x, &mut _y);

  // 钉住堆地址
  // `PhantomPinned` cannot be unpinned ⭕️
  // let mut mut_a = _x.get_mut();

  // 已经被借用了无法执行修改 ⭕️
  // x.num = "40".into();

  // pin.rs 搜 Deref for ->  &x ✅
  println!("{}", _x.num);

  // 常规写法
  let fixed_box = Box::pin(Fixed {
    num: "10".into(),
    _pin: PhantomPinned,
  });

  // 没实现 DerefMut 无法修改 ⭕️
  // fixed_box.num = "20".into();

  // `PhantomPinned` cannot be unpinned ⭕️
  // let mut_v = fixed_box.as_mut().get_mut();

  println!("{}", fixed_box.num);
}

fn demo_pinned() {
  struct PinnedSelfRef {
    text: String,
    text_ptr: *const String,
    _pin: PhantomPinned, // 让结构体变成 !Unpin
  }

  impl PinnedSelfRef {
    fn new(text: &str) -> Pin<Box<Self>> {
      // 在堆上固定对象，之后它的地址不会改变
      let mut boxed = Box::pin(PinnedSelfRef {
        text: text.into(),
        text_ptr: ptr::null(),
        _pin: PhantomPinned,
      });

      let text_ptr = &boxed.text as *const String;

      // 构造自引用仍然需要 unsafe，但对象已经被固定，不会再移动
      unsafe {
        let mut_ref = boxed.as_mut();
        mut_ref.get_unchecked_mut().text_ptr = text_ptr;
      }

      boxed
    }

    fn get(self: Pin<&Self>) -> &String {
      // 因为对象被 Pin 住，text_ptr 始终有效
      unsafe { &*self.text_ptr }
    }
  }

  let pinned = PinnedSelfRef::new("world");

  println!("[使用 Pin] 创建后:");
  println!("  pinned.text 地址: {:p}", &pinned.text as *const String);
  println!("  pinned.text_ptr 指向: {:p}", pinned.text_ptr);

  assert_eq!(&pinned.text as *const String, pinned.text_ptr);

  let value = pinned.as_ref().get();
  println!("  get() = {}", value);

  // 下面这行不能编译：PinnedSelfRef 是 !Unpin，
  // 不能从 Pin<Box<Self>> 中把对象移出来。
  //
  // let moved = *pinned;

  println!("  => 对象被 Pin 住，无法移动，自引用指针保持有效\n");
}
