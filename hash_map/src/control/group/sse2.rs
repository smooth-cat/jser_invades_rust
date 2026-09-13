use super::super::{BitMask, Tag};
use core::num::NonZeroU16;

#[cfg(target_arch = "x86")]
use core::arch::x86;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as x86;

pub(crate) type BitMaskWord = u16;
pub(crate) type NonZeroBitMaskWord = NonZeroU16;
pub(crate) const BITMASK_STRIDE: usize = 1;
pub(crate) const BITMASK_ITER_MASK: BitMaskWord = !0;

/// Abstraction over a group of control tags which can be scanned in
/// parallel.
///
/// This implementation uses a 128-bit SSE value.
#[derive(Copy, Clone)]
pub(crate) struct Group(x86::__m128i);

// FIXME: https://github.com/rust-lang/rust-clippy/issues/3859
#[expect(clippy::use_self)]
impl Group {
    /// Number of bytes in the group.
    pub(crate) const WIDTH: usize = size_of::<Self>();

    /// Returns a full group of empty tags, suitable for use as the initial
    /// value for an empty hash table.
    ///
    /// This is guaranteed to be aligned to the group size.
    #[inline]
    pub(crate) const fn static_empty() -> &'static [Tag; Group::WIDTH] {
        #[repr(C)]
        struct AlignedTags {
            _align: [Group; 0],
            tags: [Tag; Group::WIDTH],
        }
        const ALIGNED_TAGS: AlignedTags = AlignedTags {
            _align: [],
            tags: [Tag::EMPTY; Group::WIDTH],
        };
        &ALIGNED_TAGS.tags
    }

    /// Loads a group of tags starting at the given address.
    #[inline]
    pub(crate) unsafe fn load(ptr: *const Tag) -> Self {
        unsafe { Group(x86::_mm_loadu_si128(ptr.cast())) }
    }

    /// Loads a group of tags starting at the given address, which must be
    /// aligned to `align_of::<Group>()`.
    #[inline]
    pub(crate) unsafe fn load_aligned(ptr: *const Tag) -> Self {
        debug_assert_eq!(ptr.align_offset(align_of::<Self>()), 0);
        unsafe { Group(x86::_mm_load_si128(ptr.cast())) }
    }

    /// Stores the group of tags to the given address, which must be
    /// aligned to `align_of::<Group>()`.
    #[inline]
    pub(crate) unsafe fn store_aligned(self, ptr: *mut Tag) {
        debug_assert_eq!(ptr.align_offset(align_of::<Self>()), 0);
        unsafe {
            x86::_mm_store_si128(ptr.cast(), self.0);
        }
    }

    /// Returns a `BitMask` indicating all tags in the group which have
    /// the given value.
    /*
      1. #[expect(...)]：抑制 clippy 关于 as i8、as u16 可能截断/符号变化的警告。
      2. unsafe { ... }：调用 x86 SIMD intrinsics，需要 unsafe。
      3. x86::_mm_set1_epi8(tag.0 as i8)：把 tag.0 强转为 i8，再广播成 16 个相同字节的 __m128i。
      4. x86::_mm_cmpeq_epi8(self.0, 广播值)：将 self.0 的 16 个字节与广播值逐字节比较相等。
      5. 相等：该字节变成 0xFF
      6. 不相等：该字节变成 0x00
      7. x86::_mm_movemask_epi8(cmp)：取 cmp 中 16 个字节的最高位，组合成一个 16 位掩码，返回 i32（高 16 位为 0）。
      8. as u16：把 i32 截断为 u16，得到 16 位掩码。
      9. BitMask(...)：将 16 位掩码包装成 BitMask。
      10. 最终结果：返回的 BitMask 中，第 i 位为 1 表示 self.0 的第 i 个字节等于 tag；为 0 表示不等。
     */
    #[inline]
    pub(crate) fn match_tag(self, tag: Tag) -> BitMask {
        #[expect(
            clippy::cast_possible_wrap, // tag.0: Tag as i8
            // tag: i32 as u16
            //   note: _mm_movemask_epi8 returns a 16-bit mask in a i32, the
            //   upper 16-bits of the i32 are zeroed:
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation
        )]
        unsafe {
            let cmp = x86::_mm_cmpeq_epi8(self.0, x86::_mm_set1_epi8(tag.0 as i8));
            BitMask(x86::_mm_movemask_epi8(cmp) as u16)
        }
    }

    /// Returns a `BitMask` indicating all tags in the group which are
    /// `EMPTY`.
    #[inline]
    pub(crate) fn match_empty(self) -> BitMask {
        self.match_tag(Tag::EMPTY)
    }

    /// Returns a `BitMask` indicating all tags in the group which are
    /// `EMPTY` or `DELETED`.
    #[inline]
    pub(crate) fn match_empty_or_deleted(self) -> BitMask {
        #[expect(
            // tag: i32 as u16
            //   note: _mm_movemask_epi8 returns a 16-bit mask in a i32, the
            //   upper 16-bits of the i32 are zeroed:
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation
        )]
        unsafe {
            // A tag is EMPTY or DELETED iff the high bit is set
            BitMask(x86::_mm_movemask_epi8(self.0) as u16)
        }
    }

    /// Returns a `BitMask` indicating all tags in the group which are full.
    #[inline]
    pub(crate) fn match_full(&self) -> BitMask {
        BitMask(!self.match_empty_or_deleted().0)
    }

    /// Performs the following transformation on all tags in the group:
    /// - `EMPTY => EMPTY`
    /// - `DELETED => EMPTY`
    /// - `FULL => DELETED`
    #[inline]
    pub(crate) fn convert_special_to_empty_and_full_to_deleted(self) -> Self {
        // Map high_bit = 1 (EMPTY or DELETED) to 1111_1111
        // and high_bit = 0 (FULL) to 1000_0000
        //
        // Here's this logic expanded to concrete values:
        //   let special = 0 > tag = 1111_1111 (true) or 0000_0000 (false)
        //   1111_1111 | 1000_0000 = 1111_1111
        //   0000_0000 | 1000_0000 = 1000_0000
        #[expect(
            clippy::cast_possible_wrap, // tag: Tag::DELETED.0 as i8
        )]
        unsafe {
            let zero = x86::_mm_setzero_si128();
            let special = x86::_mm_cmpgt_epi8(zero, self.0);
            Group(x86::_mm_or_si128(
                special,
                x86::_mm_set1_epi8(Tag::DELETED.0 as i8),
            ))
        }
    }
}
