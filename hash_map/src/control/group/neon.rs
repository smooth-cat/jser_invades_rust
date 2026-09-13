use super::super::{BitMask, Tag};
use core::arch::aarch64 as neon;
use core::num::NonZeroU64;

pub(crate) type BitMaskWord = u64;
pub(crate) type NonZeroBitMaskWord = NonZeroU64;
pub(crate) const BITMASK_STRIDE: usize = 8;
pub(crate) const BITMASK_ITER_MASK: BitMaskWord = 0x8080_8080_8080_8080;

/// Abstraction over a group of control tags which can be scanned in
/// parallel.
///
/// This implementation uses a 64-bit NEON value.
#[derive(Copy, Clone)]
pub(crate) struct Group(neon::uint8x8_t);

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
        unsafe { Group(neon::vld1_u8(ptr.cast())) }
    }

    /// Loads a group of tags starting at the given address, which must be
    /// aligned to `align_of::<Group>()`.
    #[inline]
    pub(crate) unsafe fn load_aligned(ptr: *const Tag) -> Self {
        debug_assert_eq!(ptr.align_offset(align_of::<Self>()), 0);
        unsafe { Group(neon::vld1_u8(ptr.cast())) }
    }

    /// Stores the group of tags to the given address, which must be
    /// aligned to `align_of::<Group>()`.
    #[inline]
    pub(crate) unsafe fn store_aligned(self, ptr: *mut Tag) {
        debug_assert_eq!(ptr.align_offset(align_of::<Self>()), 0);
        unsafe {
            neon::vst1_u8(ptr.cast(), self.0);
        }
    }

    /// Returns a `BitMask` indicating all tags in the group which *may*
    /// have the given value.
    /*
      self.0 应该是一个 uint8x8_t，也就是 8 个 u8 的 SIMD 向量。
      neon::vdup_n_u8(tag.0) 把单个 u8 的 tag 广播成 8 个相同的 u8。
      neon::vceq_u8(a, b) 对 8 个 lane 逐字节比较是否相等：
      相等：该 lane 变成 0xFF
      不相等：该 lane 变成 0x00
    */
    #[inline]
    pub(crate) fn match_tag(self, tag: Tag) -> BitMask {
        unsafe {
            let cmp = neon::vceq_u8(self.0, neon::vdup_n_u8(tag.0));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
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
    /// 匹配最高位是 1 的字节
    /// 最终返回的结果：匹配的是 FF ，未匹配的是 0
    /// MaskBit 会通过 iter 直接拿到匹配的字节小端序 index
    #[inline]
    pub(crate) fn match_empty_or_deleted(self) -> BitMask {
        // 调用 NEON intrinsics 是 unsafe 的：需要目标平台支持 NEON，并满足相关安全前提
        unsafe {
            // self.0 原本是 uint8x8_t：8 个无符号 8 位整数，也就是 8 个字节。
            //
            // neon::vreinterpret_s8_u8(self.0)
            //   按位重新解释为 int8x8_t，也就是把同样 8 个字节当作有符号 i8。
            //   这只是类型层面的重新解释，不会改变任何位。
            //
            // neon::vcltz_s8(...)
            //   对每个 i8 lane 做 “compare less than zero”：
            //   如果该 lane < 0，则对应结果字节为 0xFF；
            //   否则为 0x00。
            //
            // 对原 u8 来说，什么时候重解释成 i8 会小于 0？
            // 就是最高位为 1 的时候，即原值在 0x80..=0xFF 范围内。
            let cmp = neon::vcltz_s8(neon::vreinterpret_s8_u8(self.0));

            // cmp 是 uint8x8_t，里面每个字节是 0xFF 或 0x00。
            //
            // neon::vreinterpret_u64_u8(cmp)
            //   把这 8 个字节的向量重新解释成一个 u64 lane，即 uint64x1_t。
            //
            // neon::vget_lane_u64(..., 0)
            //   取出第 0 个 u64 lane，也就是得到一个 u64。
            //
            // BitMask(...)
            //   把这个 u64 包装成 BitMask 返回。
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
    }

    /// Returns a `BitMask` indicating all tags in the group which are full.
    #[inline]
    pub(crate) fn match_full(self) -> BitMask {
        unsafe {
            let cmp = neon::vcgez_s8(neon::vreinterpret_s8_u8(self.0));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
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
        unsafe {
            let special = neon::vcltz_s8(neon::vreinterpret_s8_u8(self.0));
            Group(neon::vorr_u8(special, neon::vdup_n_u8(0x80)))
        }
    }
}
