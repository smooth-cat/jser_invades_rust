// FIXME: Replace with `core::hint::{likely, unlikely}` once they are stable.
#[cfg(feature = "nightly")]
pub(crate) use core::intrinsics::{likely, unlikely};

#[cfg(not(feature = "nightly"))]
#[inline(always)]
#[cold]
fn cold_path() {}

#[cfg(not(feature = "nightly"))]
#[inline(always)]
pub(crate) fn likely(b: bool) -> bool {
    if b {
        true
    } else {
        cold_path();
        false
    }
}

#[cfg(not(feature = "nightly"))]
#[inline(always)]
// 通过 cold_path 提升分支预测性能，在扩容场景中表示 扩容的场景比较少见
pub(crate) fn unlikely(b: bool) -> bool {
    if b {
        cold_path();
        true
    } else {
        false
    }
}
