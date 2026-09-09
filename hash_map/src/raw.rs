// 导入 TryReserveError 类型，表示预留容量操作可能返回的错误。
use crate::TryReserveError;
// 导入控制字节相关工具：位掩码迭代器、组、标签与标签切片扩展 trait。
use crate::control::{BitMaskIter, Group, Tag, TagSliceExt};
// 导入作用域守卫工具，用于在提前返回时自动执行清理操作。
use crate::scopeguard::{ScopeGuard, guard};
// 导入分支预测提示函数 likely 与 unlikely。
use crate::util::{likely, unlikely};
// 导入核心库的内存布局类型 Layout。
use core::alloc::Layout;
// 导入核心库的数组工具模块。
use core::array;
// 导入 FusedIterator trait，表示迭代器耗尽后会一直返回 None。
use core::iter::FusedIterator;
// 导入 PhantomData，用于零大小的类型级标记。
use core::marker::PhantomData;
// 导入核心库的内存操作工具模块。
use core::mem;
// 导入核心库的裸指针工具模块。
use core::ptr;
// 导入 NonNull，表示保证非空的裸指针。
use core::ptr::NonNull;
// 导入核心库的切片工具模块。
use core::slice;
// 导入分配错误处理函数，用于在不可失败场景下报告分配错误。
use stdalloc::alloc::handle_alloc_error;

#[cfg(test)]
// 仅在测试构建下导入分配错误类型。
use crate::alloc::AllocError;
// 导入分配器 trait、全局分配器实例与底层分配函数。
use crate::alloc::{Allocator, Global, do_alloc};

#[inline]
// 计算两个指针之间的字节距离（to - from）并转为 usize。
unsafe fn offset_from<T>(to: *const T, from: *const T) -> usize {
    // 调用指针自带的 offset_from 方法完成计算。
    unsafe { to.offset_from(from) as usize }
}

/// 内存分配出错时应返回错误还是中止程序。
#[derive(Copy, Clone)]
// 定义"可失败性"枚举，决定内存分配失败时的处理方式。
enum Fallibility {
    // 可失败：分配失败时返回错误。
    Fallible,
    // 不可失败：分配失败时直接 panic。
    Infallible,
}

// 为 Fallibility 实现两个错误构造辅助方法。
impl Fallibility {
    /// 容量溢出时返回的错误。
    #[cfg_attr(feature = "inline-more", inline)]
    // 根据可失败性返回容量溢出错误，或直接触发 panic。
    fn capacity_overflow(self) -> TryReserveError {
        // 按自身的可失败性进行匹配。
        match self {
            // 可失败：返回容量溢出错误。
            Fallibility::Fallible => TryReserveError::CapacityOverflow,
            // 不可失败：直接 panic。
            Fallibility::Infallible => panic!("Hash table capacity overflow"),
        }
    }

    /// 分配出错时返回的错误。
    #[cfg_attr(feature = "inline-more", inline)]
    // 根据可失败性返回分配错误，或调用运行时函数中止程序。
    fn alloc_err(self, layout: Layout) -> TryReserveError {
        // 按自身的可失败性进行匹配。
        match self {
            // 可失败：返回分配错误。
            Fallibility::Fallible => TryReserveError::AllocError { layout },
            // 不可失败：调用 handle_alloc_error 中止。
            Fallibility::Infallible => handle_alloc_error(layout),
        }
    }
}

// 为所有 Sized 类型提供类型属性常量的 trait。
trait SizedTypeProperties: Sized {
    // 常量：该类型是否为零大小类型(ZST)。
    const IS_ZERO_SIZED: bool = size_of::<Self>() == 0;
    // 常量：该类型是否需要执行 drop。
    const NEEDS_DROP: bool = mem::needs_drop::<Self>();
}

// 为所有 T 自动实现 SizedTypeProperties。
impl<T> SizedTypeProperties for T {}

/// 主哈希函数，用于选择探测的起始桶。
#[inline]
#[expect(clippy::cast_possible_truncation)]
// 通过截断哈希值得到起始桶的下标。
fn h1(hash: u64) -> usize {
    // 在 32 位平台上我们直接忽略哈希值的较高位。
    hash as usize
}

/// 基于三角形数的探测序列，保证（因为我们的表大小是 2 的幂）恰好访问每个元素组一次。
///
/// 三角形探测每次多跳 1 个组。所以第一次我们跳 1 个组（即继续线性扫描），
/// 然后跳 2 个组（跳过 1 个组），再跳 3 个组（跳过 2 个组），依此类推。
///
/// 探测会访问表中每个组的证明：
/// <https://fgiesen.wordpress.com/2015/02/22/triangular-numbers-mod-2n/>
#[derive(Clone)]
// 探测序列的状态：当前位置与当前步长。
struct ProbeSeq {
    // 当前探测到的位置。
    pos: usize,
    // 当前步长（到下一次跳转为止累计的量）。
    stride: usize,
}

// 为探测序列实现推进方法。
impl ProbeSeq {
    #[inline]
    // 将探测序列推进到下一个组。
    fn move_next(&mut self, bucket_mask: usize) {
        // 此时我们应该已经找到空桶并结束了探测。
        debug_assert!(
            // 步长必须不超过桶掩码。
            self.stride <= bucket_mask,
            // 断言失败时显示的消息。
            "Went past end of probe sequence"
        );

        // 步长增加一个组的宽度。
        self.stride = self.stride.wrapping_add(Group::WIDTH);
        // 位置按三角形数前进并回绕到桶掩码范围内。
        self.pos = self.pos.wrapping_add(self.stride) & bucket_mask;
    }
}

/// 返回容纳给定数量的条目所需的桶数，
/// 同时考虑最大负载因子。
///
/// 如果发生溢出则返回 `None`。
///
/// 这确保了 `buckets * table_layout.size >= table_layout.ctrl_align`。
// 针对 emscripten bug emscripten-core/emscripten-fastcomp#258 的变通方案
#[cfg_attr(target_os = "emscripten", inline(never))]
#[cfg_attr(not(target_os = "emscripten"), inline)]
// 把请求的容量换算成所需的桶数。
fn capacity_to_buckets(cap: usize, table_layout: TableLayout) -> Option<usize> {
    // 调试断言：容量不能为 0。
    debug_assert_ne!(cap, 0);

    // 对于小表，我们要求至少留有 1 个空桶，这样当元素不存在于表中时，
    // 查找才能保证终止。
    if cap < 15 {
        // 考虑一个小型 TableLayout，如 { size: 1, ctrl_align: 16 }，运行在
        // Group::WIDTH 为 16 的平台上（如带 SSE2 的 x86_64）。对于较小的
        // 桶大小，仅为填充到相对较大的 ctrl_align 就会浪费不少字节：
        //
        // | 容量 | 桶数 | 分配的字节数 | 每个条目的字节数 |
        // | ---- | ---- | ------------ | ---------------- |
        // |    3 |    4 |           36 | （吓人！） 12.0 |
        // |    7 |    8 |           40 | （较差）   5.7 |
        // |   14 |   16 |           48 |            3.4 |
        // |   28 |   32 |           80 |            3.3 |
        //
        // 一般来说，必须满足 buckets * table_layout.size >= table_layout.ctrl_align
        // 才能避开这些边缘情况。实现方式是针对小条目上调最小容量。这段代码
        // 只需处理小于或等于 Group::WIDTH 的 ctrl_align，因为有效的布局大小
        // 总是对齐值的倍数，所以对齐超过 Group::WIDTH 的情况不会碰到这个
        // 边缘情形。

        // 这段逻辑比较脆弱，例如将来若增加 32 字节的组，无论
        // table_layout.size 是多少它都会选择 3。
        let min_cap = match (Group::WIDTH, table_layout.size) {
            // 组宽 16 且条目大小不超过 1 字节：最小容量为 14。
            (16, 0..=1) => 14,
            // 组宽 16 且条目 2~3 字节，或组宽 8 且条目不超过 1 字节：最小容量为 7。
            (16, 2..=3) | (8, 0..=1) => 7,
            // 其他情况：最小容量为 3。
            _ => 3,
        };
        // 取请求容量与最小容量中的较大值。
        let cap = min_cap.max(cap);
        // 我们不使用 2 个桶的表，因为它只能容纳 1 个元素。
        // 相反，直接跳到可容纳 3 个元素的 4 桶表。
        let buckets = if cap < 4 {
            // 容量小于 4 时使用 4 个桶。
            4
        } else if cap < 8 {
            // 容量小于 8 时使用 8 个桶。
            8
        } else {
            // 否则使用 16 个桶。
            16
        };
        // 确保桶数据字节数不低于 ctrl_align。
        ensure_bucket_bytes_at_least_ctrl_align(table_layout, buckets);
        // 返回计算出的桶数。
        return Some(buckets);
    }

    // 否则要求 1/8 的桶为空（负载 87.5%）
    //
    // 修改此处时务必小心，calculate_layout 依赖这里的溢出检查。
    let adjusted_cap = cap.checked_mul(8)? / 7;

    // 任何溢出都会被 checked_mul 捕获。此外，上面除法带来的舍入误差
    // 会被 next_power_of_two 清理掉（由于前面的除法，它不可能溢出）。
    let buckets = adjusted_cap.next_power_of_two();
    // 确保桶数据字节数不低于 ctrl_align。
    ensure_bucket_bytes_at_least_ctrl_align(table_layout, buckets);
    // 返回计算出的桶数。
    Some(buckets)
}

// `maximum_buckets_in` 依赖如下性质：对于非 ZST 的 `T`，任何选定的
// `buckets` 都满足 `buckets * table_layout.size >=
// table_layout.ctrl_align`，因此 `calculate_layout_for` 不需要在
// `table_layout.size * buckets` 之外添加额外填充。如果小表的桶选择或
// 增长策略发生变化，请重新审视 `maximum_buckets_in`。
#[inline]
// 调试辅助函数：确保桶数据字节数至少为 ctrl_align。
fn ensure_bucket_bytes_at_least_ctrl_align(table_layout: TableLayout, buckets: usize) {
    // 仅对非零大小的条目进行检查。
    if table_layout.size != 0 {
        // 用饱和乘法计算桶数据的总字节数。
        let prod = table_layout.size.saturating_mul(buckets);
        // 调试断言：总字节数必须不低于 ctrl_align。
        debug_assert!(prod >= table_layout.ctrl_align);
    }
}

/// 返回给定桶掩码对应的最大有效容量，
/// 同时考虑最大负载因子。
#[inline]
// 把桶掩码换算成表的最大有效容量。
fn bucket_mask_to_capacity(bucket_mask: usize) -> usize {
    // 桶数不超过 8 的表。
    if bucket_mask < 8 {
        // 对于拥有 1/2/4/8 个桶的表，我们总是保留一个空槽。
        // 请记住桶掩码比桶数小 1。
        bucket_mask
    } else {
        // 桶数超过 8 的表。
        // `bucket_mask` 受最大分配大小的限制，因此永远不会是
        // `usize::MAX`，下面的 `+ 1` 不会溢出。
        debug_assert!(bucket_mask != usize::MAX);
        // 对于更大的表，我们保留 12.5% 的槽位为空。
        ((bucket_mask + 1) / 8) * 7
    }
}

/// 辅助结构体，使 `ctrl_align` 的最大值计算能够针对每个 `T` 静态完成，
/// 同时让 `calculate_layout_for` 的其余部分不依赖于 `T`
#[derive(Copy, Clone)]
// 表布局信息：条目大小与控制字节的对齐要求。
struct TableLayout {
    // 单个条目的大小。
    size: usize,
    // 控制字节的对齐要求。
    ctrl_align: usize,
}

// 为表布局实现构造与布局计算方法。
impl TableLayout {
    #[inline]
    // 针对 T 静态构造表布局。
    const fn new<T>() -> Self {
        // 取得 T 的内存布局。
        let layout = Layout::new::<T>();
        // 构造 TableLayout 实例。
        Self {
            // 条目大小。
            size: layout.size(),
            // 控制字节对齐取 T 的对齐与组宽度中的较大者。
            ctrl_align: if layout.align() > Group::WIDTH {
                // 对齐超过组宽度时使用 T 的对齐。
                layout.align()
            } else {
                // 否则使用组宽度作为对齐。
                Group::WIDTH
            },
        }
    }

    #[inline]
    // 根据桶数计算整张表的内存布局及控制字节偏移量。
    fn calculate_layout_for(self, buckets: usize) -> Option<(Layout, usize)> {
        // 调试断言：桶数必须是 2 的幂。
        debug_assert!(buckets.is_power_of_two());

        // 解构出条目大小与控制字节对齐。
        let TableLayout { size, ctrl_align } = self;
        // 由于 Layout 的相关方法尚未稳定，这里手动计算布局。
        let ctrl_offset =
            // 计算控制字节起始偏移并向上对齐，溢出时返回 None。
            size.checked_mul(buckets)?.checked_add(ctrl_align - 1)? & !(ctrl_align - 1);
        // 总长度 = 控制字节偏移 + 桶数 + 组宽度（多余的组宽度用于避免读取越界）。
        let len = ctrl_offset.checked_add(buckets + Group::WIDTH)?;

        // 我们需要额外检查分配大小不超过 `isize::MAX`
        // (https://github.com/rust-lang/rust/pull/95295)。
        if len > isize::MAX as usize - (ctrl_align - 1) {
            // 超出限制时返回 None 表示无法分配。
            return None;
        }

        // 返回计算出的布局与控制字节偏移量。
        Some((
            // 使用不检查的构造函数创建布局（大小与对齐均已验证）。
            unsafe { Layout::from_size_align_unchecked(len, ctrl_align) },
            // 控制字节的偏移量。
            ctrl_offset,
        ))
    }
}

/// 对哈希表中包含 `T` 的桶的引用。
///
/// 这通常就是指向元素本身的指针。但如果元素是 ZST（零大小类型），
/// 则我们改为跟踪元素在表中的索引，以便 `erase` 能正常工作。
pub(crate) struct Bucket<T> {
    // 实际上它是指向"元素后一个位置"而非元素本身的指针
    // 这样做是为了维持指针算术的不变量
    // 直接保存指向元素的指针会带来困难。
    // 使用 `NonNull` 以获得型变（variance）和 niche 布局
    ptr: NonNull<T>,
}

// 实现 Send 是为了支持 rayon。这是安全的，因为 Bucket 从未在公共 API 中暴露。
unsafe impl<T> Send for Bucket<T> {}

// 为 Bucket 实现 Clone。
impl<T> Clone for Bucket<T> {
    #[inline]
    // 克隆桶（仅复制内部指针）。
    fn clone(&self) -> Self {
        // 复制内部的 NonNull 指针。
        Self { ptr: self.ptr }
    }
}

// Bucket 的固有方法。
impl<T> Bucket<T> {
    /// 创建一个 [`Bucket`]，其中包含指向数据的指针。
    /// 指针计算通过计算距给定 `base` 指针的偏移量来完成
    /// （即 `base.as_ptr().sub(index)` 的便捷封装）。
    ///
    /// `index` 以 `T` 为单位；例如 `index` 为 3 表示指针偏移量为
    /// `3 * size_of::<T>()` 字节。
    ///
    /// 如果 `T` 是 ZST，则我们改为跟踪元素在表中的索引，
    /// 以便 `erase` 能正常工作（返回
    /// `NonNull::new_unchecked((index + 1) as *mut T)`）
    ///
    /// # Safety
    ///
    /// 如果 `size_of::<T>() != 0`，则安全性规则直接派生自 `*mut T` 的
    /// [`<*mut T>::sub`] 方法的安全性规则，以及 [`NonNull::new_unchecked`]
    /// 函数的安全性规则。
    ///
    /// 因此，为了维持 [`<*mut T>::sub`] 方法和 [`NonNull::new_unchecked`]
    /// 函数的安全契约，以及本 crate 工作的逻辑正确性，以下规则是充分且必要的：
    ///
    /// * `base` 指针不得为悬垂（dangling）指针，并且必须指向表中`数据部分`
    ///   第一个 `value element` 的末尾，即必须是
    ///   [`RawTable::data_end`] 或 [`RawTableInner::data_end<T>`]
    ///   返回的指针；
    ///
    /// * `index` 不得大于 `RawTableInner.bucket_mask`，即
    ///   `index <= RawTableInner.bucket_mask`，换句话说，`(index + 1)`
    ///   不得大于函数 [`RawTable::num_buckets`] 或
    ///   [`RawTableInner::num_buckets`] 返回的数量。
    ///
    /// 如果 `size_of::<T>() == 0`，则唯一的要求是 `index`
    /// 不得大于 `RawTableInner.bucket_mask`，即
    /// `index <= RawTableInner.bucket_mask`，换句话说，`(index + 1)`
    /// 不得大于函数 [`RawTable::num_buckets`] 或
    /// [`RawTableInner::num_buckets`] 返回的数量。
    #[inline]
    // 由 base 指针与索引计算桶指针（将 base 向下偏移 index 个 T 单位）。
    unsafe fn from_base_index(base: NonNull<T>, index: usize) -> Self {
        // 若 size_of::<T>() != 0，则返回指向表中数据部分某个 `element` 的指针
        // （我们从 "0" 开始计数，因此在表达式 T[last] 中，"last" 索引实际上
        // 比表中的 "buckets" 数少一，即 "last = RawTableInner.bucket_mask"）：
        //
        //                   `from_base_index(base, 1).as_ptr()` 返回的指针
        //                   指向表中数据部分的这个位置
        //                   （即 T1 的起始处）
        //                        |
        //                        |        `base: NonNull<T>` 必须指向这里
        //                        |         （即 T0 的末尾处或 C0 的起始处）
        //                        v         v
        // [Padding], Tlast, ..., |T1|, T0, |C0, C1, ..., Clast
        //                           ^
        //                           `from_base_index(base, 1)` 返回的指针
        //                           指向表中数据部分的这个位置
        //                           （即 T1 的末尾处）
        //
        // 其中：T0...Tlast - 我们存储的数据；C0...Clast - 控制字节
        // （即数据的元数据）。
        let ptr = if T::IS_ZERO_SIZED {
            // 由于 index 必须小于长度（bucket_mask），因此不会溢出，
            // 且 bucket_mask 保证小于 `isize::MAX`
            // （参见 TableLayout::calculate_layout_for 方法）
            ptr::without_provenance_mut(index + 1)
        // 非 ZST 的情况：
        } else {
            // 将 base 指针向下偏移 index 个 T 单位，得到指向该元素的指针。
            unsafe { base.as_ptr().sub(index) }
        };
        // 用计算出的指针构造 Bucket。
        Self {
            // 将裸指针转换为 NonNull（非空性由安全性契约保证）。
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

    /// 以两个指针之间的距离计算 [`Bucket`] 的索引
    /// （即 `base.as_ptr().offset_from(self.ptr.as_ptr()) as usize` 的便捷封装）。
    /// 返回值以 T 为单位：即字节距离除以 [`size_of::<T>()`]。
    ///
    /// 如果 `T` 是 ZST，则我们返回元素在表中的索引，
    /// 以便 `erase` 能正常工作（返回 `self.ptr.as_ptr() as usize - 1`）。
    ///
    /// 本函数是 [`from_base_index`] 的逆运算。
    ///
    /// # Safety
    ///
    /// 如果 `size_of::<T>() != 0`，则安全性规则直接派生自 `*const T` 的
    /// [`<*const T>::offset_from`] 方法的安全性规则。
    ///
    /// 因此，为了维持 [`<*const T>::offset_from`] 方法的安全契约，
    /// 以及本 crate 工作的逻辑正确性，以下规则是充分且必要的：
    ///
    /// * `base` 所含指针不得为悬垂（dangling）指针，并且必须指向表中`数据部分`
    ///   第一个 `element` 的末尾，即必须是
    ///   [`RawTable::data_end`] 或 [`RawTableInner::data_end<T>`]
    ///   返回的指针；
    ///
    /// * `self` 所含指针同样不得为悬垂指针；
    ///
    /// * `self` 与 `base` 必须由同一个 [`RawTable`]（或
    ///   [`RawTableInner`]）创建。
    ///
    /// 如果 `size_of::<T>() == 0`，则本函数总是安全的。
    #[inline]
    // 以两指针间的距离计算当前桶在表中的索引。
    unsafe fn to_base_index(&self, base: NonNull<T>) -> usize {
        // 若 size_of::<T>() != 0，则返回我们此前在表中数据部分存储该 `element`
        // 时所用的索引（我们从 "0" 开始计数，因此在表达式 T[last] 中，"last"
        // 索引实际上比表中的 "buckets" 数少一，即 "last = RawTableInner.bucket_mask"）。
        // 例如对表中第 5 个元素的计算过程如下：
        //
        //                        size_of::<T>()
        //                          |
        //                          |         `self = from_base_index(base, 5)` 返回的指针
        //                          |         指向表中数据部分的这个位置
        //                          |         （即 T5 的末尾处）
        //                          |           |                    `base: NonNull<T>` 必须指向这里
        //                          v           |                    （即 T0 的末尾处或 C0 的起始处）
        //                        /???\         v                      v
        // [Padding], Tlast, ..., |T10|, ..., T5|, T4, T3, T2, T1, T0, |C0, C1, C2, C3, C4, C5, ..., C10, ..., Clast
        //                                      \__________  __________/
        //                                                 \/
        //                                     `bucket.to_base_index(base)` = 5
        //                                     (base.as_ptr() as usize - self.ptr.as_ptr() as usize) / size_of::<T>()
        //
        // 其中：T0...Tlast - 我们存储的数据；C0...Clast - 控制字节（即数据的元数据）。
        if T::IS_ZERO_SIZED {
            // 这不可能是未定义行为
            self.ptr.as_ptr().addr() - 1
        // 非 ZST 的情况：
        } else {
            // 计算 base 指针与 self 指针之间的偏移（以 T 为单位）。
            unsafe { offset_from(base.as_ptr(), self.ptr.as_ptr()) }
        }
    }

    /// 获取指向 `data` 的底层裸指针 `*mut T`。
    ///
    /// # Note
    ///
    /// 如果 `T` 不是 [`Copy`]，请不要使用会触发 `T` 析构函数调用的 `*mut T`
    /// 方法（例如 [`<*mut T>::drop_in_place`] 方法），因为要正确地丢弃数据，
    /// 我们还需要清除 `data` 的控制字节。如果只丢弃了数据却没有清除
    /// `data 控制字节`，那么当 [`RawTable`] 离开作用域时就会导致重复析构（double drop）。
    ///
    /// 如果你修改了一个已初始化的 `value`，那么新 `T` 值及其借用形式上的
    /// [`Hash`] 和 [`Eq`] *必须* 与旧 `T` 值的保持一致，因为 map 不会重新
    /// 评估新值应该放在哪里，这意味着如果其位置未反映其状态，该值可能会
    /// "丢失"。
    #[inline]
    // 获取指向元素数据的底层裸指针。
    pub(crate) fn as_ptr(&self) -> *mut T {
        // ZST 分支：
        if T::IS_ZERO_SIZED {
            // 只需返回一个正确对齐的任意 ZST 指针
            // 对 ZST 来说，无效指针就足够了
            ptr::without_provenance_mut(align_of::<T>())
        // 非 ZST 的情况：
        } else {
            // 桶指针指向"元素后一个位置"，因此减 1 得到元素本身的指针。
            unsafe { self.ptr.as_ptr().sub(1) }
        }
    }

    /// 获取指向 `data` 的底层非空指针 `*mut T`。
    #[inline]
    // 获取指向元素数据的底层非空指针。
    fn as_non_null(&self) -> NonNull<T> {
        // SAFETY: `self.ptr` 本来就是一个 `NonNull`
        unsafe { NonNull::new_unchecked(self.as_ptr()) }
    }

    /// 创建一个新的 [`Bucket`]，它相对 `self` 偏移了给定的 `offset`。
    /// 指针计算通过计算距 `self` 指针的偏移量来完成
    /// （即 `self.ptr.as_ptr().sub(offset)` 的便捷封装）。本函数用于迭代器。
    ///
    /// `offset` 以 `T` 为单位；例如 `offset` 为 3 表示指针偏移量为
    /// `3 * size_of::<T>()` 字节。
    ///
    /// # Safety
    ///
    /// 如果 `size_of::<T>() != 0`，则安全性规则直接派生自 `*mut T` 的
    /// [`<*mut T>::sub`] 方法的安全性规则，以及 [`NonNull::new_unchecked`]
    /// 函数的安全性规则。
    ///
    /// 因此，为了维持 [`<*mut T>::sub`] 方法和 [`NonNull::new_unchecked`]
    /// 函数的安全契约，以及本 crate 工作的逻辑正确性，以下规则是充分且必要的：
    ///
    /// * `self` 所含指针不得为悬垂（dangling）指针；
    ///
    /// * `self.to_base_index() + offset` 不得大于 `RawTableInner.bucket_mask`，
    ///   即 `(self.to_base_index() + offset) <= RawTableInner.bucket_mask`，换句话说，
    ///   `self.to_base_index() + offset + 1` 不得大于函数
    ///   [`RawTable::num_buckets`] 或 [`RawTableInner::num_buckets`] 返回的数量。
    ///
    /// 如果 `size_of::<T>() == 0`，则唯一的要求是
    /// `self.to_base_index() + offset` 不得大于 `RawTableInner.bucket_mask`，
    /// 即 `(self.to_base_index() + offset) <= RawTableInner.bucket_mask`，换句话说，
    /// `self.to_base_index() + offset + 1` 不得大于函数
    /// [`RawTable::num_buckets`] 或 [`RawTableInner::num_buckets`] 返回的数量。
    #[inline]
    // 返回相对 self 向下偏移 offset 个 T 单位的新 Bucket（供迭代器使用）。
    unsafe fn next_n(&self, offset: usize) -> Self {
        // 根据 T 是否为 ZST 选择指针计算方式。
        let ptr = if T::IS_ZERO_SIZED {
            // 对 ZST 来说，无效指针就足够了
            ptr::without_provenance_mut(self.ptr.as_ptr().addr() + offset)
        // 非 ZST 的情况：
        } else {
            // 将 self 指针向下偏移 offset 个 T 单位。
            unsafe { self.ptr.as_ptr().sub(offset) }
        };
        // 用新指针构造 Bucket。
        Self {
            // 将裸指针转换为 NonNull（非空性由安全性契约保证）。
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

    /// 执行所指向 `data` 的析构函数（如果有的话）。
    ///
    /// # Safety
    ///
    /// 安全性注意事项参见 [`ptr::drop_in_place`]。
    ///
    /// 你应该使用 [`RawTable::erase`] 来代替本函数，
    /// 或者谨慎地直接调用本函数，因为要正确地丢弃数据，
    /// 我们还需要清除 `data` 的控制字节。
    /// 如果只丢弃了数据却没有擦除 `data 控制字节`，
    /// 当 [`RawTable`] 离开作用域时就会导致重复析构（double drop）。
    #[cfg_attr(feature = "inline-more", inline)]
    // 执行所指向数据的析构函数（如果有的话）。
    pub(crate) unsafe fn drop(&self) {
        // SAFETY: 由调用方保证指针有效且不会发生重复析构。
        unsafe {
            // 在元素指针上调用 drop_in_place，执行析构。
            self.as_ptr().drop_in_place();
        }
    }

    /// 从 `self` 中按位读取 `value` 而不移动它。这会使 `self` 中的
    /// 内存保持原样。
    ///
    /// # Safety
    ///
    /// 安全性注意事项参见 [`ptr::read`]。
    ///
    /// 你应该使用 [`RawTable::remove`] 来代替本函数，
    /// 或者谨慎地直接调用本函数，因为当读取的 `value` 离开作用域时，
    /// 编译器会调用其析构函数。由于未擦除的 `data 控制字节`，
    /// 当 [`RawTable`] 离开作用域时可能会引发重复析构（double drop）。
    #[inline]
    // 按位读取元素值而不移动它。
    pub(crate) unsafe fn read(&self) -> T {
        // SAFETY: 由调用方保证满足 `ptr::read` 的安全性要求。
        unsafe { self.as_ptr().read() }
    }

    /// 用给定的 `value` 覆写某个内存位置，而不读取或析构旧值
    /// （类似于 [`ptr::write`] 函数）。
    ///
    /// # Safety
    ///
    /// 安全性注意事项参见 [`ptr::write`]。
    ///
    /// # Note
    ///
    /// 新 `T` 值及其借用形式上的 [`Hash`] 和 [`Eq`] *必须* 与旧 `T` 值的
    /// 保持一致，因为 map 不会重新评估新值应该放在哪里，这意味着
    /// 如果其位置未反映其状态，该值可能会"丢失"。
    #[inline]
    // 用给定值覆写元素内存位置（不读取也不析构旧值）。
    pub(crate) unsafe fn write(&self, val: T) {
        // SAFETY: 由调用方保证满足 `ptr::write` 的安全性要求。
        unsafe {
            // 将 val 按位写入元素所在的内存位置。
            self.as_ptr().write(val);
        }
    }

    /// 返回指向 `value` 的共享不可变引用。
    ///
    /// # Safety
    ///
    /// 安全性注意事项参见 [`NonNull::as_ref`]。
    #[inline]
    // 返回指向元素的共享不可变引用。
    pub(crate) unsafe fn as_ref<'a>(&self) -> &'a T {
        // SAFETY: 由调用方保证满足 `NonNull::as_ref` 的安全性要求。
        unsafe { &*self.as_ptr() }
    }

    /// 返回指向 `value` 的唯一可变引用。
    ///
    /// # Safety
    ///
    /// 安全性注意事项参见 [`NonNull::as_mut`]。
    ///
    /// # Note
    ///
    /// 新 `T` 值及其借用形式上的 [`Hash`] 和 [`Eq`] *必须* 与旧 `T` 值的
    /// 保持一致，因为 map 不会重新评估新值应该放在哪里，这意味着
    /// 如果其位置未反映其状态，该值可能会"丢失"。
    #[inline]
    // 返回指向元素的唯一可变引用。
    pub(crate) unsafe fn as_mut<'a>(&self) -> &'a mut T {
        // SAFETY: 由调用方保证满足 `NonNull::as_mut` 的安全性要求。
        unsafe { &mut *self.as_ptr() }
    }
}

/// 一个提供非安全(unsafe) API 的原始哈希表。
pub(crate) struct RawTable<T, A: Allocator = Global> {
    // 内部保存的表状态（桶掩码、控制字节指针、计数等）。
    table: RawTableInner,
    // 用于分配和释放内存的分配器。
    alloc: A,
    // 告诉 dropck 我们拥有 T 的实例。
    marker: PhantomData<T>,
}

/// `RawTable` 的非泛型部分，使得无论使用多少种不同的键值类型，
/// 函数都只需被实例化一次。
struct RawTableInner {
    // 用于从哈希值获取索引的掩码。该值比
    // 表中桶的数量小 1。
    bucket_mask: usize,

    // [Padding], T_n, ..., T1, T0, C0, C1, ...
    //                              ^ 指向这里
    ctrl: NonNull<u8>,

    // 在需要扩容表之前还可以插入的元素数量。
    growth_left: usize,

    // 表中的元素数量，实际只被 len() 使用。
    items: usize,
}

// RawTable 针对 Global 分配器的特化实现。
impl<T> RawTable<T, Global> {
    /// 创建一个新的空哈希表，不分配任何内存。
    ///
    /// 实际上这会返回一个恰好有 1 个桶的表。但由于我们的负载因子强制要求
    /// 始终至少有 1 个空闲桶，该桶永远不会被写入，因此我们可以让数据指针保持悬垂。
    #[inline]
    #[cfg_attr(feature = "rustc-dep-of-std", rustc_const_stable_indirect)]
    // 创建新的空表（const 构造函数）。
    pub(crate) const fn new() -> Self {
        // 使用预定义的空表状态构造 Self。
        Self {
            // 内部表使用预定义的空状态。
            table: RawTableInner::NEW,
            // 使用全局分配器。
            alloc: Global,
            // PhantomData 标记，表示拥有 T。
            marker: PhantomData,
        }
    }

    /// 分配一个新的哈希表，其容量至少足够在不重新分配的情况下
    /// 插入给定数量的元素。
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        // 委托给使用全局分配器的版本。
        Self::with_capacity_in(capacity, Global)
    }
}

// RawTable 针对泛型分配器 A 的实现。
impl<T, A: Allocator> RawTable<T, A> {
    // 当前元素类型的表布局（桶与控制字节的大小和对齐）。
    const TABLE_LAYOUT: TableLayout = TableLayout::new::<T>();

    /// 使用给定的分配器创建一个新的空哈希表，不分配任何内存。
    ///
    /// 实际上这会返回一个恰好有 1 个桶的表。但由于我们的负载因子强制要求
    /// 始终至少有 1 个空闲桶，该桶永远不会被写入，因此我们可以让数据指针保持悬垂。
    #[inline]
    #[cfg_attr(feature = "rustc-dep-of-std", rustc_const_stable_indirect)]
    // 使用给定分配器创建新的空哈希表（const 构造函数）。
    pub(crate) const fn new_in(alloc: A) -> Self {
        // 构造 Self。
        Self {
            // 内部表使用预定义的空状态。
            table: RawTableInner::NEW,
            // 保存传入的分配器。
            alloc,
            // PhantomData 标记，表示拥有 T。
            marker: PhantomData,
        }
    }

    /// 分配一个具有给定桶数量的新哈希表。
    ///
    /// 控制字节保持未初始化状态。
    #[cfg_attr(feature = "inline-more", inline)]
    // 分配一个控制字节未初始化的新哈希表。
    unsafe fn new_uninitialized(
        // 用于分配内存的分配器。
        alloc: A,
        // 桶的数量（必须为 2 的幂）。
        buckets: usize,
        // 失败处理方式（可回退或不可回退）。
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError> {
        // 调试断言桶数量是 2 的幂。
        debug_assert!(buckets.is_power_of_two());

        // 包装构造结果。
        Ok(Self {
            // 内部表由不安全代码创建。
            table: unsafe {
                // 调用内部表的未初始化分配函数。
                RawTableInner::new_uninitialized(&alloc, Self::TABLE_LAYOUT, buckets, fallibility)
            // 分配失败时通过 ? 返回错误。
            }?,
            // 保存分配器。
            alloc,
            // PhantomData 标记，表示拥有 T。
            marker: PhantomData,
        })
    }

    /// 使用给定的分配器分配一个新哈希表，其容量至少足够在不重新分配的情况下
    /// 插入给定数量的元素。
    pub(crate) fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        // 构造 Self。
        Self {
            // 按给定容量分配内部表。
            table: RawTableInner::with_capacity(&alloc, Self::TABLE_LAYOUT, capacity),
            // 保存分配器。
            alloc,
            // PhantomData 标记，表示拥有 T。
            marker: PhantomData,
        }
    }

    /// 返回底层分配器的引用。
    #[inline]
    // 返回底层分配器的引用。
    pub(crate) fn allocator(&self) -> &A {
        // 直接返回内部保存的分配器。
        &self.alloc
    }

    /// 返回从分配的起始点来看，表中最后一个 `data` 元素之后
    /// 位置的指针。
    ///
    /// 调用者必须确保 `RawTable` 比返回的 [`NonNull<T>`] 存活得更久，
    /// 否则使用它可能导致 [`undefined behavior`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回指向数据区末尾（最后一个元素之后）的指针。
    pub(crate) fn data_end(&self) -> NonNull<T> {
        //                        `self.table.ctrl.cast()` 返回的指针
        //                        指向这里（`T0` 的末尾）
        //                          ∨
        // [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, CTa_0, CTa_1, ..., CTa_m
        //                           \________  ________/
        //                                    \/
        //       `n = buckets - 1`，即 `RawTable::num_buckets() - 1`
        //
        // 其中：T0...T_n  - 我们存储的数据；
        //        CT0...CT_n - 控制字节，即 `data` 的元数据。
        //        CTa_0...CTa_m - 额外的控制字节，其中 `m = Group::WIDTH - 1`（这样即使
        //                        `h1(hash) & self.bucket_mask` 的结果等于 `self.bucket_mask`，
        //                        从堆上加载 `Group` 字节进行的查找也能正常工作）。另见
        //                        `RawTableInner::set_ctrl` 函数。
        //
        // P.S. `h1(hash) & self.bucket_mask` 等价于 `hash as usize % self.num_buckets()`，因为桶的数量
        // 是 2 的幂，且 `self.bucket_mask = self.num_buckets() - 1`。
        self.table.ctrl.cast()
    }

    /// 返回指向数据表起始位置的指针。
    #[inline]
    #[cfg(feature = "nightly")]
    // 返回指向数据表起始位置的指针。
    pub(crate) unsafe fn data_start(&self) -> NonNull<T> {
        // 从 data_end 向前回退 num_buckets 个字节得到数据区起点。
        unsafe { NonNull::new_unchecked(self.data_end().as_ptr().wrapping_sub(self.num_buckets())) }
    }

    /// 返回该哈希表内部分配的内存总量，单位为字节。
    ///
    /// 返回的数字仅供参考。它主要用于内存分析（memory profiling）。
    #[inline]
    // 返回哈希表内部分配的内存总字节数。
    pub(crate) fn allocation_size(&self) -> usize {
        // SAFETY: 我们使用的 `table_layout` 与分配该表时
        // 使用的相同。
        unsafe { self.table.allocation_size_or_zero(Self::TABLE_LAYOUT) }
    }

    /// 从 `Bucket` 反推出桶的索引。
    #[inline]
    // 由桶指针反推出其在表中的索引。
    pub(crate) unsafe fn bucket_index(&self, bucket: &Bucket<T>) -> usize {
        // 调用桶的转换方法计算基础索引。
        unsafe { bucket.to_base_index(self.data_end()) }
    }

    /// 返回指向表中某个元素的指针。
    ///
    /// 调用者必须确保 `RawTable` 比返回的 [`Bucket<T>`] 存活得更久，
    /// 否则使用它可能导致 [`undefined behavior`]。
    ///
    /// # Safety
    ///
    /// 如果 `size_of::<T>() != 0`，那么此函数的调用者必须遵守
    /// 以下安全规则：
    ///
    /// * 表必须已经完成分配；
    ///
    /// * `index` 不得大于 [`RawTable::num_buckets`] 函数返回的数量，
    ///   即 `(index + 1) <= self.num_buckets()`。
    ///
    /// 在尚未分配的表上以索引零（`index == 0`）调用此函数是安全的，
    /// 但使用返回的 [`Bucket`] 会导致 [`undefined behavior`]。
    ///
    /// 如果 `size_of::<T>() == 0`，那么唯一的要求是 `index` 不得大于
    /// [`RawTable::num_buckets`] 函数返回的数量，即
    /// `(index + 1) <= self.num_buckets()`。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回指向表中第 index 个元素的桶指针。
    pub(crate) unsafe fn bucket(&self, index: usize) -> Bucket<T> {
        // 如果 size_of::<T>() != 0，则返回指向表 `data 部分` 中该 `element` 的指针
        // （我们从 "0" 开始计数，因此在表达式 T[n] 中，索引 "n" 实际上比 `RawTable` 的
        // "buckets"（桶）数量小 1，即 "n = RawTable::num_buckets() - 1"）：
        //
        //           `table.bucket(3).as_ptr()` 返回一个指针，指向 `RawTable` 的 `data`
        //           部分，即 T3 的起始处（见 `Bucket::as_ptr`）
        //                  |
        //                  |               `base = self.data_end()` 指向这里
        //                  |               （CT0 的起始处，即 T0 的末尾）
        //                  v                 v
        // [Pad], T_n, ..., |T3|, T2, T1, T0, |CT0, CT1, CT2, CT3, ..., CT_n, CTa_0, CTa_1, ..., CTa_m
        //                     ^                                              \__________  __________/
        //        `table.bucket(3)` 返回的指针指向                                        \/
        //         `RawTable` 的 `data` 部分（即                          额外的控制字节
        //         T3 的末尾）                                              `m = Group::WIDTH - 1`
        //
        // 其中：T0...T_n  - 我们存储的数据；
        //        CT0...CT_n - 控制字节，即 `data` 的元数据；
        //        CTa_0...CTa_m - 额外的控制字节（这样即使 `h1(hash) & self.table.bucket_mask`
        //                        的结果等于 `self.table.bucket_mask`，从堆上加载 `Group` 字节
        //                        进行的查找也能正常工作）。另见 `RawTableInner::set_ctrl` 函数。
        //
        // P.S. `h1(hash) & self.table.bucket_mask` 等价于 `hash as usize % self.num_buckets()`，因为桶的数量
        // 是 2 的幂，且 `self.table.bucket_mask = self.num_buckets() - 1`。
        debug_assert_ne!(self.table.bucket_mask, 0);
        // 调试断言索引未越界。
        debug_assert!(index < self.num_buckets());
        // 以 data_end 为基准构造桶指针。
        unsafe { Bucket::from_base_index(self.data_end(), index) }
    }

    /// 从表中擦除一个元素，但不执行 drop。
    #[cfg_attr(feature = "inline-more", inline)]
    // 不执行 drop 地擦除表中的元素。
    unsafe fn erase_no_drop(&mut self, item: &Bucket<T>) {
        // 不安全块：item 必须指向有效的桶。
        unsafe {
            // 获取该桶在表中的索引。
            let index = self.bucket_index(item);
            // 将该桶的控制字节标记为空（擦除）。
            self.table.erase(index);
        }
    }

    /// 从表中擦除一个元素，并在原地执行 drop。
    #[cfg_attr(feature = "inline-more", inline)]
    #[expect(clippy::needless_pass_by_value)]
    // 擦除元素并在原地 drop 它。
    pub(crate) unsafe fn erase(&mut self, item: Bucket<T>) {
        // 不安全块。
        unsafe {
            // 先从表中擦除该元素，因为 drop 可能会 panic。
            self.erase_no_drop(&item);
            // 然后再 drop 该元素。
            item.drop();
        }
    }

    /// 从表中移除一个元素，并将其返回。
    ///
    /// 同时返回新近变为空闲的桶的索引。
    #[cfg_attr(feature = "inline-more", inline)]
    #[expect(clippy::needless_pass_by_value)]
    // 移除元素并返回它，以及空闲桶的索引。
    pub(crate) unsafe fn remove(&mut self, item: Bucket<T>) -> (T, usize) {
        // 不安全块。
        unsafe {
            // 先擦除该元素（不 drop）。
            self.erase_no_drop(&item);
            // 读取元素值并计算桶索引。
            (item.read(), self.bucket_index(&item))
        }
    }

    /// 从表中移除一个元素，并将其返回。
    ///
    /// 同时返回新近变为空闲的桶的索引，
    /// 以及该桶原先的 `Tag`。
    #[cfg_attr(feature = "inline-more", inline)]
    #[expect(clippy::needless_pass_by_value)]
    // 移除元素并返回它、空闲桶索引以及原先的 Tag。
    pub(crate) unsafe fn remove_tagged(&mut self, item: Bucket<T>) -> (T, usize, Tag) {
        // 不安全块。
        unsafe {
            // 计算桶索引。
            let index = self.bucket_index(&item);
            // 读取该桶原先的控制字节（Tag）。
            let tag = *self.table.ctrl(index);
            // 擦除该桶。
            self.table.erase(index);
            // 返回元素、索引和 Tag。
            (item.read(), index, tag)
        }
    }

    /// 查找并从表中移除一个元素，将其返回。
    #[cfg_attr(feature = "inline-more", inline)]
    // 按哈希和相等判断查找并移除元素。
    pub(crate) fn remove_entry(&mut self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<T> {
        // 避免使用 `Option::map`，因为它会使 LLVM IR 膨胀。
        match self.find(hash, eq) {
            // 找到则移除并返回该元素。
            Some(bucket) => Some(unsafe { self.remove(bucket).0 }),
            // 未找到则返回 None。
            None => None,
        }
    }

    /// 将表中所有桶标记为空，但不 drop 其内容。
    #[cfg_attr(feature = "inline-more", inline)]
    // 不 drop 内容地将所有桶标记为空。
    pub(crate) fn clear_no_drop(&mut self) {
        // 委托给内部表的清空方法。
        self.table.clear_no_drop();
    }

    /// 移除表中所有元素，但不释放底层内存。
    #[cfg_attr(feature = "inline-more", inline)]
    // 清空表中的所有元素，但保留内存。
    pub(crate) fn clear(&mut self) {
        // 表为空时直接返回。
        if self.is_empty() {
            // 特殊处理空表的情况，以避免出人意料的 O(capacity) 时间开销。
            return;
        }
        // 即使某个元素的 drop 发生 panic，也确保表被重置。
        let mut self_ = guard(self, |self_| self_.clear_no_drop());
        // 不安全块。
        unsafe {
            // SAFETY: 即使在 drop 元素的过程中发生 panic，ScopeGuard 也会将表的
            // `items` 字段清零，从而不会对元素造成二次 drop。
            self_.table.drop_elements::<T>();
        }
    }

    /// 将表收缩至恰好容纳 `max(self.len(), min_size)` 个元素。
    #[cfg_attr(feature = "inline-more", inline)]
    // 将表收缩至容纳 max(self.len(), min_size) 个元素。
    pub(crate) fn shrink_to(&mut self, min_size: usize, hasher: impl Fn(&T) -> u64) {
        // 计算我们需要为其预留空间的
        // 最小元素数量。
        let min_size = usize::max(self.table.items, min_size);
        // 目标大小为 0 时直接释放旧表。
        if min_size == 0 {
            // 用全新的空内部表替换旧表。
            let mut old_inner = mem::replace(&mut self.table, RawTableInner::NEW);
            // 不安全块。
            unsafe {
                // SAFETY:
                // 1. 我们只调用该函数一次；
                // 2. 我们可以确定 `alloc` 和 `table_layout` 与分配此表时所用的 [`Allocator`]
                //    和 [`TableLayout`] 一致。
                // 3. 如果任何元素的 drop 函数 panic，也只会造成内存泄漏，
                //    因为我们已经用新的内部表替换了旧表。
                old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
            }
            // 释放完成后直接返回。
            return;
        }

        // 计算为这么多元素所需的桶数量。
        // 如果计算发生溢出，则说明请求的桶数量
        // 必然大于当前拥有的数量，因此无需
        // 做任何事情。
        let Some(min_buckets) = capacity_to_buckets(min_size, Self::TABLE_LAYOUT) else {
            // 无法计算出所需的桶数量时，无需收缩。
            return;
        };

        // 如果我们拥有的桶数量多于所需，则收缩表。
        if min_buckets < self.num_buckets() {
            // 表为空时的快速路径。
            if self.table.items == 0 {
                // 为空表新建一个内部表。
                let new_inner =
                    // 按最小容量创建新内部表。
                    RawTableInner::with_capacity(&self.alloc, Self::TABLE_LAYOUT, min_size);
                // 将新内部表替换进 self，并取出旧表。
                let mut old_inner = mem::replace(&mut self.table, new_inner);
                // 不安全块。
                unsafe {
                    // SAFETY:
                    // 1. 我们只调用该函数一次；
                    // 2. 我们可以确定 `alloc` 和 `table_layout` 与分配此表时所用的 [`Allocator`]
                    //    和 [`TableLayout`] 一致。
                    // 3. 如果任何元素的 drop 函数 panic，也只会造成内存泄漏，
                    //    因为我们已经用新的内部表替换了旧表。
                    old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
                }
            } else {
                // SAFETY:
                // 1. 我们可以确定 `min_size >= self.table.items`。
                // 2. [`RawTableInner`] 的控制字节必然已经正确初始化，因为我们
                //    绝不会在公开 API 中暴露 RawTable::new_uninitialized。
                let result = unsafe { self.resize(min_size, hasher, Fallibility::Infallible) };

                // SAFETY: 调用 `resize` 函数的结果不可能是错误，
                // 因为 `fallibility == Fallibility::Infallible`。
                unsafe { result.unwrap_unchecked() };
            }
        }
    }

    /// 确保至少还能向表中插入 `additional` 个元素
    /// 而不需要重新分配内存。
    #[cfg_attr(feature = "inline-more", inline)]
    // 确保表有足够容量再插入 additional 个元素。
    pub(crate) fn reserve(&mut self, additional: usize, hasher: impl Fn(&T) -> u64) {
        // 剩余增长空间不足时才需要扩容。
        if unlikely(additional > self.table.growth_left) {
            // SAFETY: [`RawTableInner`] 的控制字节必然已经正确初始化，因为我们
            // 绝不会在公开 API 中暴露 RawTable::new_uninitialized。
            let result =
                // 调用慢路径预留函数（不可失败）。
                unsafe { self.reserve_rehash(additional, hasher, Fallibility::Infallible) };

            // SAFETY: 所有分配错误都会在 `RawTableInner::reserve_rehash` 内部被捕获。
            unsafe { result.unwrap_unchecked() };
        }
    }

    /// 尝试确保至少还能向表中插入 `additional` 个元素
    /// 而不需要重新分配内存。
    #[cfg_attr(feature = "inline-more", inline)]
    // 尝试预留容量：可失败版本的 reserve。
    pub(crate) fn try_reserve(
        // 被预留容量的表。
        &mut self,
        // 需要额外预留的元素数量。
        additional: usize,
        // 重新哈希元素时使用的哈希函数。
        hasher: impl Fn(&T) -> u64,
    ) -> Result<(), TryReserveError> {
        // 剩余增长空间不足时才需要扩容。
        if additional > self.table.growth_left {
            // SAFETY: [`RawTableInner`] 的控制字节必然已经正确初始化，因为我们
            // 绝不会在公开 API 中暴露 RawTable::new_uninitialized。
            unsafe { self.reserve_rehash(additional, hasher, Fallibility::Fallible) }
        } else {
            // 剩余增长空间充足时直接成功。
            Ok(())
        }
    }

    /// `reserve` 和 `try_reserve` 的内联外慢路径。
    ///
    /// # Safety
    ///
    /// [`RawTableInner`] 的控制字节必须已正确初始化，
    /// 否则调用此函数会导致 [`undefined behavior`]
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[cold]
    #[inline(never)]
    // 预留容量的慢路径（必要时触发重新哈希）。
    unsafe fn reserve_rehash(
        // 被扩容的表。
        &mut self,
        // 需要额外预留的元素数量。
        additional: usize,
        // 重新哈希元素时使用的哈希函数。
        hasher: impl Fn(&T) -> u64,
        // 失败处理方式（可回退或不可回退）。
        fallibility: Fallibility,
    ) -> Result<(), TryReserveError> {
        // 不安全块。
        unsafe {
            // SAFETY:
            // 1. 我们可以确定 `alloc` 和 `layout` 与分配此表时所用的 [`Allocator`] 和
            //    [`TableLayout`] 一致。
            // 2. `drop` 函数是表中存储元素的实际 drop 函数。
            // 3. 调用者确保 `RawTableInner` 的控制字节
            //    已经初始化。
            self.table.reserve_rehash_inner(
                // 传递分配器。
                &self.alloc,
                // 传递额外预留数量。
                additional,
                // 用于对元素重新计算哈希的闭包。
                &|table, index| hasher(table.bucket::<T>(index).as_ref()),
                // 传递失败处理方式。
                fallibility,
                // 传递表布局。
                Self::TABLE_LAYOUT,
                // 仅当元素需要 drop 时才传入 drop 函数。
                if T::NEEDS_DROP {
                    // 对元素指针执行 drop。
                    Some(|ptr| ptr::drop_in_place(ptr.cast::<T>()))
                } else {
                    // 不需要 drop 时。
                    None
                },
            )
        }
    }

    /// 分配一个不同大小的新表，并将当前表的内容
    /// 移动到其中。
    ///
    /// # Safety
    ///
    /// [`RawTableInner`] 的控制字节必须已正确初始化，
    /// 否则调用此函数会导致 [`undefined behavior`]
    ///
    /// 此函数的调用者必须确保 `capacity >= self.table.items`，
    /// 否则：
    ///
    /// * 如果 `self.table.items != 0`，以等于 0 的 `capacity`（`capacity == 0`）
    ///   调用此函数会导致 [`undefined behavior`]。
    ///
    /// * 如果 `self.table.items > capacity_to_buckets(capacity, Self::TABLE_LAYOUT)`，
    ///   调用此函数将永远不会返回（会无限循环）。
    ///
    /// 更多信息见 [`RawTableInner::find_insert_index`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    unsafe fn resize(
        // 被调整大小的表。
        &mut self,
        // 新表的容量。
        capacity: usize,
        // 重新哈希元素时使用的哈希函数。
        hasher: impl Fn(&T) -> u64,
        // 失败处理方式（可回退或不可回退）。
        fallibility: Fallibility,
    ) -> Result<(), TryReserveError> {
        // SAFETY:
        // 1. 此函数的调用者保证 `capacity >= self.table.items`。
        // 2. 我们可以确定 `alloc` 和 `layout` 与分配此表时所用的 [`Allocator`] 和
        //    [`TableLayout`] 一致。
        // 3. 调用者确保 `RawTableInner` 的控制字节
        //    已经初始化。
        unsafe {
            // 调用内部表的大小调整函数。
            self.table.resize_inner(
                // 传递分配器。
                &self.alloc,
                // 传递新容量。
                capacity,
                // 用于对元素重新计算哈希的闭包。
                &|table, index| hasher(table.bucket::<T>(index).as_ref()),
                // 传递失败处理方式。
                fallibility,
                // 传递表布局。
                Self::TABLE_LAYOUT,
            )
        }
    }

    /// 向表中插入一个新元素，并返回其原始桶。
    ///
    /// 此函数不会检查给定元素是否已存在于表中。
    #[cfg_attr(feature = "inline-more", inline)]
    // 向表中插入一个新元素，并返回其原始桶。
    pub(crate) fn insert(&mut self, hash: u64, value: T, hasher: impl Fn(&T) -> u64) -> Bucket<T> {
        // 进入 unsafe 块。
        unsafe {
            // SAFETY:
            // 1. [`RawTableInner`] 的控制字节必须已经正确初始化，因为我们绝不会
            //    在公共 API 中暴露 `RawTable::new_uninitialized`。
            //
            // 2. 我们会在调用此函数之后立即预留额外空间（如有必要）。
            let mut index = self.table.find_insert_index(hash);

            // 如果我们正在替换一个墓碑（tombstone），那么一旦达到负载因子，就可以避免扩容表。
            // 这样做是可行的，因为在这种情况下 EMPTY 槽的数量不会改变。
            //
            // SAFETY: 该函数保证返回 `0..=self.num_buckets()` 范围内的索引。
            let old_ctrl = *self.table.ctrl(index);
            // 若剩余增长空间为 0 且旧的控制字节属于特殊空槽（EMPTY/DELETED），则需要扩容。
            if unlikely(self.table.growth_left == 0 && old_ctrl.special_is_empty()) {
                // 预留一个元素的空间（可能触发扩容）。
                self.reserve(1, hasher);
                // SAFETY: 我们可以确定 `RawTableInner` 的控制字节已初始化，
                // 并且表中有多余的空间。
                index = self.table.find_insert_index(hash);
            }

            // 在找到的插入索引处插入元素。
            self.insert_at_index(hash, index, value)
        }
    }

    /// 向表中插入一个新元素，并返回其可变引用。
    ///
    /// 此函数不会检查给定元素是否已存在于表中。
    #[cfg_attr(feature = "inline-more", inline)]
    // 向表中插入一个新条目，并返回其可变引用。
    pub(crate) fn insert_entry(
        // 可变借用自身。
        &mut self,
        // 元素的哈希值。
        hash: u64,
        // 要插入的元素。
        value: T,
        // 用于重新哈希的哈希函数。
        hasher: impl Fn(&T) -> u64,
    ) -> &mut T {
        // 调用 insert 插入元素，并将其原始桶转换为可变引用。
        unsafe { self.insert(hash, value, hasher).as_mut() }
    }

    /// 向表中插入一个新元素，而不扩容表。
    ///
    /// 表中必须有足够的空间来插入新元素。
    ///
    /// 此函数不会检查给定元素是否已存在于表中。
    #[cfg_attr(feature = "inline-more", inline)]
    #[cfg(feature = "rustc-internal-api")]
    // 在不扩容表的情况下插入一个新元素，并返回其原始桶。
    pub(crate) unsafe fn insert_no_grow(&mut self, hash: u64, value: T) -> Bucket<T> {
        // 进入 unsafe 块。
        unsafe {
            // 准备插入索引，返回索引以及旧的控制字节。
            let (index, old_ctrl) = self.table.prepare_insert_index(hash);
            // 获取该索引对应的桶。
            let bucket = self.table.bucket(index);

            // 如果我们正在替换一个 DELETED 条目，
            // 那么就不需要更新负载计数器。
            self.table.growth_left -= old_ctrl.special_is_empty() as usize;

            // 将值写入桶中。
            bucket.write(value);
            // 元素数量加一。
            self.table.items += 1;
            // 返回该桶。
            bucket
        }
    }

    /// 临时移除一个桶，对被移除的元素应用给定的函数，
    /// 并可选地将返回的值放回同一个桶中。
    ///
    /// 如果桶被清空，则返回该桶的标签。
    ///
    /// 此函数不会检查给定的桶是否确实被占用。
    #[cfg_attr(feature = "inline-more", inline)]
    // 临时移除桶，应用函数后有条件地放回新元素。
    pub(crate) unsafe fn replace_bucket_with<F>(&mut self, bucket: Bucket<T>, f: F) -> Option<Tag>
    // where 子句：约束 F 的类型。
    where
        // F 接收旧元素，返回可选的新元素。
        F: FnOnce(T) -> Option<T>,
    {
        // 进入 unsafe 块。
        unsafe {
            // 计算给定桶对应的索引。
            let index = self.bucket_index(&bucket);
            // 读取该索引处的旧控制字节。
            let old_ctrl = *self.table.ctrl(index);
            // 调试断言该桶必须处于满状态。
            debug_assert!(self.is_bucket_full(index));
            // 保存当前的剩余增长空间。
            let old_growth_left = self.table.growth_left;
            // 移除桶并取出旧元素。
            let item = self.remove(bucket).0;
            // 对旧元素应用函数 f。
            if let Some(new_item) = f(item) {
                // 恢复剩余增长空间。
                self.table.growth_left = old_growth_left;
                // 恢复旧的控制字节。
                self.table.set_ctrl(index, old_ctrl);
                // 元素数量加一。
                self.table.items += 1;
                // 将新元素写入桶中。
                self.bucket(index).write(new_item);
                // 返回 None，表示桶仍被占用。
                None
            // 若函数 f 返回 None（未提供新元素）。
            } else {
                // 返回 Some(旧控制字节)，表示桶已被清空。
                Some(old_ctrl)
            }
        }
    }

    /// 在表中搜索一个元素。如果未找到该元素，
    /// 则返回 `Err`，其中包含一个槽的位置，
    /// 具有相同哈希值的元素可以插入该槽中。
    ///
    /// 如果插入元素需要额外空间，此函数可能会调整表的大小，
    /// 以便完成插入。
    #[inline]
    // 查找元素，未找到时返回可插入相同哈希元素的槽位置。
    pub(crate) fn find_or_find_insert_index(
        // 可变借用自身。
        &mut self,
        // 元素的哈希值。
        hash: u64,
        // 相等性判断函数。
        mut eq: impl FnMut(&T) -> bool,
        // 用于重新哈希的哈希函数。
        hasher: impl Fn(&T) -> u64,
    ) -> Result<Bucket<T>, usize> {
        // 先预留一个元素的空间（可能触发扩容）。
        self.reserve(1, hasher);

        // 进入 unsafe 块。
        unsafe {
            // SAFETY:
            // 1. 我们可以确定表中至少有一个空的 `bucket`。
            // 2. [`RawTableInner`] 的控制字节必须已经正确初始化，因为我们绝不会
            //    在公共 API 中暴露 `RawTable::new_uninitialized`。
            // 3. `find_or_find_insert_index_inner` 函数只返回处于满状态的桶的 `index`，
            //    其位于 `0..self.num_buckets()` 范围内（因为表中至少有一个空的 `bucket`），
            //    因此调用 `self.bucket(index)` 和 `Bucket::as_ref` 是安全的。
            match self
                // 访问内部表。
                .table
                // 在内部表中查找，或查找可插入的索引。
                .find_or_find_insert_index_inner(hash, &mut |index| eq(self.bucket(index).as_ref()))
            // 匹配查找结果。
            {
                // SAFETY: 见上文的解释。
                Ok(index) => Ok(self.bucket(index)),
                // 未找到时直接返回插入位置的索引。
                Err(index) => Err(index),
            }
        }
    }

    /// 使用给定的哈希值在表中给定的索引处插入一个新元素，
    /// 并返回其原始桶。
    ///
    /// # Safety
    ///
    /// `index` 必须指向先前由
    /// `find_or_find_insert_index` 返回的槽，且自那次调用之后
    /// 表不能发生过任何变更。
    #[inline]
    // 在给定索引处插入新元素，并返回其原始桶。
    pub(crate) unsafe fn insert_at_index(
        // 可变借用自身。
        &mut self,
        // 元素的哈希值。
        hash: u64,
        // 插入位置的索引。
        index: usize,
        // 要插入的元素。
        value: T,
    ) -> Bucket<T> {
        // 以完整标签调用带标签的插入函数。
        unsafe { self.insert_tagged_at_index(Tag::full(hash), index, value) }
    }

    /// 使用给定的标签在表中给定的索引处插入一个新元素，
    /// 并返回其原始桶。
    ///
    /// # Safety
    ///
    /// `index` 必须指向先前由
    /// `find_or_find_insert_index` 返回的槽，且自那次调用之后
    /// 表不能发生过任何变更。
    #[inline]
    // 在给定索引处使用给定标签插入新元素。
    pub(crate) unsafe fn insert_tagged_at_index(
        // 可变借用自身。
        &mut self,
        // 与元素一起记录的标签。
        tag: Tag,
        // 插入位置的索引。
        index: usize,
        // 要插入的元素。
        value: T,
    ) -> Bucket<T> {
        // 进入 unsafe 块。
        unsafe {
            // 读取该索引处的旧控制字节。
            let old_ctrl = *self.table.ctrl(index);
            // 记录元素在给定索引处的插入。
            self.table.record_item_insert_at(index, old_ctrl, tag);

            // 获取该索引对应的桶。
            let bucket = self.bucket(index);
            // 将值写入桶中。
            bucket.write(value);
            // 返回该桶。
            bucket
        }
    }

    /// 在表中搜索一个元素。
    #[inline]
    // 根据哈希值查找元素，返回其所在的桶。
    pub(crate) fn find(&self, hash: u64, mut eq: impl FnMut(&T) -> bool) -> Option<Bucket<T>> {
        // 进入 unsafe 块。
        unsafe {
            // SAFETY:
            // 1. [`RawTableInner`] 的控制字节必须已经正确初始化，因为我们绝不会
            //    在公共 API 中暴露 `RawTable::new_uninitialized`。
            // 1. `find_inner` 函数只返回处于满状态的桶的 `index`，其位于
            //    `0..self.num_buckets()` 范围内，因此调用 `self.bucket(index)` 和
            //    `Bucket::as_ref` 是安全的。
            let result = self
                // 访问内部表。
                .table
                // 在内部表中查找元素。
                .find_inner(hash, &mut |index| eq(self.bucket(index).as_ref()));

            // 避免 `Option::map`，因为它会使 LLVM IR 变得臃肿。
            match result {
                // SAFETY: 见上文的解释。
                Some(index) => Some(self.bucket(index)),
                // 未找到时返回 None。
                None => None,
            }
        }
    }

    /// 获取表中某个元素的引用。
    #[inline]
    // 根据哈希值和相等性判断获取表中元素的引用。
    pub(crate) fn get(&self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&T> {
        // 避免 `Option::map`，因为它会使 LLVM IR 变得臃肿。
        match self.find(hash, eq) {
            // 找到时将桶转换为元素的不可变引用。
            Some(bucket) => Some(unsafe { bucket.as_ref() }),
            // 未找到时返回 None。
            None => None,
        }
    }

    /// 获取表中某个元素的可变引用。
    #[inline]
    // 根据哈希值和相等性判断获取表中元素的可变引用。
    pub(crate) fn get_mut(&mut self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&mut T> {
        // 避免 `Option::map`，因为它会使 LLVM IR 变得臃肿。
        match self.find(hash, eq) {
            // 找到时将桶转换为元素的可变引用。
            Some(bucket) => Some(unsafe { bucket.as_mut() }),
            // 未找到时返回 None。
            None => None,
        }
    }

    /// 获取表中给定桶索引处元素的引用。
    #[inline]
    // 根据桶索引获取表中元素的引用。
    pub(crate) fn get_bucket(&self, index: usize) -> Option<&T> {
        // 进入 unsafe 块。
        unsafe {
            // 若索引有效且对应的桶处于满状态。
            if index < self.num_buckets() && self.is_bucket_full(index) {
                // 返回该桶中元素的引用。
                Some(self.bucket(index).as_ref())
            // 否则。
            } else {
                // 返回 None。
                None
            }
        }
    }

    /// 获取表中给定桶索引处元素的可变引用。
    #[inline]
    // 根据桶索引获取表中元素的可变引用。
    pub(crate) fn get_bucket_mut(&mut self, index: usize) -> Option<&mut T> {
        // 索引与桶状态的检查依赖内部不变量，需在 unsafe 块中进行。
        unsafe {
            // 仅当索引在范围内且对应的桶已被占用时才返回元素。
            if index < self.num_buckets() && self.is_bucket_full(index) {
                // 索引有效，返回该桶中元素的可变引用。
                Some(self.bucket(index).as_mut())
            } else {
                // 否则返回 None。
                None
            }
        }
    }

    /// 返回指向表中某个元素的指针，但只有在验证索引未越界且
    /// 该桶已被占用之后才返回。
    #[inline]
    pub(crate) fn checked_bucket(&self, index: usize) -> Option<Bucket<T>> {
        // 桶访问依赖内部不变量，需在 unsafe 块中进行。
        unsafe {
            // 仅当索引在范围内且对应的桶已被占用时才返回桶。
            if index < self.num_buckets() && self.is_bucket_full(index) {
                // 索引有效，返回对应的桶。
                Some(self.bucket(index))
            } else {
                // 无效则返回 None。
                None
            }
        }
    }

    /// 尝试一次性获取表中 `N` 个条目的可变引用。
    ///
    /// 返回一个长度为 `N` 的数组，包含每次查询的结果。
    ///
    /// 至多只会为任意一个条目返回一个可变引用。若有任何哈希值重复，则返回
    /// `None`；若哈希值未找到，也返回 `None`。
    ///
    /// `eq` 参数应为一个闭包，使得当 `k` 等于第 `i` 个
    /// 待查找的键时，`eq(i, k)` 返回 true。
    pub(crate) fn get_disjoint_mut<const N: usize>(
        &mut self,
        // 要查找的哈希值数组。
        hashes: [u64; N],
        // 用于判断第 i 个键是否匹配的闭包。
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        // 将查找到的指针转换为可变引用，需在 unsafe 块中进行。
        unsafe {
            // 一次性查找 N 个哈希对应的条目，得到原始指针数组。
            let ptrs = self.get_disjoint_mut_pointers(hashes, eq);

            // 遍历所有查询结果，进行调试期去重检查。
            for (i, cur) in ptrs.iter().enumerate() {
                // 断言当前结果未在之前的结果中出现过。
                assert!(
                    !(cur.is_some() && ptrs[..i].contains(cur)),
                    "duplicate keys found"
                );
            }
            // 所有桶都与之前的桶互不相同，因此可以
            // 放心地返回查找的结果。

            // 将每个指针结果转换为可变引用。
            ptrs.map(|ptr| ptr.map(|mut ptr| ptr.as_mut()))
        }
    }

    // 类似 `get_disjoint_mut`，但跳过去重检查，安全性由调用者保证。
    pub(crate) unsafe fn get_disjoint_unchecked_mut<const N: usize>(
        &mut self,
        // 要查找的哈希值数组。
        hashes: [u64; N],
        // 用于判断第 i 个键是否匹配的闭包。
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        // 一次性查找 N 个哈希对应的条目，得到原始指针数组。
        let ptrs = unsafe { self.get_disjoint_mut_pointers(hashes, eq) };
        // 将每个指针转换为可变引用，安全性由调用者保证。
        ptrs.map(|ptr| ptr.map(|mut ptr| unsafe { ptr.as_mut() }))
    }

    // 对每个哈希调用 `find`，返回查找到的条目的原始指针数组。
    unsafe fn get_disjoint_mut_pointers<const N: usize>(
        &mut self,
        // 要查找的哈希值数组。
        hashes: [u64; N],
        // 用于判断第 i 个键是否匹配的闭包。
        mut eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<NonNull<T>>; N] {
        // 对每个索引 i 查找对应哈希的条目。
        array::from_fn(|i| {
            // 在表中查找第 i 个哈希，并用闭包验证键是否相等。
            self.find(hashes[i], |k| eq(i, k))
                // 将找到的桶转换为非空指针。
                .map(|cur| cur.as_non_null())
        })
    }

    /// 返回 map 在不重新分配内存的情况下能容纳的元素数量。
    ///
    /// 这个数字只是下界；表可能会容纳更多，
    /// 但保证至少能容纳这么多。
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        // 容量等于已存元素数加上剩余可增长空间。
        self.table.items + self.table.growth_left
    }

    /// 返回表中的元素数量。
    #[inline]
    pub(crate) fn len(&self) -> usize {
        // 直接返回表中已存元素的数量。
        self.table.items
    }

    /// 如果表不包含任何元素，则返回 `true`。
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        // 通过判断元素数量是否为 0 来确定表是否为空。
        self.len() == 0
    }

    /// 返回表中的桶数量。
    #[inline]
    pub(crate) fn num_buckets(&self) -> usize {
        // 桶数等于桶掩码加 1。
        self.table.bucket_mask + 1
    }

    /// 检查 `index` 处的桶是否已被占用。
    ///
    /// # Safety
    ///
    /// 调用者必须保证 `index` 小于桶的数量。
    #[inline]
    pub(crate) unsafe fn is_bucket_full(&self, index: usize) -> bool {
        // 将检查委托给表内部的同名方法。
        unsafe { self.table.is_bucket_full(index) }
    }

    /// 返回一个遍历表中每个元素的迭代器。调用者必须保证
    /// `RawTable` 的存活时间长于 `RawIter`。由于我们无法将
    /// `RawIter` 结构体的 `next` 方法标记为 unsafe，
    /// 因此必须将 `iter` 方法标记为 unsafe。
    #[inline]
    pub(crate) unsafe fn iter(&self) -> RawIter<T> {
        // SAFETY:
        // 1. 调用者必须遵守 `iter` 方法的安全契约。
        // 2. [`RawTableInner`] 的控制字节必须已经正确初始化，因为我们
        //    绝不会在公共 API 中暴露 RawTable::new_uninitialized。
        unsafe { self.table.iter() }
    }

    /// 返回一个遍历可能匹配给定哈希的已占用桶的迭代器。
    ///
    /// `RawTable` 只存储哈希值的 7 位，因此该迭代器可能
    /// 返回哈希值与所提供值不同的条目。使用返回的值之前，
    /// 你应当始终先进行验证。
    ///
    /// 调用者必须保证 `RawTable` 的存活时间长于
    /// `RawIterHash`。由于我们无法将 `RawIterHash` 结构体的
    /// `next` 方法标记为 unsafe，因此必须将 `iter_hash` 方法标记为 unsafe。
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) unsafe fn iter_hash(&self, hash: u64) -> RawIterHash<T> {
        // 直接构造哈希迭代器。
        unsafe { RawIterHash::new(self, hash) }
    }

    /// 返回一个遍历可能匹配给定哈希的已占用桶索引的迭代器。
    ///
    /// `RawTable` 只存储哈希值的 7 位，因此该迭代器可能
    /// 返回哈希值与所提供值不同的条目。使用返回的值之前，
    /// 你应当始终先进行验证。
    ///
    /// 调用者必须保证 `RawTable` 的存活时间长于
    /// `RawIterHashIndices`。由于我们无法将 `RawIterHashIndices` 结构体的
    /// `next` 方法标记为 unsafe，因此必须将 `iter_hash_buckets` 方法标记为 unsafe。
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) unsafe fn iter_hash_buckets(&self, hash: u64) -> RawIterHashIndices {
        // 直接构造桶索引迭代器。
        unsafe { RawIterHashIndices::new(&self.table, hash) }
    }

    /// 返回一个遍历表中已占用桶索引的迭代器。
    ///
    /// 安全条件参见 [`RawTableInner::full_buckets_indices`]。
    #[inline(always)]
    pub(crate) unsafe fn full_buckets_indices(&self) -> FullBucketsIndices {
        // 委托给表内部的同名方法。
        unsafe { self.table.full_buckets_indices() }
    }

    /// 返回一个迭代器，它会在不释放内存的情况下
    /// 移除表中的所有元素。
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) fn drain(&mut self) -> RawDrain<'_, T, A> {
        // `drain_iter_from` 是 unsafe 方法，需在 unsafe 块中调用。
        unsafe {
            // 先创建覆盖整个表的迭代器。
            let iter = self.iter();
            // 从该迭代器的当前位置开始抽取所有元素。
            self.drain_iter_from(iter)
        }
    }

    /// 返回一个迭代器，它会在不释放内存的情况下
    /// 移除表中的所有元素。
    ///
    /// 迭代从提供的迭代器当前位置开始。
    ///
    /// 调用者必须保证该迭代器对此 `RawTable` 有效，
    /// 且覆盖表中剩余的所有元素。
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) unsafe fn drain_iter_from(&mut self, iter: RawIter<T>) -> RawDrain<'_, T, A> {
        // 调试断言：迭代器的剩余长度必须等于表的元素数。
        debug_assert_eq!(iter.len(), self.len());
        // 构造 RawDrain 结构体。
        RawDrain {
            // 沿用传入的迭代器。
            iter,
            // 将自身的表替换为空表单例，并把原表交给抽取迭代器。
            table: mem::replace(&mut self.table, RawTableInner::NEW),
            // 指向替换后的新表，用于抽取结束时恢复。
            orig_table: NonNull::from(&mut self.table),
            // 用于变型检查的标记类型。
            marker: PhantomData,
        }
    }

    /// 返回一个消费表中所有元素的迭代器。
    ///
    /// 迭代从提供的迭代器当前位置开始。
    ///
    /// 调用者必须保证该迭代器对此 `RawTable` 有效，
    /// 且覆盖表中剩余的所有元素。
    pub(crate) unsafe fn into_iter_from(self, iter: RawIter<T>) -> RawIntoIter<T, A> {
        // 调试断言：迭代器的剩余长度必须等于表的元素数。
        debug_assert_eq!(iter.len(), self.len());

        // 获取表的底层分配信息（指针、布局与分配器）。
        let allocation = self.into_allocation();
        // 构造消费迭代器。
        RawIntoIter {
            // 沿用传入的迭代器。
            iter,
            // 底层内存分配信息。
            allocation,
            // 用于变型检查的标记类型。
            marker: PhantomData,
        }
    }

    /// 将表转换为原始内存分配。在释放该分配之前，
    /// 应先使用 `RawIter` 来 drop 表中的内容。
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) fn into_allocation(self) -> Option<(NonNull<u8>, Layout, A)> {
        // 空表单例没有实际分配，返回 None；否则计算布局并交出分配。
        let alloc = if self.table.is_empty_singleton() {
            // 没有底层分配可返回。
            None
        } else {
            // 计算该表的内存布局以及控制字节的偏移量。
            let (layout, ctrl_offset) = {
                // 根据桶数量计算布局。
                let option = Self::TABLE_LAYOUT.calculate_layout_for(self.table.num_buckets());
                // 桶数量合法时布局计算必然成功，可安全地省略检查。
                unsafe { option.unwrap_unchecked() }
            };
            // 返回指向分配起始处的指针、布局以及分配器。
            Some((
                // 指针回退 ctrl_offset，指向分配的起始地址。
                unsafe { NonNull::new_unchecked(self.table.ctrl.as_ptr().sub(ctrl_offset).cast()) },
                // 内存的布局信息。
                layout,
                // 按位复制分配器（稍后 mem::forget 自身，避免重复 drop）。
                unsafe { ptr::read(&raw const self.alloc) },
            ))
        };
        // 阻止自身的 drop 运行，避免分配器被重复释放。
        mem::forget(self);
        // 返回分配信息。
        alloc
    }
}
// unsafe impl：为 RawTable 手动实现 Send 标记 trait（声明该表可跨线程转移所有权）。
unsafe impl<T, A: Allocator> Send for RawTable<T, A>
// where 子句：为 T 和 A 补充 Send 约束。
where
    // 要求元素类型 T 是 Send。
    T: Send,
    // 要求分配器 A 是 Send。
    A: Send,
// 空的实现体（Send 仅为标记 trait，无需方法）。
{
}
// unsafe impl：为 RawTable 手动实现 Sync 标记 trait（声明该表可跨线程共享引用）。
unsafe impl<T, A: Allocator> Sync for RawTable<T, A>
// where 子句：为 T 和 A 补充 Sync 约束。
where
    // 要求元素类型 T 是 Sync。
    T: Sync,
    // 要求分配器 A 是 Sync。
    A: Sync,
// 空的实现体（Sync 仅为标记 trait，无需方法）。
{
}

// RawTableInner 的固有实现：空表常量与构造函数。
impl RawTableInner {
    // 关联常量：预构建的空表实例，可直接复用。
    const NEW: Self = RawTableInner::new();

    /// 创建一个不分配任何内存的新的空哈希表。
    ///
    /// 实际上，这会返回一个恰好只有 1 个桶的表。不过，我们可以让数据指针保持悬垂，
    /// 因为我们的负载因子迫使我们始终至少保留 1 个空闲桶，所以该桶永远不会被访问。
    #[inline]
    // 创建空表的 const 构造函数（可在编译期求值）。
    const fn new() -> Self {
        // 逐字段构造 Self。
        Self {
            // 注意要把整个切片都转换为裸指针。
            ctrl: unsafe {
                // 将静态空控制字节切片转为非空指针（不可能为 null）。
                NonNull::new_unchecked(Group::static_empty().as_ptr().cast_mut().cast())
            },
            // bucket_mask = 桶数 - 1（0 表示只有 1 个桶）。
            bucket_mask: 0,
            // 元素数为 0。
            items: 0,
            // 剩余可增长容量为 0。
            growth_left: 0,
        }
    }
}

/// 求不大于 z 的最大的 2 的幂（上一个 2 的幂）；若 z 本身已是 2 的幂则保持不变。
/// 传入 0 是未定义行为。
pub(crate) fn prev_pow2(z: usize) -> usize {
    // 最高位的位索引（usize 位数减 1）。
    let shift = usize::BITS as usize - 1;
    // 将 1 左移到 z 的最高有效位，得到不大于 z 的最大 2 的幂。
    1 << (shift - (z.leading_zeros() as usize))
}

/// 在给定 TableLayout 的前提下，找出能放入 `allocation_size` 中的最大桶数。
///
/// 这依赖于 `capacity_to_buckets` 的一些不变式，因此只能传入由
/// `capacity_to_buckets` 计算得到的 `allocation_size`。
fn maximum_buckets_in(
    // 分配块的总大小（字节）。
    allocation_size: usize,
    // 表布局（元素大小与对齐信息）。
    table_layout: TableLayout,
    // 组宽度（一次 SIMD 加载覆盖的控制字节数）。
    group_width: usize,
) -> usize {
    // 对于形如下式的方程：
    //   z >= x * y + x + g
    // 可以通过下式来最大化 x：
    //   x = (z - g) / (y + 1)
    // 对号入座（各变量对应关系）：
    //   x 是桶的数量
    //   y 是 table_layout.size（每个桶的元素大小）
    //   z 是分配块的大小
    //   g 是组的宽度
    // 但这样忽略了 ctrl_align 所需的对齐填充。
    // 如果记住以下限制条件：
    //   x 总是 2 的幂
    //   T 的布局大小必须是 T 大小的整数倍
    // 那么只要加上约束：
    //   x * y >= table_layout.ctrl_align
    // 对齐填充就可以忽略。
    // 这一点由 `capacity_to_buckets` 负责保证。
    // 记住下面这个式子有助于理解：
    //   ctrl_offset = align(x * y, ctrl_align)
    let x = (allocation_size - group_width) / (table_layout.size + 1);
    // 向下取整到 2 的幂，得到实际可用的最大桶数。
    prev_pow2(x)
}

// RawTableInner 的固有实现：分配与容量初始化。
impl RawTableInner {
    /// 以给定数量的桶分配一个新的 [`RawTableInner`]。
    /// 控制字节和桶（数据区）都保持未初始化状态。
    ///
    /// # Safety
    ///
    /// 调用者必须保证 `buckets` 是 2 的幂，并且还必须用 [`Tag::EMPTY`] 字节初始化
    /// 长度为 `self.bucket_mask + 1 + Group::WIDTH` 的所有控制字节。
    ///
    /// 其他安全性事项请参见 [`Allocator`] API。
    ///
    /// [`Allocator`]: stdalloc::alloc::Allocator
    #[cfg_attr(feature = "inline-more", inline)]
    // 按未初始化状态分配新表（unsafe：调用方需负责后续初始化控制字节）。
    unsafe fn new_uninitialized<A>(
        // 用于分配内存的分配器。
        alloc: &A,
        // 表布局（元素大小与对齐）。
        table_layout: TableLayout,
        // 桶数（2 的幂，可能因超额分配被调大）。
        mut buckets: usize,
        // 容错策略（分配失败时返回错误还是中止）。
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError>
    // where 子句：要求 A 实现分配器 trait。
    where
        // A 必须是分配器。
        A: Allocator,
    {
        // 调试断言：桶数必须是 2 的幂。
        debug_assert!(buckets.is_power_of_two());

        // 避免使用 `Option::ok_or_else`，因为它会使 LLVM IR 膨胀。
        let Some((layout, mut ctrl_offset)) = table_layout.calculate_layout_for(buckets) else {
            // 布局计算溢出：按容错策略返回容量溢出错误。
            return Err(fallibility.capacity_overflow());
        };

        // 执行实际分配，并根据结果分支处理。
        let ptr: NonNull<u8> = match do_alloc(alloc, layout) {
            // 分配成功分支。
            Ok(block) => {
                // 分配器返回的内存块不会小于请求的大小，
                // 因此这里可以用 != 而不是 >=。
                if block.len() != layout.size() {
                    // 利用超额分配（分配器多给的部分）。
                    let x = maximum_buckets_in(block.len(), table_layout, Group::WIDTH);
                    // 调试断言：实际可容纳的桶数不少于请求的桶数。
                    debug_assert!(x >= buckets);
                    // 计算新的 ctrl_offset。
                    let (oversized_layout, oversized_ctrl_offset) = {
                        // 按超额后的桶数 x 重新计算布局。
                        let option = table_layout.calculate_layout_for(x);
                        // 安全：既然 x >= buckets 且原布局计算成功，则此布局计算也一定成功。
                        unsafe { option.unwrap_unchecked() }
                    };
                    // 调试断言：超额布局的总大小不超过实际分配块大小。
                    debug_assert!(oversized_layout.size() <= block.len());
                    // 调试断言：新的 ctrl_offset 不小于原值。
                    debug_assert!(oversized_ctrl_offset >= ctrl_offset);
                    // 采用超额布局的控制字节偏移。
                    ctrl_offset = oversized_ctrl_offset;
                    // 更新桶数为实际可容纳的数量。
                    buckets = x;
                }

                // 将分配块指针转换为 u8 指针。
                block.cast()
            }
            // 分配失败分支：按容错策略处理分配错误。
            Err(_) => return Err(fallibility.alloc_err(layout)),
        };

        // SAFETY: 空指针的情况会在上面的检查中被捕获。
        let ctrl = unsafe { NonNull::new_unchecked(ptr.as_ptr().add(ctrl_offset)) };
        // 构造并返回该表。
        Ok(Self {
            // 指向控制字节数组起点的非空指针。
            ctrl,
            // bucket_mask = 桶数 - 1。
            bucket_mask: buckets - 1,
            // 初始元素数为 0。
            items: 0,
            // 剩余增长容量 = 按负载因子换算出的初始容量。
            growth_left: bucket_mask_to_capacity(buckets - 1),
        })
    }

    /// 尝试分配一个新的 [`RawTableInner`]，其容量至少足以在不重新分配的情况下
    /// 插入给定数量的元素。
    ///
    /// 所有控制字节都会用 [`Tag::EMPTY`] 字节初始化。
    #[inline]
    // 可失败的按容量分配：无法满足时返回错误而不是 panic。
    fn fallible_with_capacity<A>(
        // 用于分配内存的分配器。
        alloc: &A,
        // 表布局（元素大小与对齐）。
        table_layout: TableLayout,
        // 期望的最小容量（元素个数）。
        capacity: usize,
        // 容错策略。
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError>
    // where 子句：要求 A 实现分配器 trait。
    where
        // A 必须是分配器。
        A: Allocator,
    {
        // 容量为 0 时直接返回预置的空表。
        if capacity == 0 {
            // 返回静态空表常量。
            Ok(Self::NEW)
        } else {
            // SAFETY: 我们已检查能够成功分配新表，随后会用常量 `Tag::EMPTY` 字节
            // 初始化所有控制字节。
            unsafe {
                // 根据容量计算桶数（可能因容量溢出而失败）。
                let buckets = capacity_to_buckets(capacity, table_layout)
                    // 计算失败则返回容量溢出错误。
                    .ok_or_else(|| fallibility.capacity_overflow())?;

                // 以未初始化状态分配表（控制字节尚未写入）。
                let mut result =
                    // 分配失败则向上传播错误。
                    Self::new_uninitialized(alloc, table_layout, buckets, fallibility)?;
                // SAFETY: 我们已检查表已成功分配，因此该表已经拥有
                // `self.bucket_mask + 1 + Group::WIDTH` 个控制字节（见 TableLayout::calculate_layout_for），
                // 所以写入 `self.num_ctrl_bytes() == bucket_mask + 1 + Group::WIDTH` 个字节是安全的。
                result.ctrl_slice().fill_empty();

                // 返回初始化完成的表。
                Ok(result)
            }
        }
    }

    /// 分配一个新的 [`RawTableInner`]，其容量至少足以在不重新分配的情况下
    /// 插入给定数量的元素。
    ///
    /// 若新容量超过 [`isize::MAX`] 字节则会 panic；发生分配错误时会 [`abort`] 中止程序。
    /// 如果你想自行处理内存分配失败，请改用 [`fallible_with_capacity`]。
    ///
    /// 所有控制字节都会用 [`Tag::EMPTY`] 字节初始化。
    ///
    /// [`fallible_with_capacity`]: RawTableInner::fallible_with_capacity
    /// [`abort`]: stdalloc::abort::handle_alloc_error
    fn with_capacity<A>(alloc: &A, table_layout: TableLayout, capacity: usize) -> Self
    // where 子句：要求 A 实现分配器 trait。
    where
        // A 必须是分配器。
        A: Allocator,
    {
        // 以"不可失败"策略调用可失败版本进行分配。
        let result =
            // Infallible 策略下不会返回 Err（分配失败会直接中止程序）。
            Self::fallible_with_capacity(alloc, table_layout, capacity, Fallibility::Infallible);

        // SAFETY: 所有分配错误都会在 `RawTableInner::new_uninitialized` 内部被处理掉，
        // 因此这里不可能返回 Err。
        unsafe { result.unwrap_unchecked() }
    }

    /// 修正由 [`RawTableInner::find_insert_index_in_group`] 方法返回的插入索引。
    ///
    /// 在小于组宽度的表中（`self.num_buckets() < Group::WIDTH`），表范围之外的尾部控制
    /// 字节会被填充为 [`Tag::EMPTY`]。遗憾的是，这会触发 [`RawTableInner::find_insert_index_in_group`]
    /// 函数的一次匹配。原因在于：`group.match_empty_or_deleted().lowest_set_bit()` 返回的
    /// `Some(bit)` 在经过掩码运算（`(probe_seq.pos + bit) & self.bucket_mask`）之后，
    /// 可能指向一个已被占用的满桶。我们在这里检测这种情况，并从表的开头重新进行第二次扫描。
    /// 由于负载因子的存在，第二次扫描一定会在碰到尾部控制字节（包含 [`Tag::EMPTY`] 字节）
    /// 之前找到一个空槽位。
    ///
    /// 如果本函数被正确调用，则保证返回 `0..self.num_buckets()` 范围内某个空桶或
    /// 墓碑（deleted）桶的索引（见 `Warning` 和 `Safety`）。
    ///
    /// # Warning
    ///
    /// 表必须至少有 1 个空桶或墓碑（deleted）桶；否则，当表小于组宽度时
    /// （`self.num_buckets() < Group::WIDTH`），本函数会返回一个超出表索引范围
    /// `0..self.num_buckets()`（`0..=self.bucket_mask`）的索引。向该索引写入数据
    /// 会立即导致 [`undefined behavior`]。
    ///
    /// # Safety
    ///
    /// 这些安全规则直接派生自 [`RawTableInner::ctrl`] 方法的安全规则。
    /// 因此，为了维护这些安全性契约，以及保证本 crate 的逻辑正确，以下规则是
    /// 必要且充分的：
    ///
    /// * [`RawTableInner`] 的控制字节必须已正确初始化，否则调用本函数会导致
    ///   [`undefined behavior`]。
    ///
    /// * 本函数只能用于 [`RawTableInner::find_insert_index_in_group`] 找到的插入索引
    ///   （即在该函数之后、真正插入表之前使用）。
    ///
    /// * `index` 不得大于 `self.bucket_mask`，即 `(index + 1) <= self.num_buckets()`
    ///   （该条件由 [`RawTableInner::find_insert_index_in_group`] 函数保证）。
    ///
    /// 若传入本函数的索引并非由 [`RawTableInner::find_insert_index_in_group`] 提供，
    /// 即使该索引满足 [`RawTableInner::ctrl`] 函数的安全规则
    /// （`index < self.bucket_mask + 1 + Group::WIDTH`），也可能导致 [`undefined behavior`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 修正插入索引：若索引指向满桶，则从表开头重新扫描（unsafe）。
    unsafe fn fix_insert_index(&self, mut index: usize) -> usize {
        // SAFETY: 本函数的调用者保证 `index` 位于 `0..=self.bucket_mask` 范围内。
        if unlikely(unsafe { self.is_bucket_full(index) }) {
            // 调试断言：只有小于组宽度的表才会走到这里。
            debug_assert!(self.bucket_mask < Group::WIDTH);
            // SAFETY:
            //
            // * 由于本函数的调用者保证控制字节已正确初始化，且 `ptr = self.ctrl(0)`
            //   指向控制字节数组的起始位置，因此：`ctrl` 用于读取是有效的，按
            //   `Group::WIDTH` 正确对齐，并指向已正确初始化的控制字节（另见
            //   `TableLayout::calculate_layout_for` 和 `ptr::read`）；
            //
            // * 由于本函数的调用者保证索引是由 `self.find_insert_index_in_group()`
            //   函数提供的，因此对于大于组宽度的表（self.num_buckets() >= Group::WIDTH），
            //   我们绝不会进入该分支，因为 `find_insert_index_in_group` 中的
            //   `(probe_seq.pos + bit) & self.bucket_mask` 不可能返回满桶索引。对于
            //   小于组宽度的表，调用 `unwrap_unchecked` 也是安全的，因为表范围之外
            //   的尾部控制字节填充的是 EMPTY 字节（而且我们确信至少存在一个 FULL 桶），
            //   所以这次第二次扫描要么找到一个空槽位（由于负载因子），要么命中尾部
            //   控制字节（包含 EMPTY）。
            index = unsafe {
                // 对齐加载表起始处的控制字节组。
                Group::load_aligned(self.ctrl(0))
                    // 匹配空桶或墓碑（deleted）桶。
                    .match_empty_or_deleted()
                    // 取最低置位比特（第一个空/墓碑槽位）。
                    .lowest_set_bit()
                    // 安全性见上方 SAFETY 注释（必定能找到槽位）。
                    .unwrap_unchecked()
            };
        }
        // 返回修正后的插入索引。
        index
    }

    /// 在一个组内查找可插入的位置。
    ///
    /// **该结果可能有误报（false positive），在使用之前必须用 `fix_insert_index`
    /// 加以修正。**
    ///
    /// 本函数保证返回 `0..self.num_buckets()`（`0..=self.bucket_mask`）范围内
    /// 某个空桶或墓碑（deleted）[`Bucket`] 的索引。
    #[inline]
    // 在单个组内查找插入位置（可能有误报，需再经 fix_insert_index 修正）。
    fn find_insert_index_in_group(&self, group: &Group, probe_seq: &ProbeSeq) -> Option<usize> {
        // 查找组内第一个空桶或墓碑（deleted）桶对应的比特位。
        let bit = group.match_empty_or_deleted().lowest_set_bit();

        // 若组内存在空桶或墓碑（deleted）桶的比特位。
        if likely(bit.is_some()) {
            // 这等价于 `(probe_seq.pos + bit) % self.num_buckets()`，因为桶数是 2 的幂，
            // 且 `self.bucket_mask = self.num_buckets() - 1`。
            Some((probe_seq.pos + bit.unwrap()) & self.bucket_mask)
        } else {
            // 组内没有可插入的槽位。
            None
        }
    }

    /// 在表中搜索某个元素，或者搜索一个可以插入该元素的潜在槽位
    /// （空桶或墓碑（deleted）[`Bucket`] 的索引）。
    ///
    /// 这里使用动态分发来减少生成的代码量，但会被 LLVM 优化消除。
    ///
    /// 本函数不会对表的 `data` 部分做任何修改，也不会修改表的
    /// `items` 或 `growth_left` 字段。
    ///
    /// 表必须至少有 1 个空桶或墓碑（deleted）桶；否则，如果
    /// `eq: &mut dyn FnMut(usize) -> bool` 函数不返回 `true`，那么对于大于组宽度的表，
    /// 本函数将永远不会返回（陷入无限循环）；而对于小于组宽度的表，
    /// 则会返回一个超出表索引范围的索引。
    ///
    /// 本函数保证只会把 `FULL` 桶的索引提供给 `eq: &mut dyn FnMut(usize) -> bool`
    /// 函数，并在找到元素时返回其 `index`（即 `Ok(index)`）。如果未找到元素，
    /// 且表中至少有 1 个空桶或墓碑（deleted）[`Bucket`]，则函数保证返回
    /// `0..self.num_buckets()` 范围内的索引；但无论如何，若本函数返回 `Err`，
    /// 其中包含的索引一定在 `0..=self.num_buckets()` 范围内。
    ///
    /// # Safety
    ///
    /// [`RawTableInner`] 的控制字节必须已正确初始化，否则调用
    /// 本函数会导致 [`undefined behavior`]。
    ///
    /// 当表小于组宽度、且表中原本不存在至少一个空桶或墓碑（deleted）桶时，
    /// 若向本函数返回的索引写入数据，会立即导致 [`undefined behavior`]。这是因为
    /// 在这种情况下，由于表范围之外的尾部 [`Tag::EMPTY`] 控制字节，函数会把
    /// `self.bucket_mask + 1` 作为索引返回。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 查找元素；若未找到则顺便记录一个可能的插入槽位（unsafe）。
    unsafe fn find_or_find_insert_index_inner(
        // 对表内部的不可变引用。
        &self,
        // 要查找的元素的完整哈希值。
        hash: u64,
        // 相等性判断回调：传入候选桶索引，返回是否匹配。
        eq: &mut dyn FnMut(usize) -> bool,
    ) -> Result<usize, usize> {
        // 暂存的插入槽位索引（尚未找到时为 None）。
        let mut insert_index = None;

        // 用哈希值构造完整的控制字节标签。
        let tag_hash = Tag::full(hash);
        // 根据哈希值初始化探测序列。
        let mut probe_seq = self.probe_seq(hash);

        // 沿探测序列逐组扫描。
        loop {
            // SAFETY:
            // * 本函数的调用者保证控制字节已正确初始化。
            //
            // * 由于经过 `self.bucket_mask` 掩码运算，且桶数是 2 的幂，因此
            //   `ProbeSeq.pos` 不会大于表的 `self.bucket_mask = self.num_buckets() - 1`
            //   （见 `self.probe_seq` 函数）。
            //
            // * 即使 `ProbeSeq.pos` 返回 `position == self.bucket_mask`，由于控制字节
            //   范围被扩展为 `self.bucket_mask + 1 + Group::WIDTH`，调用 `Group::load`
            //   也是安全的（事实上，这意味着对于已分配的表，最后一个控制字节
            //   永远不会被读取）；
            //
            // * 此外，即使 `RawTableInner` 尚未分配，`ProbeSeq.pos` 也总是返回
            //   "0"（零），因此 Group::load 会按非对齐方式读取 `Group::static_empty()`
            //   的字节，这是安全的（见 RawTableInner::new）。
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            // 遍历组内与目标标签哈希匹配的桶。
            for bit in group.match_tag(tag_hash) {
                // 把组内比特位换算成表内的实际桶索引。
                let index = (probe_seq.pos + bit) & self.bucket_mask;

                // 调用回调判断该桶中的元素是否就是我们要找的。
                if likely(eq(index)) {
                    // 找到匹配元素，返回其索引。
                    return Ok(index);
                }
            }

            // 在当前组中未找到目标元素；若尚未记录插入槽位，
            // 则尝试从该组获取一个插入槽位。
            if likely(insert_index.is_none()) {
                // 从当前组中获取一个（可能需要修正的）插入槽位。
                insert_index = self.find_insert_index_in_group(&group, &probe_seq);
            }

            // 若已经拿到插入槽位。
            if let Some(insert_index) = insert_index {
                // 只有当组内至少存在一个空桶时才停止搜索。
                // 否则，要找的元素可能位于后续的组中。
                if likely(group.match_empty().any_bit_set()) {
                    // 此时我们必定已经找到了一个插入槽位，因为当前组至少包含一个空桶。
                    // 对于小于组宽度的表，由于负载因子的存在，当前（也是唯一的）组中
                    // 仍然会有一个空桶。
                    unsafe {
                        // SAFETY:
                        // * 本函数的调用者保证控制字节已正确初始化。
                        //
                        // * 我们使用的索引是由 `self.find_insert_index_in_group` 找到的。
                        return Err(self.fix_insert_index(insert_index));
                    }
                }
            }

            // 推进探测序列，继续检查下一组。
            probe_seq.move_next(self.bucket_mask);
        }
    }

    /// 搜索一个适合插入新元素的空桶或墓碑（deleted）桶，并为该槽位写入哈希值。
    /// 返回该槽位的索引，以及在找到的索引处存储的旧控制字节。
    ///
    /// 本函数不会检查给定元素是否已存在于表中；同样，也不会检查表中
    /// 是否有足够的空间插入新元素。函数调用者必须确保表中至少有 1 个空桶或
    /// 墓碑（deleted）桶；否则，对于大于组宽度的表，本函数将永远不会返回
    /// （陷入无限循环）；而对于小于组宽度的表，则会返回一个超出表索引范围的索引。
    ///
    /// 如果表中至少有 1 个空桶或墓碑（deleted）桶，则函数保证返回
    /// `0..self.num_buckets()` 范围内的 `index`；但无论如何，只要本函数返回了
    /// `index`，它就一定在 `0..=self.num_buckets()` 范围内。
    ///
    /// 本函数不会对表的 `data` 部分做任何修改，也不会修改表的
    /// `items` 或 `growth_left` 字段。
    ///
    /// # Safety
    ///
    /// 这些安全规则直接派生自 [`RawTableInner::set_ctrl_hash`] 和
    /// [`RawTableInner::find_insert_index`] 方法的安全规则。因此，为了维护这些方法
    /// 的安全性契约，以及保证本 crate 的逻辑正确，调用本函数时必须遵守以下规则：
    ///
    /// * [`RawTableInner`] 必须已完成分配且控制字节已正确初始化，否则调用本函数
    ///   会导致 [`undefined behavior`]。
    ///
    /// * 本函数的调用者必须确保在调用本函数之后，立即在返回的索引处
    ///   （与给定哈希匹配）写入表的 "data" 部分的条目。
    ///
    /// 当表小于组宽度、且表中原本不存在至少一个空桶或墓碑（deleted）桶时，
    /// 若向本函数返回的 `index` 写入数据，会立即导致 [`undefined behavior`]。这是因为
    /// 在这种情况下，由于表范围之外的尾部 [`Tag::EMPTY`] 控制字节，函数会把
    /// `self.bucket_mask + 1` 作为索引返回。
    ///
    /// 调用者必须自行增加表的 `items` 字段；此外，如果旧控制字节是 [`Tag::EMPTY`]，
    /// 还要减少表的 `growth_left` 字段；如果旧控制字节是 [`Tag::DELETED`]，
    /// 则保持 `growth_left` 不变。
    ///
    /// 关于如何正确地从 [`RawTable`] / [`RawTableInner`] 中移除或保存 `element`，
    /// 另见 [`Bucket::as_ptr`] 方法。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 找到插入槽位并写入哈希标签（unsafe：安全规则见上方文档注释）。
    unsafe fn prepare_insert_index(&mut self, hash: u64) -> (usize, Tag) {
        // 进入 unsafe 块：安全性契约见上方注释。
        unsafe {
            // SAFETY: 本函数的调用者保证控制字节已正确初始化。
            let index: usize = self.find_insert_index(hash);
            // SAFETY:
            // 1. `find_insert_index` 函数要么返回小于或等于表的
            //    `self.num_buckets() = self.bucket_mask + 1` 的 `index`，
            //    要么在找不到空桶或墓碑（deleted）槽位时永远不会返回。
            // 2. 本函数的调用者保证表已经完成分配。
            let old_ctrl = *self.ctrl(index);
            // 把新元素的哈希标签写入该槽位的控制字节。
            self.set_ctrl_hash(index, hash);
            // 返回槽位索引和被覆盖的旧控制字节。
            (index, old_ctrl)
        }
    }

    /// 在表中搜索适合插入新元素的空桶或已删除桶，返回新 [`Bucket`] 的 `index`。
    ///
    /// 此函数不会对表的 `data` 部分做任何更改，也不会更改表的 `items` 或 `growth_left` 字段。
    ///
    /// 表中必须至少有 1 个空桶或已删除桶，否则对于大于组宽度的表，此函数将永不返回（陷入无限循环）；
    /// 对于小于组宽度的表，则会返回超出表索引范围的索引。
    ///
    /// 如果表中至少有 1 个空桶或已删除桶，则此函数保证返回 `0..self.num_buckets()` 范围内的索引；
    /// 但无论如何，返回值都处于 `0..=self.num_buckets()` 范围内。
    ///
    /// # Safety
    ///
    /// [`RawTableInner`] 必须已正确初始化控制字节，否则调用此函数会导致 [`undefined behavior`]。
    ///
    /// 当表小于组宽度且表中没有至少一个空桶或已删除桶时，尝试向此函数返回的索引写入数据会立即导致
    /// [`undefined behavior`]。这是因为此时由于表范围之外的尾部 [`Tag::EMPTY`] 控制字节，
    /// 函数会返回 `self.bucket_mask + 1` 作为索引。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 在表中搜索适合插入新元素的空桶或已删除桶，返回其索引。
    unsafe fn find_insert_index(&self, hash: u64) -> usize {
        // 根据哈希值创建探测序列。
        let mut probe_seq = self.probe_seq(hash);
        // 开始探测循环。
        loop {
            // SAFETY:
            // * 此函数的调用者确保控制字节已正确初始化。
            //
            // * 由于使用了 `self.bucket_mask` 掩码，且桶数是 2 的幂（见 `self.probe_seq` 函数），
            //   `ProbeSeq.pos` 不会大于表的 `self.bucket_mask = self.num_buckets() - 1`。
            //
            // * 即使 `ProbeSeq.pos` 返回 `position == self.bucket_mask`，由于扩展的控制字节范围
            //   （为 `self.bucket_mask + 1 + Group::WIDTH`），调用 `Group::load` 也是安全的
            //  （实际上，这意味着对于已分配的表，永远不会读取最后一个控制字节）；
            //
            // * 此外，即使 `RawTableInner` 尚未分配，`ProbeSeq.pos` 也总是返回 "0"（零），
            //   此时 Group::load 读取的是未对齐的 `Group::static_empty()` 字节，这是安全的
            //   （见 RawTableInner::new）。
            // 从探测位置加载一组控制字节。
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            // 在当前组内寻找空桶或已删除桶的插入索引。
            let index = self.find_insert_index_in_group(&group, &probe_seq);
            // 若在组内找到了候选索引。
            if likely(index.is_some()) {
                // SAFETY:
                // * 此函数的调用者确保控制字节已正确初始化。
                //
                // * 我们使用的槽位/索引是由 `self.find_insert_index_in_group` 找到的。
                unsafe {
                    // 修正该索引并返回（处理小表误命中已占用桶的情况）。
                    return self.fix_insert_index(index.unwrap_unchecked());
                }
            }
            // 将探测序列推进到下一个组。
            probe_seq.move_next(self.bucket_mask);
        }
    }

    /// 在表中搜索元素，返回找到元素的 `index`。
    /// 此处使用动态分发以减少生成的代码量，但该开销会被 LLVM 优化消除。
    ///
    /// 此函数不会对表的 `data` 部分做任何更改，也不会更改表的 `items` 或 `growth_left` 字段。
    ///
    /// 表中必须至少有 1 个空桶，否则若 `eq: &mut dyn FnMut(usize) -> bool` 函数不返回 `true`，
    /// 此函数也将永不返回（陷入无限循环）。
    ///
    /// 此函数保证只会把 `FULL` 桶的索引提供给 `eq: &mut dyn FnMut(usize) -> bool` 函数，并以
    /// `Some(index)` 的形式返回找到元素的 `index`，因此索引总是处于 `0..self.num_buckets()` 范围内。
    ///
    /// # Safety
    ///
    /// [`RawTableInner`] 必须已正确初始化控制字节，否则调用此函数会导致 [`undefined behavior`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline(always)]
    // 在表中搜索元素，返回找到元素的索引。
    unsafe fn find_inner(&self, hash: u64, eq: &mut dyn FnMut(usize) -> bool) -> Option<usize> {
        // 取哈希值的高 7 位标签。
        let tag_hash = Tag::full(hash);
        // 根据哈希值创建探测序列。
        let mut probe_seq = self.probe_seq(hash);

        // 开始探测循环。
        loop {
            // SAFETY:
            // * 此函数的调用者确保控制字节已正确初始化。
            //
            // * 由于使用了 `self.bucket_mask` 掩码，`ProbeSeq.pos` 不会大于表的
            //   `self.bucket_mask = self.num_buckets() - 1`。
            //
            // * 即使 `ProbeSeq.pos` 返回 `position == self.bucket_mask`，由于扩展的控制字节范围
            //   （为 `self.bucket_mask + 1 + Group::WIDTH`），调用 `Group::load` 也是安全的
            //  （实际上，这意味着对于已分配的表，永远不会读取最后一个控制字节）；
            //
            // * 此外，即使 `RawTableInner` 尚未分配，`ProbeSeq.pos` 也总是返回 "0"（零），
            //   此时 Group::load 读取的是未对齐的 `Group::static_empty()` 字节，这是安全的
            //   （见 RawTableInner::new_in）。
            // 从探测位置加载一组控制字节。
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            // 遍历组内标签匹配哈希的元素位。
            for bit in group.match_tag(tag_hash) {
                // 这等价于 `(probe_seq.pos + bit) % self.num_buckets()`，因为桶数是 2 的幂，
                // 且 `self.bucket_mask = self.num_buckets() - 1`。
                // 计算该位对应的全局桶索引。
                let index = (probe_seq.pos + bit) & self.bucket_mask;

                // 若相等性判断成立，说明找到了目标元素。
                if likely(eq(index)) {
                    // 以 Some 形式返回找到的索引。
                    return Some(index);
                }
            }

            // 若当前组中存在空桶，说明探测已覆盖所有可能位置，元素不存在。
            if likely(group.match_empty().any_bit_set()) {
                // 返回 None 表示未找到。
                return None;
            }

            // 将探测序列推进到下一个组。
            probe_seq.move_next(self.bucket_mask);
        }
    }

    /// 为原地重新哈希数据（即不分配新内存）做准备。将所有 FULL 索引的 `control bytes` 转换为
    /// `Tag::DELETED`，并将所有 `Tag::DELETED` 控制字节转换为 `Tag::EMPTY`，即执行如下转换：
    ///
    /// - `Tag::EMPTY` 控制字节   -> `Tag::EMPTY`；
    /// - `Tag::DELETED` 控制字节 -> `Tag::EMPTY`；
    /// - `FULL` 控制字节    -> `Tag::DELETED`。
    ///
    /// 此函数不会对表的 `data` 部分做任何更改，也不会更改表的 `items` 或 `growth_left` 字段。
    ///
    /// # Safety
    ///
    /// 调用此函数时必须遵守以下安全规则：
    ///
    /// * [`RawTableInner`] 必须已分配；
    ///
    /// * 此函数的调用者在把 `Tag::DELETED` 字节重新插入其理想位置时，必须将其转换回 `FULL`
    ///   字节（由于墓碑的存在，第一次插入时无法做到这一点）。若调用者不这样做，调用此函数
    ///   可能导致内存泄漏。
    ///
    /// * [`RawTableInner`] 必须已正确初始化控制字节，否则调用此函数会导致 [`undefined behavior`]。
    ///
    /// 对尚未分配的表调用此函数会导致 [`undefined behavior`]。
    ///
    /// 另请参阅 [`Bucket::as_ptr`] 方法，了解如何正确地从 [`RawTable`] / [`RawTableInner`]
    /// 中移除或保存 `data element`。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 为原地重新哈希做准备：FULL 控制字节转换为 DELETED，DELETED 转换为 EMPTY。
    unsafe fn prepare_rehash_in_place(&mut self) {
        // 把所有 FULL 控制字节批量转换为 DELETED，把所有 DELETED 控制字节转换为 EMPTY。
        // 这实际上释放了所有包含 DELETED 条目的桶。
        //
        // SAFETY:
        // 1. 由于我们从 0 迭代到 `buckets - 1`，`i` 保证在边界内；
        // 2. 即使 `i == self.bucket_mask`，由于扩展的控制字节范围（`self.bucket_mask + 1 + Group::WIDTH`），
        //    调用 `Group::load_aligned` 也是安全的；
        // 3. 此函数的调用者保证 [`RawTableInner`] 已经分配；
        // 4. 由于我们从 0 开始、以 `Group::WIDTH` 为步长走到末尾，可以在此使用
        //    `Group::load_aligned` 和 `Group::store_aligned`（见 TableLayout::calculate_layout_for）。
        unsafe {
            // 以组宽度为步长遍历所有桶。
            for i in (0..self.num_buckets()).step_by(Group::WIDTH) {
                // 对齐加载当前组的控制字节。
                let group = Group::load_aligned(self.ctrl(i));
                // 把特殊标记转换为 EMPTY、FULL 转换为 DELETED。
                let group = group.convert_special_to_empty_and_full_to_deleted();
                // 把转换后的组写回控制字节。
                group.store_aligned(self.ctrl(i));
            }
        }

        // 修正尾部的控制字节。对于小于组宽度的表的处理方式，见 set_ctrl 中的注释。
        // 若桶数小于组宽度（小表的特殊情况）。
        if unlikely(self.num_buckets() < Group::WIDTH) {
            // SAFETY: 我们有 `self.bucket_mask + 1 + Group::WIDTH` 个控制字节，
            // 因此以 `Group::WIDTH` 为偏移复制 `self.num_buckets() == self.bucket_mask + 1` 个字节是安全的。
            unsafe {
                // 从第 0 个控制字节复制全部桶数个字节到组宽度处。
                self.ctrl(0)
                    .copy_to(self.ctrl(Group::WIDTH), self.num_buckets());
            }
        } else {
            // SAFETY: 我们有 `self.bucket_mask + 1 + Group::WIDTH` 个控制字节，
            // 因此以 `self.num_buckets() == self.bucket_mask + 1` 为偏移复制 `Group::WIDTH` 个字节是安全的。
            unsafe {
                // 把前组宽度的控制字节复制到表末尾的重复区域。
                self.ctrl(0)
                    .copy_to(self.ctrl(self.num_buckets()), Group::WIDTH);
            }
        }
    }

    /// 返回遍历表中每个元素的迭代器。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`undefined behavior`]：
    ///
    /// * 调用者必须确保 `RawTableInner` 的寿命长于 `RawIter`。由于我们无法把 `RawIter`
    ///   结构体上的 `next` 方法标记为 unsafe，只能把 `iter` 方法标记为 unsafe。
    ///
    /// * [`RawTableInner`] 必须已正确初始化控制字节。
    ///
    /// 类型 `T` 必须是表中存储元素的真实类型，否则使用返回的 [`RawIter`] 会导致 [`undefined behavior`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回遍历表中每个元素的迭代器。
    unsafe fn iter<T>(&self) -> RawIter<T> {
        // SAFETY:
        // 1. 由于此函数的调用者确保控制字节已正确初始化，且 `self.data_end()` 指向
        //    控制字节数组的起始处，因此：`ctrl` 可用于读取、按 `Group::WIDTH` 正确对齐，
        //    并指向已正确初始化的控制字节。
        // 2. `data` 桶在表中的索引等于 `ctrl` 的索引（即等于零）。
        // 3. 我们把表的桶数的精确值传给该函数。
        //
        //                         `ctrl` 指向这里（第一个控制字节
        //                         `CT0` 的起始处）
        //                          ∨
        // [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, CTa_0, CTa_1, ..., CTa_m
        //                           \________  ________/
        //                                    \/
        //       `n = buckets - 1`，即 `RawTableInner::num_buckets() - 1`
        //
        // 其中：T0...T_n  - 我们存储的数据；
        //       CT0...CT_n - `data` 的控制字节或元数据。
        //       CTa_0...CTa_m - 额外的控制字节，其中 `m = Group::WIDTH - 1`（这样即使
        //                       `h1(hash) & self.bucket_mask` 的结果等于 `self.bucket_mask`，
        //                       从堆上加载 `Group` 字节的搜索也能正常工作）。另请参阅
        //                       `RawTableInner::set_ctrl` 函数。
        //
        // P.S. `h1(hash) & self.bucket_mask` 等价于 `hash as usize % self.num_buckets()`，因为桶数是 2 的幂，
        // 且 `self.bucket_mask = self.num_buckets() - 1`。
        unsafe {
            // 以索引 0 创建指向数据部分起始处的桶。
            let data = Bucket::from_base_index(self.data_end(), 0);
            // 构造原始迭代器。
            RawIter {
                // SAFETY: 见上文解释
                // 用控制字节指针、数据桶与桶数构造范围迭代器。
                iter: RawIterRange::new(self.ctrl.as_ptr(), data, self.num_buckets()),
                // 记录表中元素总数。
                items: self.items,
            }
        }
    }

    /// 执行表中存储的值的析构函数（如果有的话）。
    ///
    /// # Note
    ///
    /// 此函数不会擦除表的控制字节，也不会更改表的 `items` 或 `growth_left` 字段。
    /// 如有必要，此函数的调用者必须手动设置这些表字段，例如使用 [`clear_no_drop`] 函数。
    ///
    /// 调用此函数时要小心，因为元素的 drop 函数可能 panic，这会使表处于不一致的状态。
    ///
    /// # Safety
    ///
    /// 类型 `T` 必须是表中存储元素的真实类型，否则调用此函数可能导致 [`undefined behavior`]。
    ///
    /// 如果 `T` 是需要 drop 的类型且**表非空**，多次调用此函数会导致 [`undefined behavior`]。
    ///
    /// 如果 `T` 不是 [`Copy`]，在调用此函数之后尝试使用表中存储的值可能导致 [`undefined behavior`]。
    ///
    /// 对尚未分配的表、控制字节未初始化的表，以及在 `self.items == 0` 时没有实际数据但控制字节
    /// 为 `Full` 的表调用此函数是安全的。
    ///
    /// 另请参阅 [`Bucket::drop`] / [`Bucket::as_ptr`] 方法，了解如何正确地从 [`RawTable`] /
    /// [`RawTableInner`] 中移除或保存 `element`。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    // 执行表中存储的所有值的析构函数（如果有）。
    unsafe fn drop_elements<T>(&mut self) {
        // 检查 `self.items != 0`。防止在控制字节未初始化的表上创建迭代器。
        if T::NEEDS_DROP && self.items != 0 {
            // SAFETY: 我们确信 RawTableInner 的寿命长于返回的 `RawIter` 迭代器，
            // 且此函数的调用者必须遵守 `drop_elements` 方法的安全契约。
            unsafe {
                // 遍历表中的每个元素。
                for item in self.iter::<T>() {
                    // SAFETY: 调用者必须遵守 `drop_elements` 方法的安全契约。
                    // 就地执行该元素的析构函数。
                    item.drop();
                }
            }
        }
    }

    /// 执行表中存储的值的析构函数（如果有的话），然后释放表的内存。
    ///
    /// # Note
    ///
    /// 调用此函数会自动使所有桶（[`Bucket`]）实例失效（悬垂），并使表的 `ctrl` 字段失效（悬垂）。
    ///
    /// 此函数不会更改表的 `bucket_mask`、`items` 或 `growth_left` 字段。如有必要，调用者必须手动设置这些表字段。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`undefined behavior`]：
    ///
    /// * 多次调用此函数；
    ///
    /// * 类型 `T` 必须是表中存储元素的真实类型。
    ///
    /// * `alloc` 必须与分配此表所用的 [`Allocator`] 是同一个。
    ///
    /// * `table_layout` 必须与分配此表所用的 [`TableLayout`] 是同一个。
    ///
    /// 此函数的调用者应注意元素的 drop 函数可能 panic，因为这：
    ///
    ///    * 可能使表处于不一致的状态；
    ///
    ///    * 内存永远不会被释放，因此可能发生内存泄漏。
    ///
    /// 调用此函数之后尝试使用表的 `ctrl` 字段（解引用）会导致 [`undefined behavior`]。
    ///
    /// 对尚未分配的表、控制字节未初始化的表，以及在 `self.items == 0` 时没有实际数据但控制字节
    /// 为 `Full` 的表调用此函数是安全的。
    ///
    /// 另请参阅 [`RawTableInner::drop_elements`] 或 [`RawTableInner::free_buckets`] 以了解更多信息。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    // 执行表中所有值的析构函数，然后释放表内存。
    unsafe fn drop_inner_table<T, A: Allocator>(&mut self, alloc: &A, table_layout: TableLayout) {
        // 若表不是空表单例。
        if !self.is_empty_singleton() {
            // SAFETY: 调用者必须遵守 `drop_inner_table` 方法的安全契约。
            unsafe {
                // 先析构表中所有元素。
                self.drop_elements::<T>();
            }
            // SAFETY:
            // 1. 我们已检查表已分配。
            // 2. 调用者必须遵守 `drop_inner_table` 方法的安全契约。
            unsafe {
                // 再释放表的内存。
                self.free_buckets(alloc, table_layout);
            }
        }
    }

    /// 返回表中元素的指针（等价于 `Bucket::from_base_index(self.data_end::<T>(), index)` 的便捷方法）。
    ///
    /// 调用者必须确保 `RawTableInner` 的寿命长于返回的 [`Bucket<T>`]，否则使用它可能导致 [`undefined behavior`]。
    ///
    /// # Safety
    ///
    /// 若 `size_of::<T>() != 0`，则安全规则直接源自 [`Bucket::from_base_index`] 函数的安全规则。因此，调用
    /// 此函数时必须遵守以下安全规则：
    ///
    /// * 表必须已分配；
    ///
    /// * `index` 不得大于 [`RawTableInner::num_buckets`] 函数返回的数量，即 `(index + 1) <= self.num_buckets()`。
    ///
    /// * 类型 `T` 必须是表中存储元素的真实类型，否则使用返回的 [`Bucket`] 可能导致 [`undefined behavior`]。
    ///
    /// 对尚未分配的表以索引零（`index == 0`）调用此函数是安全的，但使用返回的 [`Bucket`] 会导致 [`undefined behavior`]。
    ///
    /// 若 `size_of::<T>() == 0`，则唯一的要求是 `index` 不得大于 [`RawTable::num_buckets`] 函数返回的数量，
    /// 即 `(index + 1) <= self.num_buckets()`。
    ///
    /// ```none
    /// 若 size_of::<T>() != 0，则返回指向表中 `data part` 的 `element` 的指针
    /// （我们从 "0" 开始计数，因此在表达式 T[n] 中，"n" 索引实际上比我们的 `RawTableInner` 的
    /// "buckets" 数量小 1，即 "n = RawTableInner::num_buckets() - 1"）：
    ///
    ///           `table.bucket(3).as_ptr()` 返回的指针指向这里，即 `RawTableInner` 的 `data`
    ///           部分，也就是 T3 的起始处（见 [`Bucket::as_ptr`]）
    ///                  |
    ///                  |               `base = table.data_end::<T>()` 指向这里
    ///                  |               （CT0 的起始处或 T0 的结尾处）
    ///                  v                 v
    /// [Pad], T_n, ..., |T3|, T2, T1, T0, |CT0, CT1, CT2, CT3, ..., CT_n, CTa_0, CTa_1, ..., CTa_m
    ///                     ^                                              \__________  __________/
    ///        `table.bucket(3)` 返回的指针指向 `RawTableInner` 的 `data` 部分                    \/
    ///         （T3 的结尾处）                                              额外的控制字节
    ///                                                                      `m = Group::WIDTH - 1`
    ///
    /// 其中：T0...T_n  - 我们存储的数据；
    ///       CT0...CT_n - `data` 的控制字节或元数据；
    ///       CTa_0...CTa_m - 额外的控制字节（这样即使 `h1(hash) & self.bucket_mask` 的结果
    ///                       等于 `self.bucket_mask`，从堆上加载 `Group` 字节的搜索也能正常工作）。
    ///                       另请参阅 `RawTableInner::set_ctrl` 函数。
    ///
    /// P.S. `h1(hash) & self.bucket_mask` 等价于 `hash as usize % self.num_buckets()`，因为桶数是 2 的幂，
    /// 且 `self.bucket_mask = self.num_buckets() - 1`。
    /// ```
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回表中指定索引处的桶（指针便捷方法）。
    unsafe fn bucket<T>(&self, index: usize) -> Bucket<T> {
        // 调试断言：bucket_mask 不为 0（表已分配）。
        debug_assert_ne!(self.bucket_mask, 0);
        // 调试断言：索引必须小于桶数。
        debug_assert!(index < self.num_buckets());
        // 由基准索引创建对应的桶。
        unsafe { Bucket::from_base_index(self.data_end(), index) }
    }

    /// 返回指向表中 `data` 元素起始处的裸 `*mut u8` 指针
    /// （等价于 `self.data_end::<u8>().as_ptr().sub((index + 1) * size_of)` 的便捷方法）。
    ///
    /// 调用者必须确保 `RawTableInner` 的寿命长于返回的 `*mut u8`，否则使用它可能导致 [`undefined behavior`]。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`undefined behavior`]：
    ///
    /// * 表必须已分配；
    ///
    /// * `index` 不得大于 [`RawTableInner::num_buckets`] 函数返回的数量，即 `(index + 1) <= self.num_buckets()`；
    ///
    /// * `size_of` 必须等于表中存储元素的大小；
    ///
    /// ```none
    /// 若 size_of::<T>() != 0，则返回指向表中 `data part` 的 `element` 的指针
    /// （我们从 "0" 开始计数，因此在表达式 T[n] 中，"n" 索引实际上比我们的 `RawTableInner` 的
    /// "buckets" 数量小 1，即 "n = RawTableInner::num_buckets() - 1"）：
    ///
    ///           `table.bucket_ptr(3, size_of::<T>())` 返回的指针指向这里，即
    ///           `RawTableInner` 的 `data` 部分，也就是 T3 的起始处
    ///                  |
    ///                  |               `base = table.data_end::<u8>()` 指向这里
    ///                  |               （CT0 的起始处或 T0 的结尾处）
    ///                  v                 v
    /// [Pad], T_n, ..., |T3|, T2, T1, T0, |CT0, CT1, CT2, CT3, ..., CT_n, CTa_0, CTa_1, ..., CTa_m
    ///                                                                    \__________  __________/
    ///                                                                               \/
    ///                                                                    额外的控制字节
    ///                                                                     `m = Group::WIDTH - 1`
    ///
    /// 其中：T0...T_n  - 我们存储的数据；
    ///       CT0...CT_n - `data` 的控制字节或元数据；
    ///       CTa_0...CTa_m - 额外的控制字节（这样即使 `h1(hash) & self.bucket_mask` 的结果
    ///                       等于 `self.bucket_mask`，从堆上加载 `Group` 字节的搜索也能正常工作）。
    ///                       另请参阅 `RawTableInner::set_ctrl` 函数。
    ///
    /// P.S. `h1(hash) & self.bucket_mask` 等价于 `hash as usize % self.num_buckets()`，因为桶数是 2 的幂，
    /// 且 `self.bucket_mask = self.num_buckets() - 1`。
    /// ```
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回表中指定索引处的数据元素的裸字节指针。
    unsafe fn bucket_ptr(&self, index: usize, size_of: usize) -> *mut u8 {
        // 调试断言：bucket_mask 不为 0（表已分配）。
        debug_assert_ne!(self.bucket_mask, 0);
        // 调试断言：索引必须小于桶数。
        debug_assert!(index < self.num_buckets());
        unsafe {
            // 取数据末尾指针作为基准。
            let base: *mut u8 = self.data_end().as_ptr();
            // 从基准向前偏移 (index + 1) * size_of 字节，得到元素起始地址。
            base.sub((index + 1) * size_of)
        }
    }

    /// 返回从分配起点看，表中最后一个 `data` 元素之后位置的指针
    /// （等价于 `self.ctrl.cast()` 的便捷方法）。
    ///
    /// 此函数实际上返回指向索引 "0"（零）处的 `data element` 结尾的指针。
    ///
    /// 调用者必须确保 `RawTableInner` 的寿命长于返回的 [`NonNull<T>`]，否则使用它可能导致 [`undefined behavior`]。
    ///
    /// # Note
    ///
    /// 类型 `T` 必须是表中存储元素的真实类型，否则使用返回的 [`NonNull<T>`] 可能导致 [`undefined behavior`]。
    ///
    /// ```none
    ///                        `table.data_end::<T>()` 返回的指针指向这里
    ///                        （`T0` 的结尾处）
    ///                          ∨
    /// [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, CTa_0, CTa_1, ..., CTa_m
    ///                           \________  ________/
    ///                                    \/
    ///       `n = buckets - 1`，即 `RawTableInner::num_buckets() - 1`
    ///
    /// 其中：T0...T_n  - 我们存储的数据；
    ///       CT0...CT_n - `data` 的控制字节或元数据。
    ///       CTa_0...CTa_m - 额外的控制字节，其中 `m = Group::WIDTH - 1`（这样即使
    ///                       `h1(hash) & self.bucket_mask` 的结果等于 `self.bucket_mask`，
    ///                       从堆上加载 `Group` 字节的搜索也能正常工作）。另请参阅
    ///                       `RawTableInner::set_ctrl` 函数。
    ///
    /// P.S. `h1(hash) & self.bucket_mask` 等价于 `hash as usize % self.num_buckets()`，因为桶数是 2 的幂，
    /// 且 `self.bucket_mask = self.num_buckets() - 1`。
    /// ```
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回数据部分的末尾指针（即控制字节数组的起始处）。
    fn data_end<T>(&self) -> NonNull<T> {
        // 把控制字节指针强制转换为指向 T 的指针。
        self.ctrl.cast()
    }

    /// 返回表上探测序列的类迭代器对象。
    ///
    /// 此迭代器永不终止，但保证恰好访问每个桶组一次。使用 `probe_seq` 的循环必须在
    /// 到达包含空桶的组时终止。
    #[inline]
    // 根据哈希值创建探测序列。
    fn probe_seq(&self, hash: u64) -> ProbeSeq {
        // 构造探测序列。
        ProbeSeq {
            // 这等价于 `hash as usize % self.num_buckets()`，因为桶数是 2 的幂，
            // 且 `self.bucket_mask = self.num_buckets() - 1`。
            // 初始探测位置：h1(hash) 经掩码取模。
            pos: h1(hash) & self.bucket_mask,
            // 初始步长为 0。
            stride: 0,
        }
    }
    #[inline]
    // 记录在指定索引处插入元素：更新增长空间、控制字节与元素计数。
    unsafe fn record_item_insert_at(&mut self, index: usize, old_ctrl: Tag, new_ctrl: Tag) {
        // 若旧控制字节为特殊空值（EMPTY/DELETED），则减少剩余可增长空间。
        self.growth_left -= usize::from(old_ctrl.special_is_empty());
        unsafe {
            // 把新控制字节写入指定索引（并同步尾部的重复控制字节）。
            self.set_ctrl(index, new_ctrl);
        }
        // 元素计数加 1。
        self.items += 1;
    }

    #[inline]
    // 判断旧索引 i 与新索引 new_i 是否位于同一个未对齐的组内。
    fn is_in_same_group(&self, i: usize, new_i: usize, hash: u64) -> bool {
        // 取该哈希探测序列的初始位置。
        let probe_seq_pos = self.probe_seq(hash).pos;
        // 计算某位置相对于探测起点所属的组号。
        let probe_index =
            |pos: usize| (pos.wrapping_sub(probe_seq_pos) & self.bucket_mask) / Group::WIDTH;
        // 两位置的组号相同则返回 true。
        probe_index(i) == probe_index(new_i)
    }

    /// 把控制字节设置为哈希，并可能在数组末尾同步写入复制的控制字节。
    ///
    /// 此函数不会对表的 `data` 部分做任何更改，也不会更改表的 `items` 或 `growth_left` 字段。
    ///
    /// # Safety
    ///
    /// 安全规则直接源自 [`RawTableInner::set_ctrl`] 方法的安全规则。因此，为遵守该方法的安全契约，
    /// 调用此函数时必须遵守以下规则：
    ///
    /// * [`RawTableInner`] 必须已分配；
    ///
    /// * `index` 不得大于 `RawTableInner.bucket_mask`，即 `index <= RawTableInner.bucket_mask`，
    ///   换句话说，`(index + 1)` 不得大于 [`RawTableInner::num_buckets`] 函数返回的数量。
    ///
    /// 对尚未分配的表调用此函数会导致 [`undefined behavior`]。
    ///
    /// 另请参阅 [`Bucket::as_ptr`] 方法，了解如何正确地从 [`RawTable`] / [`RawTableInner`]
    /// 中移除或保存 `data element`。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 把指定索引处的控制字节设置为该哈希对应的标签。
    unsafe fn set_ctrl_hash(&mut self, index: usize, hash: u64) {
        unsafe {
            // SAFETY: 调用者必须遵守 [`RawTableInner::set_ctrl_hash`] 的安全规则
            // 由完整哈希派生标签并写入控制字节。
            self.set_ctrl(index, Tag::full(hash));
        }
    }

    /// 把给定索引处控制字节中的哈希替换为提供的哈希，并可能在控制字节数组末尾同步复制新的
    /// 控制字节，返回旧的控制字节。
    ///
    /// 此函数不会对表的 `data` 部分做任何更改，也不会更改表的 `items` 或 `growth_left` 字段。
    ///
    /// # Safety
    ///
    /// 安全规则直接源自 [`RawTableInner::set_ctrl_hash`] 和 [`RawTableInner::ctrl`] 方法的
    /// 安全规则。因此，为遵守这两个方法的安全契约，调用此函数时必须遵守以下规则：
    ///
    /// * [`RawTableInner`] 必须已分配；
    ///
    /// * `index` 不得大于 `RawTableInner.bucket_mask`，即 `index <= RawTableInner.bucket_mask`，
    ///   换句话说，`(index + 1)` 不得大于 [`RawTableInner::num_buckets`] 函数返回的数量。
    ///
    /// 对尚未分配的表调用此函数会导致 [`undefined behavior`]。
    ///
    /// 另请参阅 [`Bucket::as_ptr`] 方法，了解如何正确地从 [`RawTable`] / [`RawTableInner`]
    /// 中移除或保存 `data element`。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 替换指定索引处的控制字节哈希，返回旧的控制字节。
    unsafe fn replace_ctrl_hash(&mut self, index: usize, hash: u64) -> Tag {
        unsafe {
            // SAFETY: 调用者必须遵守 [`RawTableInner::replace_ctrl_hash`] 的安全规则
            // 读取旧的控制字节。
            let prev_ctrl = *self.ctrl(index);
            // 写入新的控制字节哈希。
            self.set_ctrl_hash(index, hash);
            // 返回旧的控制字节。
            prev_ctrl
        }
    }

    /// 设置一个控制字节，并可能在数组末尾同步写入复制的控制字节。
    ///
    /// 此函数不会对表的 `data` 部分做任何更改，也不会更改表的 `items` 或 `growth_left` 字段。
    ///
    /// # Safety
    ///
    /// 调用此函数时必须遵守以下安全规则：
    ///
    /// * [`RawTableInner`] 必须已分配；
    ///
    /// * `index` 不得大于 `RawTableInner.bucket_mask`，即 `index <= RawTableInner.bucket_mask`，
    ///   换句话说，`(index + 1)` 不得大于 [`RawTableInner::num_buckets`] 函数返回的数量。
    ///
    /// 对尚未分配的表调用此函数会导致 [`undefined behavior`]。
    ///
    /// 另请参阅 [`Bucket::as_ptr`] 方法，了解如何正确地从 [`RawTable`] / [`RawTableInner`]
    /// 中移除或保存 `data element`。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 设置指定索引处的控制字节，并同步尾部的重复控制字节。
    unsafe fn set_ctrl(&mut self, index: usize, ctrl: Tag) {
        // 不使用分支地把前 Group::WIDTH 个控制字节复制到数组末尾。若表小于组宽度
        // (self.num_buckets() < Group::WIDTH)，则 `index2 = Group::WIDTH + index`；
        // 否则 `index2` 为：
        //
        // - 若 index >= Group::WIDTH，则 index == index2。
        // - 否则 index2 == self.bucket_mask + 1 + index。
        //
        // 最后一个复制的控制字节实际上永远不会被读取，因为我们为未对齐加载对初始索引做了掩码，
        // 但我们仍然会写入它，因为这样 set_ctrl 的实现更简单。
        //
        // 若桶数少于 Group::WIDTH，此代码会把桶复制到尾部组的末尾。例如，当有 2 个桶、
        // 组大小为 4 时，控制字节看起来像这样：
        //
        //     实际    |             复制的
        // ---------------------------------------------
        // | [A] | [B] | [Tag::EMPTY] | [EMPTY] | [A] | [B] |
        // ---------------------------------------------

        // 这等价于 `(index.wrapping_sub(Group::WIDTH)) % self.num_buckets() + Group::WIDTH`，
        // 因为桶数是 2 的幂，且 `self.bucket_mask = self.num_buckets() - 1`。
        // 计算需要同步写入的复制控制字节的位置。
        let index2 = ((index.wrapping_sub(Group::WIDTH)) & self.bucket_mask) + Group::WIDTH;

        // SAFETY: 调用者必须遵守 [`RawTableInner::set_ctrl`] 的安全规则
        unsafe {
            // 写入指定索引处的控制字节。
            *self.ctrl(index) = ctrl;
            // 同步写入复制位置的控制字节。
            *self.ctrl(index2) = ctrl;
        }
    }

    /// 返回指向控制字节的指针。
    ///
    /// # Safety
    ///
    /// 对于已分配的 [`RawTableInner`]，若 `index` 大于 `self.bucket_mask + 1 + Group::WIDTH`，
    /// 结果为 [`Undefined Behavior`]。此时，以 `index == self.bucket_mask + 1 + Group::WIDTH`
    /// 调用此函数会返回指向已分配表末尾的指针，该指针本身没有用处。
    ///
    /// 对尚未分配的表以 `index >= self.bucket_mask + 1 + Group::WIDTH` 调用此函数会导致
    /// [`Undefined Behavior`]。
    ///
    /// 因此，为同时满足这两个要求，你应当始终遵守规则
    /// `index < self.bucket_mask + 1 + Group::WIDTH`
    ///
    /// 对尚未分配的 [`RawTableInner`] 调用此函数，若仅用于只读目的是安全的。
    ///
    /// 另请参阅 [`Bucket::as_ptr()`] 方法，了解如何正确地从 [`RawTable`] / [`RawTableInner`]
    /// 中移除或保存 `data element`。
    ///
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回指向指定索引处控制字节的指针。
    unsafe fn ctrl(&self, index: usize) -> *mut Tag {
        // 调试断言：索引必须小于控制字节总数。
        debug_assert!(index < self.num_ctrl_bytes());
        // SAFETY: 调用者必须遵守 [`RawTableInner::ctrl`] 的安全规则
        // 从 ctrl 指针偏移 index 个字节并转换为 Tag 指针。
        unsafe { self.ctrl.as_ptr().add(index).cast() }
    }

    /// 以可能未初始化的标签形式，获取全部控制字节的切片。
    // 获取所有控制字节构成的切片。
    fn ctrl_slice(&mut self) -> &mut [mem::MaybeUninit<Tag>] {
        // SAFETY: 我们拥有正确数量的控制字节。
        unsafe { slice::from_raw_parts_mut(self.ctrl.as_ptr().cast(), self.num_ctrl_bytes()) }
    }

    #[inline]
    // 返回表中的桶数。
    fn num_buckets(&self) -> usize {
        // 桶数等于掩码加 1。
        self.bucket_mask + 1
    }

    /// 检查 `index` 处的桶是否已被占用（满）。
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `index` 小于桶数。
    #[inline]
    // 检查指定索引处的桶是否为满。
    unsafe fn is_bucket_full(&self, index: usize) -> bool {
        // 调试断言：索引必须小于桶数。
        debug_assert!(index < self.num_buckets());
        // 读取控制字节并判断其是否为满。
        unsafe { (*self.ctrl(index)).is_full() }
    }

    #[inline]
    // 返回控制字节的总数。
    fn num_ctrl_bytes(&self) -> usize {
        // 桶数（bucket_mask + 1）加上组宽度的额外控制字节。
        self.bucket_mask + 1 + Group::WIDTH
    }

    #[inline]
    // 判断该表是否为未分配的空表单例。
    fn is_empty_singleton(&self) -> bool {
        // 未分配表的 bucket_mask 为 0。
        self.bucket_mask == 0
    }

    /// 尝试分配一个新哈希表，其容量至少足以在不重新分配内存的情况下插入给定数量的元素，
    /// 并把新表放入 `ScopeGuard` 中返回，以防范哈希函数 panic。
    ///
    /// # Note
    ///
    /// 建议（但非强制）：
    ///
    /// * 新表的 `capacity` 大于或等于 `self.items`。
    ///
    /// * `alloc` 与分配此表所用的 [`Allocator`] 是同一个。
    ///
    /// * `table_layout` 与分配此表所用的 [`TableLayout`] 是同一个。
    ///
    /// 若 `table_layout` 与分配此表所用的 `TableLayout` 不一致，则对 `self` 与此函数返回的新表
    /// 使用 `mem::swap` 会导致 [`undefined behavior`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 准备调整表大小：分配并初始化新表，用 ScopeGuard 包裹以防哈希函数 panic。
    fn prepare_resize<'a, A>(
        // 取自身共享引用。
        &self,
        // 分配器引用（携带生命周期 'a）。
        alloc: &'a A,
        // 表的布局信息。
        table_layout: TableLayout,
        // 新表的容量。
        capacity: usize,
        // 可失败性标记。
        fallibility: Fallibility,
    ) -> Result<crate::scopeguard::ScopeGuard<Self, impl FnMut(&mut Self) + 'a>, TryReserveError>
    // 约束：A 必须实现 Allocator。
    where
        // Allocator 约束。
        A: Allocator,
    {
        // 调试断言：现有元素数不超过新容量。
        debug_assert!(self.items <= capacity);

        // 分配并初始化新表。
        let new_table =
            RawTableInner::fallible_with_capacity(alloc, table_layout, capacity, fallibility)?;

        // 哈希函数可能 panic，此时我们直接释放新表，而不 drop 任何可能已复制进其中的元素。
        //
        // 成功时也会用这个守卫释放旧表，见本函数底部的注释。
        // 用作用域守卫包裹新表：若哈希函数 panic 或失败路径触发，则释放新表内存。
        Ok(guard(new_table, move |self_| {
            // 若新表已分配（非空表单例）。
            if !self_.is_empty_singleton() {
                // SAFETY:
                // 1. 我们已检查该表已分配。
                // 2. 我们确信 `alloc` 与 `table_layout` 与分配此表所用的
                //    [`Allocator`] 和 [`TableLayout`] 一致。
                unsafe { self_.free_buckets(alloc, table_layout) };
            }
        }))
    }

    /// 预留空间或重新哈希，以便为额外 `additional` 个元素腾出空间。
    ///
    /// 此处使用动态分发以减少生成的代码量，但内联后该开销会被 LLVM 优化消除。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`undefined behavior`]：
    ///
    /// * `alloc` 必须与分配此表所用的 [`Allocator`] 是同一个。
    ///
    /// * `layout` 必须与分配此表所用的 [`TableLayout`] 是同一个。
    ///
    /// * `drop` 函数（`fn(*mut u8)`）必须是表中存储元素的真实 drop 函数。
    ///
    /// * [`RawTableInner`] 必须已正确初始化控制字节。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[expect(clippy::inline_always)]
    #[inline(always)]
    // 预留或重新哈希的内部实现：优先原地重哈希，否则扩容。
    unsafe fn reserve_rehash_inner<A>(
        // 取自身可变引用。
        &mut self,
        // 分配器引用。
        alloc: &A,
        // 需要额外容纳的元素数量。
        additional: usize,
        // 哈希函数（动态分发）。
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        // 可失败性标记。
        fallibility: Fallibility,
        // 表的布局信息。
        layout: TableLayout,
        // 可选的元素 drop 函数。
        drop: Option<unsafe fn(*mut u8)>,
    ) -> Result<(), TryReserveError>
    // 约束：A 必须实现 Allocator。
    where
        // Allocator 约束。
        A: Allocator,
    {
        // 避免 `Option::ok_or_else`，因为它会使 LLVM IR 膨胀。
        // 计算插入后的元素总数，若溢出则返回容量溢出错误。
        let Some(new_items) = self.items.checked_add(additional) else {
            // 溢出：返回容量溢出错误。
            return Err(fallibility.capacity_overflow());
        };
        // 计算当前表的满载容量。
        let full_capacity = bucket_mask_to_capacity(self.bucket_mask);
        // 若新元素数不超过满载容量的一半，说明存在大量被 DELETED 条目占用的空闲空间。
        if new_items <= full_capacity / 2 {
            // 若有大量因 DELETED 条目而被锁定的空闲容量，则无需重新分配，直接原地重哈希。

            // SAFETY:
            // 1. 我们确信 `[`RawTableInner`]` 已经分配
            //    （因为 new_items <= full_capacity / 2）；
            // 2. 调用者保证 `drop` 函数是表中存储元素的真实 drop 函数。
            // 3. 调用者保证 `layout` 与分配此表所用的 [`TableLayout`] 一致。
            // 4. 调用者保证 `RawTableInner` 的控制字节已初始化。
            unsafe {
                // 在原地重新哈希，清理墓碑。
                self.rehash_in_place(hasher, layout.size, drop);
            }
            // 原地重哈希成功。
            Ok(())
        } else {
            // 否则，保守地把表扩容到至少下一个尺寸，以避免删除操作频繁触发重哈希。
            //
            // SAFETY:
            // 1. 我们确信 `capacity >= self.items`。
            // 2. 调用者保证 `alloc` 和 `layout` 与分配此表所用的 [`Allocator`] 和
            //    [`TableLayout`] 一致。
            // 3. 调用者保证 `RawTableInner` 的控制字节已初始化。
            unsafe {
                // 调整表大小以容纳更多元素。
                self.resize_inner(
                    // 传入分配器。
                    alloc,
                    // 新容量取 new_items 与满载容量加 1 的较大者。
                    usize::max(new_items, full_capacity + 1),
                    // 传入哈希函数。
                    hasher,
                    // 传入可失败性标记。
                    fallibility,
                    // 传入表布局。
                    layout,
                )
            }
        }
    }

    /// 返回遍历表中所有满桶索引的迭代器。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，行为未定义：
    ///
    /// * 调用者必须确保 `RawTableInner` 的寿命长于 `FullBucketsIndices`。由于我们无法把
    ///   `FullBucketsIndices` 结构体上的 `next` 方法标记为 unsafe，只能把
    ///   `full_buckets_indices` 方法标记为 unsafe。
    ///
    /// * [`RawTableInner`] 必须已正确初始化控制字节。
    #[inline(always)]
    // 返回遍历表中所有满桶索引的迭代器。
    unsafe fn full_buckets_indices(&self) -> FullBucketsIndices {
        // SAFETY:
        // 1. 由于此函数的调用者确保控制字节已正确初始化，且 `self.ctrl(0)` 指向
        //    控制字节数组的起始处，因此：`ctrl` 可用于读取、按 `Group::WIDTH` 正确对齐，
        //    并指向已正确初始化的控制字节。
        // 2. `items` 的值等于添加到表中的数据（值）的数量。
        //
        //                         `ctrl` 指向这里（第一个控制字节
        //                         `CT0` 的起始处）
        //                          ∨
        // [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, Group::WIDTH
        //                           \________  ________/
        //                                    \/
        //       `n = buckets - 1`，即 `RawTableInner::num_buckets() - 1`
        //
        // 其中：T0...T_n  - 我们存储的数据；
        //       CT0...CT_n - `data` 的控制字节或元数据。
        unsafe {
            // 取第 0 个控制字节的指针并构造 NonNull。
            let ctrl = NonNull::new_unchecked(self.ctrl(0).cast::<u8>());

            // 构造满桶索引迭代器。
            FullBucketsIndices {
                // 加载第一组
                // SAFETY: 见上文解释。
                current_group: Group::load_aligned(ctrl.as_ptr().cast())
                    // 匹配出满桶的位掩码。
                    .match_full()
                    // 转换为位掩码迭代器。
                    .into_iter(),
                // 第一组的起始索引为 0。
                group_first_index: 0,
                // 保存控制字节指针。
                ctrl,
                // 记录表中元素总数。
                items: self.items,
            }
        }
    }

    /// 分配一个不同大小的新表，并把当前表的内容移动过去。
    ///
    /// 此处使用动态分发以减少生成的代码量，但内联后该开销会被 LLVM 优化消除。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`undefined behavior`]：
    ///
    /// * `alloc` 必须与分配此表所用的 [`Allocator`] 是同一个；
    ///
    /// * `layout` 必须与分配此表所用的 [`TableLayout`] 是同一个；
    ///
    /// * [`RawTableInner`] 必须已正确初始化控制字节。
    ///
    /// 此函数的调用者必须确保 `capacity >= self.items`，否则：
    ///
    /// * 若 `self.items != 0`，以 `capacity == 0` 调用此函数会导致 [`undefined behavior`]。
    ///
    /// * 若 `capacity_to_buckets(capacity) < Group::WIDTH` 且
    ///   `self.items > capacity_to_buckets(capacity)`，调用此函数会导致 [`undefined behavior`]。
    ///
    /// * 若 `capacity_to_buckets(capacity) >= Group::WIDTH` 且
    ///   `self.items > capacity_to_buckets(capacity)`，调用此函数将永不返回（陷入无限循环）。
    ///
    /// 注意：建议（但非强制）新表的 `capacity` 大于或等于 `self.items`。若 `capacity <= self.items`，
    /// 此函数可能永不返回。更多信息见 [`RawTableInner::find_insert_index`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[expect(clippy::inline_always)]
    #[inline(always)]
    // 调整表大小的内部实现：分配新表并把旧表元素逐个搬移过去。
    unsafe fn resize_inner<A>(
        // 取自身可变引用。
        &mut self,
        // 分配器引用。
        alloc: &A,
        // 新表的容量。
        capacity: usize,
        // 哈希函数（动态分发）。
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        // 可失败性标记。
        fallibility: Fallibility,
        // 表的布局信息。
        layout: TableLayout,
    ) -> Result<(), TryReserveError>
    // 约束：A 必须实现 Allocator。
    where
        // Allocator 约束。
        A: Allocator,
    {
        // SAFETY: 我们确信 `alloc` 和 `layout` 与分配此表所用的 [`Allocator`] 和 [`TableLayout`] 一致。
        // 准备新表（分配并初始化，带 ScopeGuard 保护）。
        let mut new_table = self.prepare_resize(alloc, layout, capacity, fallibility)?;

        // SAFETY: 我们确信 RawTableInner 的寿命长于返回的 `FullBucketsIndices` 迭代器，
        // 且此函数的调用者确保控制字节已正确初始化。
        unsafe {
            // 遍历旧表中所有满桶的索引。
            for full_byte_index in self.full_buckets_indices() {
                // 这里可能 panic（哈希函数）。
                let hash = hasher(self, full_byte_index);

                // SAFETY:
                // 我们可以在此使用 insert() 的简化版本，因为：
                // 1. 表中没有 DELETED 条目。
                // 2. 我们确信表中有足够的空间。
                // 3. 所有元素都是唯一的。
                // 4. 此函数的调用者保证 `capacity > 0`，因此 `new_table` 必然已有已分配的内存。
                // 5. 我们会在循环结束后设置新表的 `growth_left` 和 `items` 字段。
                // 6. 调用此函数后，我们立即在返回的索引处插入与给定哈希匹配的数据。
                // 在新表中为新元素寻找插入索引。
                let (new_index, _) = new_table.prepare_insert_index(hash);

                // SAFETY:
                //
                // * `src` 用于读取 `layout.size` 字节是有效的，因为表仍存活，
                //   且 `full_byte_index` 保证在边界内（见 `FullBucketsIndices::next_impl`）；
                //
                // * `dst` 用于写入 `layout.size` 字节是有效的，因为调用者保证 `table_layout`
                //   与分配旧表所用的 [`TableLayout`] 一致，且我们持有 `prepare_insert_index`
                //   返回的 `new_index`。
                //
                // * `src` 与 `dst` 均已正确对齐。
                //
                // * `src` 与 `dst` 指向不同的内存区域。
                // 把元素数据从旧表的桶复制到新表的桶（不重叠复制）。
                ptr::copy_nonoverlapping(
                    // 源地址：旧表中该满桶的指针。
                    self.bucket_ptr(full_byte_index, layout.size),
                    // 目标地址：新表中该插入索引的指针。
                    new_table.bucket_ptr(new_index, layout.size),
                    // 复制字节数：单个元素的大小。
                    layout.size,
                );
            }
        }

        // 哈希函数没有 panic，因此可以安全地设置新表的 `growth_left` 和 `items` 字段。
        // 扣除已搬移的元素数，得到新表剩余可增长空间。
        new_table.growth_left -= self.items;
        // 新表的元素数与旧表相同。
        new_table.items = self.items;

        // 我们已成功复制所有元素且未 panic。现在把 self 替换为新表。旧表的内存将被释放，
        // 但其中的元素不会被 drop（因为它们已被移动到新表中）。
        // SAFETY: 调用者保证 `table_layout` 与分配此表所用的 [`TableLayout`] 一致。
        // 把 self 替换为新表。
        mem::swap(self, &mut new_table);

        // 调整大小成功。
        Ok(())
    }
    /// 原地重新哈希表的内容（即不改变内存分配）。
    ///
    /// 若 `hasher` panic，表的部分内容可能会丢失。
    ///
    /// 此处使用动态分发以减少生成的代码量，但内联后该开销会被 LLVM 优化消除。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`undefined behavior`]：
    ///
    /// * `size_of` 必须等于表中存储元素的大小；
    ///
    /// * `drop` 函数（`fn(*mut u8)`）必须是表中存储元素的真实 drop 函数。
    ///
    /// * [`RawTableInner`] 必须已分配；
    ///
    /// * [`RawTableInner`] 必须已正确初始化控制字节。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[cfg_attr(feature = "inline-more", expect(clippy::inline_always))]
    #[cfg_attr(feature = "inline-more", inline(always))]
    #[cfg_attr(not(feature = "inline-more"), inline)]
    // 原地重新哈希：清理墓碑并把各元素放回其理想位置。
    unsafe fn rehash_in_place(
        // 取自身可变引用。
        &mut self,
        // 哈希函数（动态分发）。
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        // 元素大小。
        size_of: usize,
        // 可选的元素 drop 函数。
        drop: Option<unsafe fn(*mut u8)>,
    ) {
        // 若哈希函数 panic，则妥善清理所有尚未重新哈希的元素。遗憾的是我们无法保留这些元素：
        // 我们丢失了它们的哈希，若不冒再次 panic 的风险就没有恢复它们的办法。
        unsafe {
            // 先把 FULL 控制字节转为 DELETED、DELETED 转为 EMPTY。
            self.prepare_rehash_in_place();
        }

        // 设置作用域守卫：若后续 panic，清理所有 DELETED 桶并修正 growth_left。
        let mut guard = guard(self, move |self_| {
            // 遍历所有桶。
            for i in 0..self_.num_buckets() {
                unsafe {
                    // 所有尚未重新哈希的元素都带有 DELETED 标签。它们需要被 drop，
                    // 并且把标签重置为 EMPTY。
                    if *self_.ctrl(i) == Tag::DELETED {
                        // 把控制字节重置为 EMPTY。
                        self_.set_ctrl(i, Tag::EMPTY);
                        // 若提供了 drop 函数，则析构该元素。
                        if let Some(drop) = drop {
                            drop(self_.bucket_ptr(i, size_of));
                        }
                        // 元素计数减 1。
                        self_.items -= 1;
                    }
                }
            }
            // 重新计算剩余可增长空间。
            self_.growth_left = bucket_mask_to_capacity(self_.bucket_mask) - self_.items;
        });

        // 此时，DELETED 元素就是尚未重新哈希的元素。找出它们，并按其理想位置重新插入。
        // 外层循环遍历所有桶。
        'outer: for i in 0..guard.num_buckets() {
            unsafe {
                // 跳过不是 DELETED 的桶（即已经处理过的）。
                if *guard.ctrl(i) != Tag::DELETED {
                    continue;
                }
            }

            // 取该待处理元素的指针。
            let i_p = unsafe { guard.bucket_ptr(i, size_of) };

            // 内层循环：不断为该元素寻找位置，直到安顿好。
            loop {
                // 对当前元素重新计算哈希
                let hash = hasher(*guard, i);

                // 为它寻找合适的插入位置
                //
                // SAFETY: 此函数的调用者确保控制字节已正确初始化。
                let new_i = unsafe { guard.find_insert_index(hash) };

                // 探测的工作方式是按组扫描所有控制字节，而这些组可能未按组大小对齐。
                // 若新位置与旧位置都落在同一个未对齐的组内，那么移动它毫无益处，
                // 我们只需继续处理下一个元素即可。
                if likely(guard.is_in_same_group(i, new_i, hash)) {
                    unsafe { guard.set_ctrl_hash(i, hash) };
                    continue 'outer;
                }

                // 取新位置的元素指针。
                let new_i_p = unsafe { guard.bucket_ptr(new_i, size_of) };

                // 我们要把当前元素移动到新位置。把我们的 H2 写入新位置的控制字节。
                let prev_ctrl = unsafe { guard.replace_ctrl_hash(new_i, hash) };
                if prev_ctrl == Tag::EMPTY {
                    unsafe { guard.set_ctrl(i, Tag::EMPTY) };
                    // 若目标槽位为空，直接把当前元素移入新槽位并清空旧控制字节。
                    unsafe {
                        ptr::copy_nonoverlapping(i_p, new_i_p, size_of);
                    }
                    continue 'outer;
                }

                // 若目标槽位已被占用，则交换两个元素，然后继续处理刚被换到旧槽位的那个元素。
                debug_assert_eq!(prev_ctrl, Tag::DELETED);
                unsafe {
                    ptr::swap_nonoverlapping(i_p, new_i_p, size_of);
                }
            }
        }

        // 所有元素安顿完毕，重新计算剩余可增长空间。
        guard.growth_left = bucket_mask_to_capacity(guard.bucket_mask) - guard.items;

        // 全部成功，解除守卫（避免清理逻辑再次执行）。
        mem::forget(guard);
    }

    /// 释放表内存，但不 drop 任何条目。
    ///
    /// # Note
    ///
    /// 此函数只能在 [`drop_elements`](RawTableInner::drop_elements) 之后调用，否则可能导致内存泄漏。
    /// 此外，调用此函数会自动使所有桶（[`Bucket`]）实例失效（悬垂），并使表的 `ctrl` 字段失效（悬垂）。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`Undefined Behavior`]：
    ///
    /// * [`RawTableInner`] 必须已分配；
    ///
    /// * `alloc` 必须与分配此表所用的 [`Allocator`] 是同一个。
    ///
    /// * `table_layout` 必须与分配此表所用的 [`TableLayout`] 是同一个。
    ///
    /// 更多信息另请参阅 [`GlobalAlloc::dealloc`] 或 [`Allocator::deallocate`]。
    ///
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    /// [`GlobalAlloc::dealloc`]: stdalloc::alloc::GlobalAlloc::dealloc
    /// [`Allocator::deallocate`]: stdalloc::alloc::Allocator::deallocate
    #[inline]
    // 释放表的内存分配。
    unsafe fn free_buckets<A>(&mut self, alloc: &A, table_layout: TableLayout)
    // 约束：A 必须实现 Allocator。
    where
        // Allocator 约束。
        A: Allocator,
    {
        unsafe {
            // SAFETY: 调用者必须遵守 `free_buckets` 方法的安全契约。
            // 取分配指针与布局。
            let (ptr, layout) = self.allocation_info(table_layout);
            // 通过分配器释放内存。
            alloc.deallocate(ptr, layout);
        }
    }

    /// 返回指向已分配内存的指针，以及分配表时所用的布局。
    ///
    /// # Safety
    ///
    /// 此函数的调用者必须遵守以下安全规则：
    ///
    /// * [`RawTableInner`] 必须已分配，否则调用此函数会导致 [`undefined behavior`]
    ///
    /// * `table_layout` 必须与分配此表所用的 [`TableLayout`] 是同一个。不满足此条件
    ///   可能导致 [`undefined behavior`]。
    ///
    /// 更多信息另请参阅 [`GlobalAlloc::dealloc`] 或 [`Allocator::deallocate`]。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    /// [`GlobalAlloc::dealloc`]: stdalloc::GlobalAlloc::dealloc
    /// [`Allocator::deallocate`]: stdalloc::Allocator::deallocate
    #[inline]
    // 返回内存分配指针与布局信息。
    unsafe fn allocation_info(&self, table_layout: TableLayout) -> (NonNull<u8>, Layout) {
        // 调试断言：只能在非空表上调用。
        debug_assert!(
            // 判断是否为非空表单例。
            !self.is_empty_singleton(),
            // 断言失败信息（保持原样的字符串字面量）。
            "this function can only be called on non-empty tables"
        );

        // 计算布局与控制字节偏移。
        let (layout, ctrl_offset) = {
            // 以桶数计算内存布局。
            let option = table_layout.calculate_layout_for(self.num_buckets());
            unsafe { option.unwrap_unchecked() }
        };
        (
            // SAFETY: 调用者必须遵守 `allocation_info` 方法的安全契约。
            unsafe { NonNull::new_unchecked(self.ctrl.as_ptr().sub(ctrl_offset)) },
            // 布局信息。
            layout,
        )
    }

    /// 返回哈希表内部分配的内存总量，单位为字节。
    ///
    /// 返回的数字仅供参考，主要用于内存性能分析。
    ///
    /// # Safety
    ///
    /// `table_layout` 必须与分配此表所用的 [`TableLayout`] 是同一个。不满足此条件
    /// 可能导致 [`undefined behavior`]。
    ///
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 返回表分配的内存大小；空表返回 0。
    unsafe fn allocation_size_or_zero(&self, table_layout: TableLayout) -> usize {
        // 若是未分配的空表单例。
        if self.is_empty_singleton() {
            // 大小为 0。
            0
        } else {
            // SAFETY:
            // 1. 我们已检查该表已分配。
            // 2. 调用者保证 `table_layout` 与分配此表所用的 [`TableLayout`] 一致。
            unsafe { self.allocation_info(table_layout).1.size() }
        }
    }

    /// 把表中所有桶标记为空，但不 drop 其内容。
    #[inline]
    // 清空表的所有标记信息但不析构元素。
    fn clear_no_drop(&mut self) {
        // 若表已分配，则清空控制字节。
        if !self.is_empty_singleton() {
            // 把所有控制字节填充为 EMPTY。
            self.ctrl_slice().fill_empty();
        }
        // 元素计数清零。
        self.items = 0;
        // 重新计算剩余可增长空间。
        self.growth_left = bucket_mask_to_capacity(self.bucket_mask);
    }

    /// 擦除给定索引处 [`Bucket`] 的控制字节，使其不再被判定为满，减少表的 `items`，
    /// 并在可能的情况下增加 `self.growth_left`。
    ///
    /// 此函数实际上并不会擦除/drop [`Bucket`] 本身，即它不会对表的 `data` 部分做任何更改。
    /// 此函数的调用者必须负责正确地 drop `data`，否则调用此函数可能导致内存泄漏。
    ///
    /// # Safety
    ///
    /// 调用此函数时必须遵守以下安全规则：
    ///
    /// * [`RawTableInner`] 必须已分配；
    ///
    /// * 给定位置的控制字节必须是满（FULL）的；
    ///
    /// * `index` 不得大于 `RawTableInner.bucket_mask`，即 `index <= RawTableInner.bucket_mask`，
    ///   换句话说，`(index + 1)` 不得大于 [`RawTableInner::num_buckets`] 函数返回的数量。
    ///
    /// 对尚未分配的表调用此函数会导致 [`undefined behavior`]。
    ///
    /// 对没有元素的表调用此函数的行为未指定，但后续调用其他函数很可能因减法下溢
    /// 而导致 [`undefined behavior`]（`当 self.items == 0 时 self.items -= 1 会溢出`）。
    ///
    /// 另请参阅 [`Bucket::as_ptr`] 方法，了解如何正确地从 [`RawTable`] / [`RawTableInner`]
    /// 中移除或保存 `data element`。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    // 擦除指定索引处桶的控制字节（不析构元素数据）。
    unsafe fn erase(&mut self, index: usize) {
        unsafe {
            // 调试断言：该桶必须是满的。
            debug_assert!(self.is_bucket_full(index));
        }

        // 这等价于 `index.wrapping_sub(Group::WIDTH) % self.num_buckets()`，因为桶数是 2 的幂，
        // 且 `self.bucket_mask = self.num_buckets() - 1`。
        // 计算 index 前一个组的对齐位置。
        let index_before = index.wrapping_sub(Group::WIDTH) & self.bucket_mask;
        // SAFETY:
        // - 调用者必须遵守 `erase` 方法的安全契约；
        // - 由于使用了 `self.bucket_mask` 掩码，`index_before` 保证在范围内。
        let (empty_before, empty_after) = unsafe {
            (
                // 加载 index_before 所在组的空位掩码。
                Group::load(self.ctrl(index_before)).match_empty(),
                // 加载 index 所在组的空位掩码。
                Group::load(self.ctrl(index)).match_empty(),
            )
        };

        // 在映射中的插入与搜索由两个关键函数完成：
        //
        // - `find_insert_index` 函数：查找组内任意 `Tag::EMPTY` 或 `Tag::DELETED` 槽位的索引以便插入。
        //   若在第一个组中立即没有找到 `Tag::EMPTY` 或 `Tag::DELETED` 槽位，它会跳到下一个 `Group`
        //   继续寻找，依此类推，直到遍历完控制字节中的所有组。
        //
        // - `find_inner` 函数：通过查看组内所有 `FULL` 字节来寻找目标元素的索引。若它没有立即找到元素，
        //   且组内没有 `Tag::EMPTY` 字节，则意味着 `find_insert_index` 可能在下一个组中找到了合适的槽位。
        //   因此 `find_inner` 会继续往后跳，若仍未找到目标元素且再次没有 `Tag::EMPTY` 字节，
        //   则继续往后跳，依此类推。只有当 `find_inner` 找到目标元素或碰到 `Tag::EMPTY` 槽位/字节时，
        //   搜索才会停止。
        //
        // 由此带来两个推论：
        //
        // - 映射中必须存在 `Tag::EMPTY` 槽位（字节）；
        //
        // - 不能简单地把待擦除的字节标记为 `Tag::EMPTY`，否则 `find_inner` 可能在找到目标元素之前
        //   就碰到 `Tag::EMPTY` 字节而停止搜索。
        //
        // 因此必须检查被擦除元素之后和之前的所有字节。若我们处于一个由 `FULL` 或 `Tag::DELETED`
        // 字节构成的连续 `Group` 中（前后 `FULL` 或 `Tag::DELETED` 字节的数量大于等于
        // `Group::WIDTH`），那么必须把我们的字节标记为 `Tag::DELETED`，以便 `find_inner` 继续往后
        // 搜索。另一方面，若 `Group` 中存在至少一个 `Tag::EMPTY` 槽位，那么 `find_inner` 反正也会
        // 碰到 `Tag::EMPTY` 字节，因此我们可以安全地把被擦除的字节也标记为 `Tag::EMPTY`。
        //
        // 最后，由于 `index_before == (index.wrapping_sub(Group::WIDTH) & self.bucket_mask) == index`，
        // 且综合上述所有内容，小于组宽度的表（self.num_buckets() < Group::WIDTH）不可能出现
        // `Tag::DELETED` 字节。
        //
        // 注意，在此上下文中 `leading_zeros` 指的是组末尾的字节，而 `trailing_zeros`
        // 指的是组开头的字节。
        // 根据前后空位分布决定新控制字节：连续满则留墓碑，否则标记为 EMPTY 并回收增长空间。
        let ctrl = if empty_before.leading_zeros() + empty_after.trailing_zeros() >= Group::WIDTH {
            // 前后都连续，标记为 DELETED（墓碑）。
            Tag::DELETED
        } else {
            // 存在相邻空位，剩余可增长空间加 1。
            self.growth_left += 1;
            // 标记为 EMPTY。
            Tag::EMPTY
        };
        // SAFETY: 调用者必须遵守 `erase` 方法的安全契约。
        unsafe {
            // 把新的控制字节写入指定索引（并同步尾部副本）。
            self.set_ctrl(index, ctrl);
        }
        // 元素计数减 1。
        self.items -= 1;
    }
}

// 为可克隆类型实现表的 Clone trait。
impl<T: Clone, A: Allocator + Clone> Clone for RawTable<T, A> {
    // 克隆表：分配同尺寸新表并复制所有元素。
    fn clone(&self) -> Self {
        // 若原表是未分配的空表单例。
        if self.table.is_empty_singleton() {
            // 直接用相同的分配器创建新的空表。
            Self::new_in(self.alloc.clone())
        } else {
            // SAFETY: 这是安全的，因为我们取的是已分配表的大小，因此不会发生容量溢出，
            // `self.table.num_buckets()` 是 2 的幂，且所有分配器错误都会在
            // `RawTableInner::new_uninitialized` 内部被捕获。
            let result = unsafe {
                Self::new_uninitialized(
                    // 克隆分配器。
                    self.alloc.clone(),
                    // 新表桶数与原表相同。
                    self.table.num_buckets(),
                    // 不可失败模式。
                    Fallibility::Infallible,
                )
            };

            // SAFETY: 调用 `new_uninitialized` 函数的结果不可能是错误，
            // 因为 `fallibility == Fallibility::Infallible。
            let mut new_table = unsafe { result.unwrap_unchecked() };

            // SAFETY:
            // 克隆元素可能失败（clone 函数可能 panic）。但我们无需担心未初始化的控制位，因为：
            // 1. 表中的元素数量为零，这意味着 Drop 函数不会读取控制位。
            // 2. `clone_from_spec` 方法会先从 `self` 复制所有控制位（从而初始化它们）。
            //    但这不会影响 `Drop` 函数，因为 `clone_from_spec` 函数只有在成功克隆所有元素之后
            //    才会设置 `items`。
            unsafe { new_table.clone_from_spec(self) };
            // 返回克隆出的新表。
            new_table
        }
    }

    // 用 source 的内容替换 self 的内容。
    fn clone_from(&mut self, source: &Self) {
        // 若源表是未分配的空表单例。
        if source.table.is_empty_singleton() {
            // 用新的空表单例替换内部表。
            let mut old_inner = mem::replace(&mut self.table, RawTableInner::NEW);
            unsafe {
                // SAFETY:
                // 1. 我们只调用该函数一次；
                // 2. 我们确信 `alloc` 和 `table_layout` 与分配此表所用的 [`Allocator`]
                //    和 [`TableLayout`] 一致。
                // 3. 若任何元素的 drop 函数 panic，则只会造成内存泄漏，
                //    因为我们已经把内部表替换为新表。
                // 释放旧表的内存。
                old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
            }
        } else {
            unsafe {
                // 确保一旦发生 panic，我们就清空表并使其处于空表状态。
                let mut self_ = guard(self, |self_| {
                    // panic 时清空表（不析构元素）。
                    self_.clear_no_drop();
                });

                // 首先 drop 我们的所有元素，但不清理控制字节。若此过程 panic，
                // 作用域守卫会清空表，泄漏尚未 drop 的元素。
                //
                // 该泄漏不可避免：我们不能尝试 drop 更多元素，因为这可能导致再次 panic
                // 并中止进程。
                //
                // SAFETY: 若出了问题，我们在 drop 元素之后立即清空表，因此不会双重 drop，
                // 因为 `items` 将等于零。
                self_.table.drop_elements::<T>();

                // 如有必要，调整我们的表以匹配源表。
                if self_.num_buckets() != source.num_buckets() {
                    // 分配与源表相同桶数的新内部表。
                    let new_inner = {
                        // 创建未初始化的新表。
                        let result = RawTableInner::new_uninitialized(
                            // 使用自身的分配器。
                            &self_.alloc,
                            // 表布局。
                            Self::TABLE_LAYOUT,
                            // 与源表相同的桶数。
                            source.num_buckets(),
                            // 不可失败模式。
                            Fallibility::Infallible,
                        );
                        // 解包结果（不可失败）。
                        result.unwrap_unchecked()
                    };
                    // 用新的未初始化内部表替换旧的。这样做没问题，因为若出了问题，
                    // `ScopeGuard` 会初始化所有控制字节并留下空表。
                    let mut old_inner = mem::replace(&mut self_.table, new_inner);
                    // 若旧表已分配。
                    if !old_inner.is_empty_singleton() {
                        // SAFETY:
                        // 1. 我们已检查该表已分配。
                        // 2. 我们确信 `alloc` 和 `table_layout` 与
                        // 分配此表所用的 [`Allocator`] 和 [`TableLayout`] 一致。
                        // 释放旧表内存。
                        old_inner.free_buckets(&self_.alloc, Self::TABLE_LAYOUT);
                    }
                }

                // 克隆元素可能失败（clone 函数可能 panic），但 `clone_from_impl` 函数内部的
                // `ScopeGuard` 会处理这种情况，在必要时 drop 所有已克隆的元素。我们的
                // `ScopeGuard` 会清空表。
                self_.clone_from_spec(source);

                // 若克隆成功，解除作用域守卫。
                ScopeGuard::into_inner(self_);
            }
        }
    }
}

/// 针对 `Copy` 类型的 `clone_from` 特化
// 为 clone_from 提供可特化的 trait。
trait RawTableClone {
    // 按特化方式从 source 克隆内容。
    unsafe fn clone_from_spec(&mut self, source: &Self);
}
// 通用实现：走 clone_from_impl 的通用路径。
impl<T: Clone, A: Allocator + Clone> RawTableClone for RawTable<T, A> {
    // 使用 default_fn! 宏定义默认方法（可被特化覆盖）。
    default_fn! {
        // 内联属性。
        #[cfg_attr(feature = "inline-more", inline)]
        // 默认的克隆实现：调用通用克隆逻辑。
        unsafe fn clone_from_spec(&mut self, source: &Self) {
            unsafe {
                // 调用通用的克隆实现。
                self.clone_from_impl(source);
            }
        }
    }
}
// 仅在 nightly 特性下编译。
#[cfg(feature = "nightly")]
// 针对 TrivialClone 类型（可平凡克隆）的特化实现。
impl<T: core::clone::TrivialClone, A: Allocator + Clone> RawTableClone for RawTable<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 特化的克隆实现：直接按字节复制控制字节与数据。
    unsafe fn clone_from_spec(&mut self, source: &Self) {
        unsafe {
            // 一次性复制全部控制字节。
            source
                // 源表的控制字节。
                .table
                // 取源表第 0 个控制字节指针。
                .ctrl(0)
                // 复制到目标表。
                .copy_to_nonoverlapping(self.table.ctrl(0), self.table.num_ctrl_bytes());
            // 一次性复制全部数据元素。
            source
                // 源表数据起始指针。
                .data_start()
                // 转为裸指针。
                .as_ptr()
                // 复制到目标表数据区。
                .copy_to_nonoverlapping(self.data_start().as_ptr(), self.table.num_buckets());
        }

        // 复制元素计数。
        self.table.items = source.table.items;
        // 复制剩余可增长空间。
        self.table.growth_left = source.table.growth_left;
    }
}

// 为可克隆类型提供克隆辅助方法。
impl<T: Clone, A: Allocator + Clone> RawTable<T, A> {
    /// `clone` 和 `clone_from` 的公共代码。假设：
    /// - `self.num_buckets() == source.num_buckets()`。
    /// - 任何已存在的元素都已被 drop。
    /// - 控制字节尚未初始化。
    #[cfg_attr(feature = "inline-more", inline)]
    // 通用克隆实现：复制控制字节后逐个克隆元素。
    unsafe fn clone_from_impl(&mut self, source: &Self) {
        // 原样复制控制字节。我们在单次遍历中完成
        unsafe {
            source
                // 源表。
                .table
                // 取源表第 0 个控制字节指针。
                .ctrl(0)
                // 复制全部控制字节到目标表。
                .copy_to_nonoverlapping(self.table.ctrl(0), self.table.num_ctrl_bytes());
        }

        // 元素的克隆可能 panic，此时我们需要确保只 drop 已经克隆好的元素。
        let mut guard = guard((0, &mut *self), |(index, self_)| {
            // 若元素类型需要 drop。
            if T::NEEDS_DROP {
                // 遍历已克隆的索引范围。
                for i in 0..*index {
                    unsafe {
                        // 若该桶已满（已写入元素）。
                        if self_.is_bucket_full(i) {
                            // drop 该已克隆的元素。
                            self_.bucket(i).drop();
                        }
                    }
                }
            }
        });

        unsafe {
            // 遍历源表中的所有元素。
            for from in source.iter() {
                // 取源元素的桶索引。
                let index = source.bucket_index(&from);
                // 取目标表的对应桶。
                let to = guard.1.bucket(index);
                // 把克隆的元素写入目标桶。
                to.write(from.as_ref().clone());

                // 更新索引，以备展开（unwind）时使用。
                guard.0 = index + 1;
            }
        }

        // 成功克隆了所有元素，无需清理。
        mem::forget(guard);

        // 复制源表的元素计数。
        self.table.items = source.table.items;
        // 复制源表的剩余可增长空间。
        self.table.growth_left = source.table.growth_left;
    }
}
// 为 RawTable 实现 Default trait。
impl<T, A: Allocator + Default> Default for RawTable<T, A> {
    #[inline]
    // 创建默认（空）的表。
    fn default() -> Self {
        // 用分配器的默认值创建新表。
        Self::new_in(Default::default())
    }
}

// 仅在 nightly 特性下编译。
#[cfg(feature = "nightly")]
// nightly 下利用 may_dangle 放宽 Drop 检查的变体。
unsafe impl<#[may_dangle] T, A: Allocator> Drop for RawTable<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 析构表：先 drop 元素再释放内存。
    fn drop(&mut self) {
        // SAFETY:
        // 1. 我们只调用该函数一次；
        // 2. 我们确信 `alloc` 和 `table_layout` 与分配此表所用的 [`Allocator`]
        //    和 [`TableLayout`] 一致。
        // 3. 若任何元素的 drop 函数失败，则只会造成内存泄漏，
        //    而我们并不在意，因为我们正处于 `RawTable` 的 `Drop` 函数中，
        //    因此不会留下处于不一致状态的表。
        unsafe {
            // 析构内部表（drop 元素并释放内存）。
            self.table
                .drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
        }
    }
}
// 非 nightly 编译时的 Drop 实现。
#[cfg(not(feature = "nightly"))]
impl<T, A: Allocator> Drop for RawTable<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 析构表：先 drop 元素再释放内存。
    fn drop(&mut self) {
        // SAFETY:
        // 1. 我们只调用该函数一次；
        // 2. 我们确信 `alloc` 和 `table_layout` 与分配此表所用的 [`Allocator`]
        //    和 [`TableLayout`] 一致。
        // 3. 若任何元素的 drop 函数失败，则只会造成内存泄漏，
        //    而我们并不在意，因为我们正处于 `RawTable` 的 `Drop` 函数中，
        //    因此不会留下处于不一致状态的表。
        unsafe {
            // 析构内部表（drop 元素并释放内存）。
            self.table
                .drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
        }
    }
}

// 为 RawTable 实现 IntoIterator trait，使其可用 for 循环消费。
impl<T, A: Allocator> IntoIterator for RawTable<T, A> {
    // 迭代器产出的元素类型。
    type Item = T;
    // 对应的迭代器类型。
    type IntoIter = RawIntoIter<T, A>;

    #[cfg_attr(feature = "inline-more", inline)]
    // 把表转换为消耗型迭代器。
    fn into_iter(self) -> RawIntoIter<T, A> {
        unsafe {
            // 先创建原始迭代器。
            let iter = self.iter();
            // 从迭代器当前位置开始构建消耗型迭代器。
            self.into_iter_from(iter)
        }
    }
}

/// 遍历表某个子范围的迭代器。与 `RawIter` 不同，此迭代器不跟踪元素计数。
pub(crate) struct RawIterRange<T> {
    // 当前组中满桶的位掩码。每处理一个元素，就会从该掩码中清除对应位。
    current_group: BitMaskIter,

    // 指向当前组对应桶的指针。
    data: Bucket<T>,

    // 指向下一组控制字节的指针，
    // 必须按组大小对齐。
    next_ctrl: *const u8,

    // 指向本范围最后一个控制字节之后的位置。
    end: *const u8,
}

// 为 RawIterRange 实现构造与其他方法。
impl<T> RawIterRange<T> {
    /// 返回覆盖表某个子集的 `RawIterRange`。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`undefined behavior`]：
    ///
    /// * `ctrl` 必须可用于读取，即表的寿命长于 `RawIterRange`；
    ///
    /// * `ctrl` 必须按组大小（`Group::WIDTH`）正确对齐；
    ///
    /// * `ctrl` 必须指向已正确初始化的控制字节数组；
    ///
    /// * `data` 必须是表中 `ctrl` 索引处的 [`Bucket`]；
    ///
    /// * `len` 的值必须小于或等于表的桶数，且 `ctrl.as_ptr().add(len).offset_from(ctrl.as_ptr())`
    ///   的返回值必须为正。
    ///
    /// * `ctrl.add(len)` 指针必须在边界内，或指向同一个 [allocated table] 末尾之后一个字节。
    ///
    /// * `len` 必须是 2 的幂。
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[cfg_attr(feature = "inline-more", inline)]
    // 从控制字节指针、数据桶与长度构造范围迭代器。
    unsafe fn new(ctrl: *const u8, data: Bucket<T>, len: usize) -> Self {
        // 调试断言：长度不能为 0。
        debug_assert_ne!(len, 0);
        // 调试断言：ctrl 必须按 Group 对齐。
        debug_assert!(ctrl.cast::<Group>().is_aligned());
        // SAFETY: 调用者必须遵守 [`RawIterRange::new`] 的安全规则
        // 计算范围的结束指针。
        let end = unsafe { ctrl.add(len) };

        // 加载第一组并让 ctrl 前进指向下一组
        // SAFETY: 调用者必须遵守 [`RawIterRange::new`] 的安全规则
        let (current_group, next_ctrl) = unsafe {
            (
                // 对齐加载第一组控制字节并匹配出满桶位掩码。
                Group::load_aligned(ctrl.cast()).match_full(),
                // ctrl 前进一个组宽度。
                ctrl.add(Group::WIDTH),
            )
        };

        Self {
            // 把满桶位掩码转换为迭代器。
            current_group: current_group.into_iter(),
            // 记录数据桶。
            data,
            // 记录下一组控制字节指针。
            next_ctrl,
            // 记录结束指针。
            end,
        }
    }

    /// 把一个 `RawIterRange` 拆分为两半。
    ///
    /// 若剩余范围小于或等于组宽度，则返回 `None`。
    #[cfg_attr(feature = "inline-more", inline)]
    // 仅在 rayon 特性下编译。
    #[cfg(feature = "rayon")]
    pub(crate) fn split(mut self) -> (Self, Option<RawIterRange<T>>) {
        unsafe {
            // 若已到达或越过末尾。
            if self.end <= self.next_ctrl {
                // 若当前正在处理的组是最后一组，则无需拆分。
                (self, None)
            } else {
                // len 是当前正在处理的组之后的剩余元素数。它必须是组大小的倍数
                // （小表已被上面的检查捕获）。
                let len = offset_from(self.end, self.next_ctrl);
                // 调试断言：长度是组宽度的倍数。
                debug_assert_eq!(len % Group::WIDTH, 0);

                // 把剩余元素拆成两半，但在剩余组数为奇数时把中点向下取整。这确保了：
                // - 尾部至少有 1 个组。
                // - 考虑到我们还有当前组要处理，拆分大致均匀。
                let mid = (len / 2) & !(Group::WIDTH - 1);

                // 构造覆盖尾部的迭代器。
                let tail = Self::new(
                    // 尾部的控制字节起点。
                    self.next_ctrl.add(mid),
                    // 尾部的数据桶起点。
                    self.data.next_n(Group::WIDTH).next_n(mid),
                    // 尾部的长度。
                    len - mid,
                );
                // 调试断言：数据桶位置正确。
                debug_assert_eq!(
                    self.data.next_n(Group::WIDTH).next_n(mid).ptr,
                    tail.data.ptr
                );
                // 调试断言：结束指针一致。
                debug_assert_eq!(self.end, tail.end);
                // 截断自身，使其只覆盖前半部分。
                self.end = self.next_ctrl.add(mid);
                // 调试断言：拆分边界正确。
                debug_assert_eq!(self.end.add(Group::WIDTH), tail.next_ctrl);
                // 返回自身与可选的尾部。
                (self, Some(tail))
            }
        }
    }

    /// # Safety
    /// 若 `DO_CHECK_PTR_RANGE` 为 false，调用者必须确保我们不会在产出所有元素之后继续迭代。
    #[cfg_attr(feature = "inline-more", inline)]
    // 前进到下一个满桶；DO_CHECK_PTR_RANGE 决定是否检查指针范围。
    unsafe fn next_impl<const DO_CHECK_PTR_RANGE: bool>(&mut self) -> Option<Bucket<T>> {
        // 循环直到找到满桶或耗尽范围。
        loop {
            // 若当前组中还有未处理的满桶位。
            if let Some(index) = self.current_group.next() {
                // 返回该索引对应的桶。
                return Some(unsafe { self.data.next_n(index) });
            }

            // 若启用了指针范围检查且已到达末尾。
            if DO_CHECK_PTR_RANGE && self.next_ctrl >= self.end {
                // 返回 None 表示迭代结束。
                return None;
            }

            // 我们可能会越过 self.end 最多读到下一个组的边界，但这没有问题，
            // 因为这只会发生在小于组大小的表上，此时尾部的控制字节都是 EMPTY。
            // 在更大的表上，self.end 保证按组大小对齐（因为表的大小是 2 的幂）。
            unsafe {
                // 加载下一组控制字节并重建满桶位掩码迭代器。
                self.current_group = Group::load_aligned(self.next_ctrl.cast())
                    .match_full()
                    .into_iter();
                // 数据桶前进一个组宽度。
                self.data = self.data.next_n(Group::WIDTH);
                // 控制字节指针前进一个组宽度。
                self.next_ctrl = self.next_ctrl.add(Group::WIDTH);
            }
        }
    }

    /// 通过对每个元素应用某个操作，把所有元素折叠进累加器，并返回最终结果。
    ///
    /// `fold_impl()` 接受三个参数：迭代器中剩余的元素数量、一个初始值，以及一个接受两个参数的
    /// 闭包：'累加器' 和一个元素。闭包返回累加器在下一次迭代时应具有的值。
    ///
    /// 初始值是累加器在第一次调用时将具有的值。
    ///
    /// 在对迭代器的每个元素应用此闭包之后，`fold_impl()` 返回累加器。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`Undefined Behavior`]：
    ///
    /// * [`RawTableInner`] / [`RawTable`] 必须存活且未被移动，即表的寿命长于 `RawIterRange`；
    ///
    /// * 提供的 `n` 值必须与表中实际的元素数量一致。
    ///
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[expect(clippy::while_let_on_iterator)]
    #[cfg_attr(feature = "inline-more", inline)]
    // 折叠实现：对每个满桶元素依次应用折叠函数。
    unsafe fn fold_impl<F, B>(mut self, mut n: usize, mut acc: B, mut f: F) -> B
    // 约束：F 是折叠函数。
    where
        // FnMut 约束。
        F: FnMut(B, Bucket<T>) -> B,
    {
        // 循环处理所有剩余元素。
        loop {
            // 处理当前组内的所有满桶。
            while let Some(index) = self.current_group.next() {
                // 返回的 `index` 将始终处于 `0..Group::WIDTH` 范围内，
                // 因此调用 `self.data.next_n(index)` 是安全的（详见下方解释）。
                // 调试断言：还有剩余元素。
                debug_assert!(n != 0);
                // 取该索引对应的桶。
                let bucket = unsafe { self.data.next_n(index) };
                // 把桶元素折叠进累加器。
                acc = f(acc, bucket);
                // 剩余计数减 1。
                n -= 1;
            }

            // 若所有元素都已处理完。
            if n == 0 {
                // 返回最终累加值。
                return acc;
            }

            // SAFETY: 此函数的调用者确保：
            //
            // 1. 提供的 `n` 值与表中实际的元素数量一致；
            // 2. 表存活且未被移动。
            //
            // 考虑到上述条件，我们始终保持在边界内，因为：
            //
            // 1. 对于小于组宽度的表（self.num_buckets() <= Group::WIDTH），
            //    我们永远不会进入该分支，因为此时我们应该已经产出了表的所有元素。
            //
            // 2. 对于大于组宽度的表。桶数是 2 的幂（2 ^ n），Group::WIDTH 也是 2 的幂（2 ^ k）。
            //    由于 `(2 ^ n) > (2 ^ k)`，因此 `(2 ^ n) % (2 ^ k) = 0`。由于我们从控制字节数组的
            //    起始处开始，且在取得所有元素之后绝不尝试继续迭代，因此最后一个 `self.current_group`
            //    将从 `self.num_buckets() - Group::WIDTH` 索引处读取字节。我们还知道
            //    `self.current_group.next()` 返回的索引始终处于 `0..Group::WIDTH` 范围内。
            //
            //    知道了以上内容，并考虑到我们把 `self.data` 的索引与读取 `self.current_group`
            //    时使用的索引同步了，后续的 `self.data.next_n(index)` 将始终返回索引号小于
            //    `self.num_buckets()` 的桶。
            //
            //    最后一个 `self.next_ctrl`（其索引为 `self.num_buckets()`）实际上永远不会被读取，
            //    因为此时我们应该已经产出了表的所有元素。
            unsafe {
                // 加载下一组控制字节并重建满桶位掩码迭代器。
                self.current_group = Group::load_aligned(self.next_ctrl.cast())
                    .match_full()
                    .into_iter();
                // 数据桶前进一个组宽度。
                self.data = self.data.next_n(Group::WIDTH);
                // 控制字节指针前进一个组宽度。
                self.next_ctrl = self.next_ctrl.add(Group::WIDTH);
            }
        }
    }
}

// 我们让原始迭代器无条件实现 Send 和 Sync，而由实际迭代器实现中的 PhantomData 决定真正的 Send/Sync 约束。
unsafe impl<T> Send for RawIterRange<T> {}
unsafe impl<T> Sync for RawIterRange<T> {}

// 为 RawIterRange 实现 Clone trait。
impl<T> Clone for RawIterRange<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 克隆范围迭代器。
    fn clone(&self) -> Self {
        Self {
            // 克隆数据桶。
            data: self.data.clone(),
            // 复制下一组控制字节指针。
            next_ctrl: self.next_ctrl,
            // 克隆当前组的位掩码迭代器。
            current_group: self.current_group.clone(),
            // 复制结束指针。
            end: self.end,
        }
    }
}

// 为 RawIterRange 实现 Iterator trait。
impl<T> Iterator for RawIterRange<T> {
    // 迭代器产出的元素类型。
    type Item = Bucket<T>;

    #[cfg_attr(feature = "inline-more", inline)]
    // 前进到下一个满桶并返回它。
    fn next(&mut self) -> Option<Bucket<T>> {
        unsafe {
            // SAFETY: 我们把检查标志设为 true。
            self.next_impl::<true>()
        }
    }

    #[inline]
    // 返回迭代器剩余元素的数量提示。
    fn size_hint(&self) -> (usize, Option<usize>) {
        // 我们没有元素计数，因此只能根据范围大小进行猜测。
        let remaining_buckets = if self.end > self.next_ctrl {
            // 计算剩余的控制字节数。
            unsafe { offset_from(self.end, self.next_ctrl) }
        } else {
            // 已无剩余字节。
            0
        };

        // 加上一个组宽度，以包含我们当前正在处理的组。
        (0, Some(Group::WIDTH + remaining_buckets))
    }
}

// 标记 RawIterRange 为融合迭代器。
impl<T> FusedIterator for RawIterRange<T> {}

/// 返回表中每个满桶的裸指针的迭代器。
///
/// 为了获得最大的灵活性，此迭代器不受生命周期约束，但在使用它时你必须遵守以下几条规则：
/// - 迭代期间不得释放哈希表（包括通过扩容/缩容的方式）。
/// - 擦除迭代器已经产出过的桶是可以的。
/// - 擦除迭代器尚未产出过的桶，仍可能导致迭代器产出该桶（除非调用了 `reflect_remove`）。
/// - 迭代器创建之后插入的元素是否会被该迭代器产出是未指定的（除非调用了 `reflect_insert`）。
/// - 迭代器产出桶的顺序是未指定的，并且将来可能会改变。
pub(crate) struct RawIter<T> {
    // 内部的范围迭代器。
    pub(crate) iter: RawIterRange<T>,
    // 剩余待产出的元素数量。
    items: usize,
}

// 为 RawIter 实现辅助方法。
impl<T> RawIter<T> {
    // drop 迭代器剩余的所有元素。
    unsafe fn drop_elements(&mut self) {
        unsafe {
            // 若元素类型需要 drop 且还有剩余元素。
            if T::NEEDS_DROP && self.items != 0 {
                // 遍历剩余的元素。
                for item in self {
                    // 逐个 drop 元素。
                    item.drop();
                }
            }
        }
    }
}

// 为 RawIter 实现 Clone trait。
impl<T> Clone for RawIter<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 克隆迭代器。
    fn clone(&self) -> Self {
        Self {
            // 克隆内部迭代器。
            iter: self.iter.clone(),
            // 复制剩余元素计数。
            items: self.items,
        }
    }
}
// 为 RawIter 实现 Default trait。
impl<T> Default for RawIter<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 创建默认的空迭代器。
    fn default() -> Self {
        // SAFETY: 因为该表是静态的，所以它的寿命总是长于迭代器。
        unsafe { RawTableInner::NEW.iter() }
    }
}

// 为 RawIter 实现 Iterator trait。
impl<T> Iterator for RawIter<T> {
    // 迭代器产出的元素类型。
    type Item = Bucket<T>;

    #[cfg_attr(feature = "inline-more", inline)]
    // 前进到下一个满桶并返回它。
    fn next(&mut self) -> Option<Bucket<T>> {
        // 内部迭代器按桶遍历，
        // 因此若我们已经产出了所有元素，它可能会做无用功。
        if self.items == 0 {
            // 剩余计数为 0，直接返回 None。
            return None;
        }

        let nxt = unsafe {
            // SAFETY: 我们使用 `items` 字段检查待产出的元素数量。
            self.iter.next_impl::<false>()
        };

        // 调试断言：必须取到了元素。
        debug_assert!(nxt.is_some());
        // 剩余计数减 1。
        self.items -= 1;

        // 返回取到的元素。
        nxt
    }

    #[inline]
    // 返回剩余元素数量的精确提示。
    fn size_hint(&self) -> (usize, Option<usize>) {
        // 剩余数量即 items 的值。
        (self.items, Some(self.items))
    }

    #[inline]
    // 用折叠实现高效地折叠所有剩余元素。
    fn fold<B, F>(self, init: B, f: F) -> B
    // 约束：Self 必须可确定大小。
    where
        // Sized 约束。
        Self: Sized,
        // FnMut 约束。
        F: FnMut(B, Self::Item) -> B,
    {
        unsafe { self.iter.fold_impl(self.items, init, f) }
    }
}

// 标记 RawIter 为精确大小迭代器。
impl<T> ExactSizeIterator for RawIter<T> {}
// 标记 RawIter 为融合迭代器。
impl<T> FusedIterator for RawIter<T> {}

/// 返回表中每个满桶索引的迭代器。
///
/// 为了获得最大的灵活性，此迭代器不受生命周期约束，但在使用它时你必须遵守以下几条规则：
/// - 迭代期间不得释放哈希表（包括通过扩容/缩容的方式）。
/// - 擦除迭代器已经产出过的桶是可以的。
/// - 擦除迭代器尚未产出过的桶，仍可能导致迭代器产出该桶的索引。
/// - 迭代器创建之后插入的元素是否会被该迭代器产出是未指定的。
/// - 迭代器产出桶索引的顺序是未指定的，并且将来可能会改变。
#[derive(Clone)]
pub(crate) struct FullBucketsIndices {
    // 当前组中满桶的位掩码。每处理一个元素，就会从该掩码中清除对应位。
    current_group: BitMaskIter,

    // 当前组字节索引的初始值（相对于控制字节的起始处）。
    group_first_index: usize,

    // 指向当前组控制字节的指针，
    // 必须按组大小（Group::WIDTH）对齐。
    ctrl: NonNull<u8>,

    // 表中的元素数量。
    items: usize,
}

// 为 FullBucketsIndices 实现 Default trait。
impl Default for FullBucketsIndices {
    #[cfg_attr(feature = "inline-more", inline)]
    // 创建默认的空迭代器。
    fn default() -> Self {
        // SAFETY: 因为该表是静态的，所以它的寿命总是长于迭代器。
        unsafe { RawTableInner::NEW.full_buckets_indices() }
    }
}
// 为 FullBucketsIndices 实现核心方法。
impl FullBucketsIndices {
    /// 前进迭代器并返回下一个值。
    ///
    /// # Safety
    ///
    /// 若违反以下任一条件，结果为 [`Undefined Behavior`]：
    ///
    /// * [`RawTableInner`] / [`RawTable`] 必须存活且未被移动，即表的寿命长于 `FullBucketsIndices`；
    ///
    /// * 绝不在取得所有元素之后继续尝试迭代。
    ///
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline(always)]
    // 前进到下一个满桶索引。
    unsafe fn next_impl(&mut self) -> Option<usize> {
        // 循环直到找到满桶或遍历结束。
        loop {
            // 若当前组中还有未处理的满桶位。
            if let Some(index) = self.current_group.next() {
                // 返回的 `self.group_first_index + index` 将始终
                // 处于 `0..self.num_buckets()` 范围内。见下方解释。
                // 返回全局满桶索引。
                return Some(self.group_first_index + index);
            }

            // SAFETY: 此函数的调用者确保：
            //
            // 1. 绝不在取得所有元素之后继续尝试迭代；
            // 2. 表存活且未被移动；
            // 3. 最初的 `self.ctrl` 指向控制字节数组的起始处。
            //
            // 考虑到上述条件，我们始终保持在边界内，因为：
            //
            // 1. 对于小于组宽度的表（self.num_buckets() <= Group::WIDTH），
            //    我们永远不会进入该分支，因为此时我们应该已经产出了表的所有元素。
            //
            // 2. 对于大于组宽度的表。桶数是 2 的幂（2 ^ n），Group::WIDTH 也是 2 的幂（2 ^ k）。
            //    由于 `(2 ^ n) > (2 ^ k)`，因此 `(2 ^ n) % (2 ^ k) = 0`。由于我们从控制字节数组的
            //    起始处开始，且在取得所有元素之后绝不尝试继续迭代，因此最后一个 `self.ctrl`
            //    将等于 `self.num_buckets() - Group::WIDTH`，所以 `self.current_group.next()`
            //    返回的索引始终处于 `0..Group::WIDTH` 范围内，
            //    且后续的 `self.group_first_index + index` 将始终返回小于
            //    `self.num_buckets()` 的数。
            unsafe {
                // 控制字节指针前进一个组宽度。
                self.ctrl = NonNull::new_unchecked(self.ctrl.as_ptr().add(Group::WIDTH));
            }

            // SAFETY: 见上文解释。
            unsafe {
                // 加载下一组控制字节并重建满桶位掩码迭代器。
                self.current_group = Group::load_aligned(self.ctrl.as_ptr().cast())
                    .match_full()
                    .into_iter();
                // 组起始索引前进一个组宽度。
                self.group_first_index += Group::WIDTH;
            }
        }
    }
}

// 为 FullBucketsIndices 实现 Iterator trait。
impl Iterator for FullBucketsIndices {
    // 迭代器产出的元素类型。
    type Item = usize;

    /// 前进迭代器并返回下一个值。由调用者确保 `RawTable` 的寿命长于 `FullBucketsIndices`，
    /// 因为我们无法把 `next` 方法标记为 unsafe。
    #[inline(always)]
    // 前进到下一个满桶索引。
    fn next(&mut self) -> Option<usize> {
        // 若已经产出了所有元素则直接返回。
        if self.items == 0 {
            // 返回 None 表示迭代结束。
            return None;
        }

        // SAFETY:
        // 1. 我们使用 `items` 字段检查待产出的元素数量。
        // 2. 调用者确保表存活且未被移动。
        let nxt = unsafe { self.next_impl() };

        // 调试断言：必须取到了索引。
        debug_assert!(nxt.is_some());
        // 剩余计数减 1。
        self.items -= 1;

        // 返回取到的索引。
        nxt
    }

    #[inline(always)]
    // 返回剩余元素数量的精确提示。
    fn size_hint(&self) -> (usize, Option<usize>) {
        // 剩余数量即 items 的值。
        (self.items, Some(self.items))
    }
}

// 标记 FullBucketsIndices 为精确大小迭代器。
impl ExactSizeIterator for FullBucketsIndices {}
// 标记 FullBucketsIndices 为融合迭代器。
impl FusedIterator for FullBucketsIndices {}

/// 消费表并返回其中元素的迭代器。
pub(crate) struct RawIntoIter<T, A: Allocator = Global> {
    // 内部的原始迭代器。
    iter: RawIter<T>,
    // 保存的内存分配信息（指针、布局、分配器），用于迭代器释放时归还内存。
    allocation: Option<(NonNull<u8>, Layout, A)>,
    // PhantomData 标记，声明我们拥有类型 T 的实例。
    marker: PhantomData<T>,
}

// 为 RawIntoIter 实现辅助方法。
impl<T, A: Allocator> RawIntoIter<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 获取内部迭代器的克隆。
    pub(crate) fn iter(&self) -> RawIter<T> {
        // 克隆内部迭代器。
        self.iter.clone()
    }
}

// RawIntoIter 满足 Send 约束（T 与 A 均需满足）。
unsafe impl<T, A: Allocator> Send for RawIntoIter<T, A>
where
    // T 必须满足 Send。
    T: Send,
    // A 必须满足 Send。
    A: Send,
{
}
// RawIntoIter 满足 Sync 约束（T 与 A 均需满足）。
unsafe impl<T, A: Allocator> Sync for RawIntoIter<T, A>
where
    // T 必须满足 Sync。
    T: Sync,
    // A 必须满足 Sync。
    A: Sync,
{
}

// 仅在 nightly 特性下编译。
#[cfg(feature = "nightly")]
// nightly 下利用 may_dangle 放宽 Drop 检查的变体。
unsafe impl<#[may_dangle] T, A: Allocator> Drop for RawIntoIter<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 析构迭代器：drop 剩余元素并释放表内存。
    fn drop(&mut self) {
        unsafe {
            // Drop 所有剩余元素
            self.iter.drop_elements();

            // 释放表
            if let Some((ptr, layout, ref alloc)) = self.allocation {
                // 通过保存的分配器释放内存。
                alloc.deallocate(ptr, layout);
            }
        }
    }
}
// 非 nightly 编译时的 Drop 实现。
#[cfg(not(feature = "nightly"))]
impl<T, A: Allocator> Drop for RawIntoIter<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 析构迭代器：drop 剩余元素并释放表内存。
    fn drop(&mut self) {
        unsafe {
            // Drop 所有剩余元素
            self.iter.drop_elements();

            // 释放表
            if let Some((ptr, layout, ref alloc)) = self.allocation {
                // 通过保存的分配器释放内存。
                alloc.deallocate(ptr, layout);
            }
        }
    }
}

// 为 RawIntoIter 实现 Default trait。
impl<T, A: Allocator> Default for RawIntoIter<T, A> {
    // 创建默认的空迭代器。
    fn default() -> Self {
        Self {
            // 使用默认的空内部迭代器。
            iter: Default::default(),
            // 没有待释放的分配。
            allocation: None,
            // PhantomData 标记。
            marker: PhantomData,
        }
    }
}
// 为 RawIntoIter 实现 Iterator trait。
impl<T, A: Allocator> Iterator for RawIntoIter<T, A> {
    // 迭代器产出的元素类型。
    type Item = T;

    #[cfg_attr(feature = "inline-more", inline)]
    // 取出下一个元素（按值读取并移出）。
    fn next(&mut self) -> Option<T> {
        unsafe { Some(self.iter.next()?.read()) }
    }

    #[inline]
    // 委托给内部迭代器的数量提示。
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

// 标记 RawIntoIter 为精确大小迭代器。
impl<T, A: Allocator> ExactSizeIterator for RawIntoIter<T, A> {}
// 标记 RawIntoIter 为融合迭代器。
impl<T, A: Allocator> FusedIterator for RawIntoIter<T, A> {}

/// 消费元素但不释放表存储空间的迭代器。
pub(crate) struct RawDrain<'a, T, A: Allocator = Global> {
    // 内部的原始迭代器。
    iter: RawIter<T>,

    // 在抽取期间，表被移动进迭代器中。这确保了若抽取迭代器被泄漏而未被 drop，
    // 留下的会是一个空表。
    table: RawTableInner,
    // 指向原表中该位置的非空指针。
    orig_table: NonNull<RawTableInner>,

    // 我们不使用 &'a mut RawTable<T>，因为我们希望 RawDrain 对 T 是协变的。
    marker: PhantomData<&'a RawTable<T, A>>,
}

// 为 RawDrain 实现辅助方法。
impl<T, A: Allocator> RawDrain<'_, T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 获取内部迭代器的克隆。
    pub(crate) fn iter(&self) -> RawIter<T> {
        // 克隆内部迭代器。
        self.iter.clone()
    }
}

// RawDrain 满足 Send 约束（T 与 A 均需满足）。
unsafe impl<T, A: Allocator> Send for RawDrain<'_, T, A>
where
    // T 必须满足 Send。
    T: Send,
    // A 必须满足 Send。
    A: Send,
{
}
// RawDrain 满足 Sync 约束（T 与 A 均需满足）。
unsafe impl<T, A: Allocator> Sync for RawDrain<'_, T, A>
where
    // T 必须满足 Sync。
    T: Sync,
    // A 必须满足 Sync。
    A: Sync,
{
}

// 为 RawDrain 实现 Drop trait。
impl<T, A: Allocator> Drop for RawDrain<'_, T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 析构抽取迭代器：drop 剩余元素并把空表还回原位置。
    fn drop(&mut self) {
        unsafe {
            // Drop 所有剩余元素。注意这可能会 panic。
            self.iter.drop_elements();

            // 既然所有元素都已被 drop，现在重置表的内容。
            self.table.clear_no_drop();

            // 把现已为空的表移回其原始位置。
            self.orig_table
                .as_ptr()
                .copy_from_nonoverlapping(&raw const self.table, 1);
        }
    }
}

// 为 RawDrain 实现 Iterator trait。
impl<T, A: Allocator> Iterator for RawDrain<'_, T, A> {
    // 迭代器产出的元素类型。
    type Item = T;

    #[cfg_attr(feature = "inline-more", inline)]
    // 取出下一个元素（按值读取并移出）。
    fn next(&mut self) -> Option<T> {
        unsafe {
            // 从内部迭代器取下一个桶。
            let item = self.iter.next()?;
            // 按值读取元素并返回。
            Some(item.read())
        }
    }

    #[inline]
    // 委托给内部迭代器的数量提示。
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

// 标记 RawDrain 为精确大小迭代器。
impl<T, A: Allocator> ExactSizeIterator for RawDrain<'_, T, A> {}
// 标记 RawDrain 为融合迭代器。
impl<T, A: Allocator> FusedIterator for RawDrain<'_, T, A> {}

/// 遍历可能匹配给定哈希的已占用桶的迭代器。
///
/// `RawTable` 只存储哈希值的 7 位，因此此迭代器可能返回哈希值与所提供的值不同的条目。
/// 在使用返回值之前，你应当始终对其进行校验。
///
/// 为了获得最大的灵活性，此迭代器不受生命周期约束，但在使用它时你必须遵守以下几条规则：
/// - 迭代期间不得释放哈希表（包括通过扩容/缩容的方式）。
/// - 擦除迭代器已经产出过的桶是可以的。
/// - 擦除迭代器尚未产出过的桶，仍可能导致迭代器产出该桶。
/// - 迭代器创建之后插入的元素是否会被该迭代器产出是未指定的。
/// - 迭代器产出桶的顺序是未指定的，并且将来可能会改变。
pub(crate) struct RawIterHash<T> {
    // 内部的索引迭代器。
    inner: RawIterHashIndices,
    // PhantomData 标记，携带类型 T。
    _marker: PhantomData<T>,
}

// 派生 Clone，使索引迭代器可克隆。
#[derive(Clone)]
pub(crate) struct RawIterHashIndices {
    // 详见 `RawTableInner` 的对应字段。
    // 我们不能存储 `*const RawTableInner`，因为用户对 `RawTable` 调用 `&mut` 方法时
    // 它会失效。
    bucket_mask: usize,
    // 控制字节指针。
    ctrl: NonNull<u8>,

    // 哈希值的高 7 位。
    tag_hash: Tag,

    // 搜索中要探测的组序列。
    probe_seq: ProbeSeq,

    // 当前正在处理的组。
    group: Group,

    // 组内标签哈希匹配的元素。
    bitmask: BitMaskIter,
}

// 为 RawIterHash 实现构造方法。
impl<T> RawIterHash<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 根据表与哈希值创建哈希迭代器。
    unsafe fn new<A: Allocator>(table: &RawTable<T, A>, hash: u64) -> Self {
        RawIterHash {
            // 基于内部表构造索引迭代器。
            inner: unsafe { RawIterHashIndices::new(&table.table, hash) },
            // PhantomData 标记。
            _marker: PhantomData,
        }
    }
}

// 为 RawIterHash 实现 Clone trait。
impl<T> Clone for RawIterHash<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 克隆迭代器。
    fn clone(&self) -> Self {
        Self {
            // 克隆内部索引迭代器。
            inner: self.inner.clone(),
            // PhantomData 标记。
            _marker: PhantomData,
        }
    }
}

// 为 RawIterHash 实现 Default trait。
impl<T> Default for RawIterHash<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 创建默认的空迭代器。
    fn default() -> Self {
        Self {
            // 使用默认的索引迭代器。
            inner: RawIterHashIndices::default(),
            // PhantomData 标记。
            _marker: PhantomData,
        }
    }
}

// 为 RawIterHashIndices 实现 Default trait。
impl Default for RawIterHashIndices {
    #[cfg_attr(feature = "inline-more", inline)]
    // 创建默认的空索引迭代器。
    fn default() -> Self {
        // SAFETY: 因为该表是静态的，所以它的寿命总是长于迭代器。
        unsafe { RawIterHashIndices::new(&RawTableInner::NEW, 0) }
    }
}

// 为 RawIterHashIndices 实现构造方法。
impl RawIterHashIndices {
    #[cfg_attr(feature = "inline-more", inline)]
    // 根据内部表与哈希值创建索引迭代器。
    unsafe fn new(table: &RawTableInner, hash: u64) -> Self {
        // 取哈希值的高 7 位标签。
        let tag_hash = Tag::full(hash);
        // 根据哈希值创建探测序列。
        let probe_seq = table.probe_seq(hash);
        // 加载探测起始位置的组。
        let group = unsafe { Group::load(table.ctrl(probe_seq.pos)) };
        // 匹配组内标签匹配的元素位并转为迭代器。
        let bitmask = group.match_tag(tag_hash).into_iter();

        RawIterHashIndices {
            // 复制桶掩码。
            bucket_mask: table.bucket_mask,
            // 复制控制字节指针。
            ctrl: table.ctrl,
            // 记录哈希标签。
            tag_hash,
            // 记录探测序列。
            probe_seq,
            // 记录当前组。
            group,
            // 记录匹配位掩码迭代器。
            bitmask,
        }
    }
}

// 为 RawIterHash 实现 Iterator trait。
impl<T> Iterator for RawIterHash<T> {
    // 迭代器产出的元素类型。
    type Item = Bucket<T>;

    // 前进到下一个可能匹配的桶。
    fn next(&mut self) -> Option<Bucket<T>> {
        unsafe {
            // 从索引迭代器取下一个索引。
            match self.inner.next() {
                Some(index) => {
                    // 这里不能使用 `RawTable::bucket`，因为我们没有
                    // 可用的实际 `RawTable` 引用。
                    // 调试断言：索引不超过桶掩码。
                    debug_assert!(index <= self.inner.bucket_mask);
                    // 由控制字节指针与索引构造桶。
                    let bucket = Bucket::from_base_index(self.inner.ctrl.cast(), index);
                    // 返回该桶。
                    Some(bucket)
                }
                // 迭代结束。
                None => None,
            }
        }
    }
}

// 为 RawIterHashIndices 实现 Iterator trait。
impl Iterator for RawIterHashIndices {
    // 迭代器产出的元素类型。
    type Item = usize;

    // 前进到下一个可能匹配的桶索引。
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            // 循环探测各组直到找到匹配或结束。
            loop {
                // 若当前组中还有未处理的匹配位。
                if let Some(bit) = self.bitmask.next() {
                    // 计算该位对应的全局桶索引。
                    let index = (self.probe_seq.pos + bit) & self.bucket_mask;
                    // 返回该索引。
                    return Some(index);
                }
                // 若当前组中有空位，说明探测已覆盖所有可能位置。
                if likely(self.group.match_empty().any_bit_set()) {
                    // 返回 None 表示迭代结束。
                    return None;
                }
                // 将探测序列推进到下一个组。
                self.probe_seq.move_next(self.bucket_mask);

                // 这里不能使用 `RawTableInner::ctrl`，因为我们没有
                // 可用的实际 `RawTableInner` 引用。
                // 取新探测位置。
                let index = self.probe_seq.pos;
                // 调试断言：位置在扩展控制字节范围内。
                debug_assert!(index < self.bucket_mask + 1 + Group::WIDTH);
                // 计算该位置控制字节的指针。
                let group_ctrl = self.ctrl.as_ptr().add(index).cast();

                // 加载新组。
                self.group = Group::load(group_ctrl);
                // 重新匹配标签并更新位掩码迭代器。
                self.bitmask = self.group.match_tag(self.tag_hash).into_iter();
            }
        }
    }
}

// 条件提取迭代器：按谓词筛选并移除元素。
pub(crate) struct RawExtractIf<'a, T, A: Allocator> {
    // 内部的原始迭代器。
    pub iter: RawIter<T>,
    // 对表的共享可变引用。
    pub table: &'a mut RawTable<T, A>,
}

// 为 RawExtractIf 实现核心方法。
impl<T, A: Allocator> RawExtractIf<'_, T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    // 取出下一个满足谓词的元素并从表中移除。
    pub(crate) fn next<F>(&mut self, mut f: F) -> Option<T>
    // 约束：F 是谓词函数。
    where
        // FnMut 约束。
        F: FnMut(&mut T) -> bool,
    {
        unsafe {
            // 遍历剩余元素。
            for item in &mut self.iter {
                // 若元素满足谓词。
                if f(item.as_mut()) {
                    // 从表中移除并按值返回该元素。
                    return Some(self.table.remove(item).0);
                }
            }
        }
        // 没有满足条件的元素。
        None
    }
}
// 仅在测试构建下编译的测试模块。
#[cfg(test)]
mod test_map {
    // 引入父模块的所有符号。
    use super::*;

    #[test]
    // 测试 prev_pow2 函数的正确性。
    fn test_prev_pow2() {
        // 跳过 0，该输入未定义。
        let mut pow2: usize = 1;
        // 从最小的 2 的幂开始逐级翻倍测试。
        while (pow2 << 1) > 0 {
            // 计算下一个 2 的幂。
            let next_pow2 = pow2 << 1;
            // 断言 prev_pow2 对 2 的幂返回其自身。
            assert_eq!(pow2, prev_pow2(pow2));
            // 需要跳过 2，因为它本身也是 2 的幂，
            // 所以 prev_pow2 不会为它返回"前一个"2 的幂。
            if next_pow2 > 2 {
                // 断言对 2 的幂加 1 返回该 2 的幂。
                assert_eq!(pow2, prev_pow2(pow2 + 1));
                // 断言对下一个 2 的幂减 1 返回当前 2 的幂。
                assert_eq!(pow2, prev_pow2(next_pow2 - 1));
            }
            // 前进到下一个 2 的幂。
            pow2 = next_pow2;
        }
    }

    #[test]
    // 测试小类型的最小桶数约束。
    fn test_minimum_capacity_for_small_types() {
        // 标记调用位置，便于 panic 时定位。
        #[track_caller]
        // 泛型辅助函数：检查容量为 1 的表的桶数是否达到最小值。
        fn test_t<T>() {
            // 创建容量为 1 的表。
            let raw_table: RawTable<T> = RawTable::with_capacity(1);
            // 取实际桶数。
            let actual_buckets = raw_table.num_buckets();
            // 计算最小桶数：组宽度除以类型大小。
            let min_buckets = Group::WIDTH / size_of::<T>();
            // 断言实际桶数不小于最小桶数。
            assert!(
                // 比较实际桶数与最小桶数。
                actual_buckets >= min_buckets,
                // 断言失败信息（保持原样的字符串字面量）。
                "expected at least {min_buckets} buckets, got {actual_buckets} buckets"
            );
        }

        // 以 u8 类型测试。
        test_t::<u8>();

        // 这只对某些平台是"小"类型，例如带 SSE2 的 x86_64，
        // 但在其他平台上运行也没有坏处。
        // 以 u16 类型测试。
        test_t::<u16>();
    }

    // 测试辅助函数：对给定表执行原地重新哈希。
    fn rehash_in_place<T>(table: &mut RawTable<T>, hasher: impl Fn(&T) -> u64) {
        unsafe {
            // 调用内部表的原地重哈希。
            table.table.rehash_in_place(
                // 传入哈希函数包装（按桶索引取元素再计算哈希）。
                &|table, index| hasher(table.bucket::<T>(index).as_ref()),
                // 元素大小。
                size_of::<T>(),
                // 若类型需要 drop，则传入 drop 函数。
                if mem::needs_drop::<T>() {
                    Some(|ptr| ptr::drop_in_place(ptr.cast::<T>()))
                } else {
                    None
                },
            );
        }
    }

    #[test]
    // 测试原地重新哈希后元素仍能找到。
    fn rehash() {
        // 创建一个新的空表。
        let mut table = RawTable::new();
        // 定义哈希函数：直接返回元素本身的值。
        let hasher = |i: &u64| *i;
        // 插入 0..100 的元素。
        for i in 0..100 {
            // 插入键值对（键值相同）。
            table.insert(i, i, hasher);
        }

        // 验证重哈希前所有元素都能找到。
        for i in 0..100 {
            unsafe {
                // 断言能找到元素 i。
                assert_eq!(table.find(i, |x| *x == i).map(|b| b.read()), Some(i));
            }
            // 断言找不到元素 i+100。
            assert!(table.find(i + 100, |x| *x == i + 100).is_none());
        }

        // 执行原地重新哈希。
        rehash_in_place(&mut table, hasher);

        // 验证重哈希后所有元素仍能找到。
        for i in 0..100 {
            unsafe {
                // 断言能找到元素 i。
                assert_eq!(table.find(i, |x| *x == i).map(|b| b.read()), Some(i));
            }
            // 断言找不到元素 i+100。
            assert!(table.find(i + 100, |x| *x == i + 100).is_none());
        }
    }

    /// 检查我们在 drop 期间不会尝试读取
    /// 未初始化表的内存
    #[test]
    // 测试 drop 未初始化的表不会出错。
    fn test_drop_uninitialized() {
        // 引入向量类型。
        use stdalloc::vec::Vec;

        // 创建未初始化的表。
        let table = unsafe {
            // SAFETY: `buckets` 是 2 的幂，而且我们
            // 并不打算实际使用返回的 RawTable。
            RawTable::<(u64, Vec<i32>)>::new_uninitialized(Global, 8, Fallibility::Infallible)
                // 解包结果。
                .unwrap()
        };
        // 直接 drop 该未初始化的表。
        drop(table);
    }

    /// 检查当 `ITEMS` 为零时，即使我们拥有 `FULL` 控制字节，
    /// 也不会尝试 drop 数据
    #[test]
    // 测试元素数为零（但控制字节为 FULL）时 drop 不析构数据。
    fn test_drop_zero_items() {
        // 引入向量类型。
        use stdalloc::vec::Vec;
        unsafe {
            // SAFETY: `buckets` 是 2 的幂，而且我们
            // 并不打算实际使用返回的 RawTable。
            // 创建未初始化的表。
            let mut table =
                RawTable::<(u64, Vec<i32>)>::new_uninitialized(Global, 8, Fallibility::Infallible)
                    // 解包结果。
                    .unwrap();

            // 我们模拟一个（看似）已满的表。

            // SAFETY: 我们已检查表已分配，因此表已有
            // `self.bucket_mask + 1 + Group::WIDTH` 个控制字节（见 TableLayout::calculate_layout_for），
            // 所以写入 `table.table.num_ctrl_bytes() == bucket_mask + 1 + Group::WIDTH` 个字节是安全的。
            table.table.ctrl_slice().fill_empty();

            // SAFETY: table.capacity() 保证小于 table.num_buckets()
            table.table.ctrl(0).write_bytes(0, table.capacity());

            // 修正尾部的控制字节。对于小于组宽度的表的处理方式，见 set_ctrl 中的注释。
            // 若桶数小于组宽度（小表的特殊情况）。
            if table.num_buckets() < Group::WIDTH {
                // SAFETY: 我们有 `self.bucket_mask + 1 + Group::WIDTH` 个控制字节，
                // 因此以 `Group::WIDTH` 为偏移复制 `self.num_buckets() == self.bucket_mask + 1` 个字节是安全的。
                table
                    // 内部表。
                    .table
                    // 取第 0 个控制字节指针。
                    .ctrl(0)
                    // 复制全部桶数个字节到组宽度处。
                    .copy_to(table.table.ctrl(Group::WIDTH), table.table.num_buckets());
            } else {
                // SAFETY: 我们有 `self.bucket_mask + 1 + Group::WIDTH` 个控制字节，
                // 因此以 `self.num_buckets() == self.bucket_mask + 1` 为偏移复制 `Group::WIDTH` 个字节是安全的。
                table
                    // 内部表。
                    .table
                    // 取第 0 个控制字节指针。
                    .ctrl(0)
                    // 复制组宽度个字节到表末尾的重复区域。
                    .copy_to(table.table.ctrl(table.table.num_buckets()), Group::WIDTH);
            }
            // drop 该表，验证不会析构任何数据。
            drop(table);
        }
    }

    /// 检查当 `ITEMS` 为零时，即使我们拥有 `FULL` 控制字节，
    /// 也不会尝试 drop 数据
    #[test]
    // 仅在 panic 支持展开（unwind）时编译。
    #[cfg(panic = "unwind")]
    // 测试 clone_from 过程中 clone panic 的安全性。
    fn test_catch_panic_clone_from() {
        // 引入父模块的分配器相关类型。
        use super::{AllocError, Allocator, Global};
        // 引入原子类型与内存序。
        use core::sync::atomic::{AtomicI8, Ordering};
        // 引入线程工具。
        use std::thread;
        // 引入原子引用计数。
        use stdalloc::sync::Arc;
        // 引入向量类型。
        use stdalloc::vec::Vec;

        // 自定义分配器的内部状态。
        struct MyAllocInner {
            // 记录分配器 drop 次数的原子计数器。
            drop_count: Arc<AtomicI8>,
        }

        // 派生 Clone，使分配器可克隆。
        #[derive(Clone)]
        // 自定义分配器。
        struct MyAlloc {
            // 持有内部状态。
            _inner: Arc<MyAllocInner>,
        }

        // 为内部状态实现 Drop trait。
        impl Drop for MyAllocInner {
            // drop 时减少计数器并打印信息。
            fn drop(&mut self) {
                // 打印释放信息（保持原样的字符串字面量）。
                println!("MyAlloc freed.");
                // 计数器减 1。
                self.drop_count.fetch_sub(1, Ordering::SeqCst);
            }
        }

        // 为自定义分配器实现 Allocator trait。
        unsafe impl Allocator for MyAlloc {
            // 分配内存：委托给全局分配器。
            fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
                // 使用全局分配器。
                let g = Global;
                // 委托分配。
                g.allocate(layout)
            }

            // 释放内存：委托给全局分配器。
            unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
                unsafe {
                    // 使用全局分配器。
                    let g = Global;
                    // 委托释放。
                    g.deallocate(ptr, layout);
                }
            }
        }

        // 常量：未布防（clone 不 panic）。
        const DISARMED: bool = false;
        // 常量：布防（clone 会 panic）。
        const ARMED: bool = true;

        // 测试用元素类型：clone 可 panic，drop 检查双重释放。
        struct CheckedCloneDrop {
            // 是否在 clone 时 panic。
            panic_in_clone: bool,
            // 是否已被 drop。
            dropped: bool,
            // 需要 drop 的数据。
            need_drop: Vec<u64>,
        }

        // 为测试元素实现 Clone trait。
        impl Clone for CheckedCloneDrop {
            // 克隆：若布防则 panic，否则克隆所有字段。
            fn clone(&self) -> Self {
                // 若布防则触发 panic（保持原样的字符串字面量）。
                assert!(!self.panic_in_clone, "panic in clone");
                // 构造克隆体。
                Self {
                    // 复制 panic 标志。
                    panic_in_clone: self.panic_in_clone,
                    // 复制 drop 标志。
                    dropped: self.dropped,
                    // 克隆需 drop 的数据。
                    need_drop: self.need_drop.clone(),
                }
            }
        }

        // 为测试元素实现 Drop trait。
        impl Drop for CheckedCloneDrop {
            // drop：检查没有双重释放。
            fn drop(&mut self) {
                // 若已 drop 过则触发 panic（保持原样的字符串字面量）。
                assert!(!self.dropped, "double drop");
                // 标记为已 drop。
                self.dropped = true;
            }
        }

        // 创建原子计数器，初始值为 2。
        let dropped: Arc<AtomicI8> = Arc::new(AtomicI8::new(2));

        // 使用自定义分配器创建空表。
        let mut table = RawTable::new_in(MyAlloc {
            // 用共享计数器构造内部状态。
            _inner: Arc::new(MyAllocInner {
                // 克隆计数器供内部使用。
                drop_count: dropped.clone(),
            }),
        });

        // 插入 7 个未布防的元素。
        for (idx, panic_in_clone) in core::iter::repeat_n(DISARMED, 7).enumerate() {
            // 索引转为 u64 作为键。
            let idx = idx as u64;
            // 插入键值对。
            table.insert(
                // 哈希。
                idx,
                // 值：键与测试元素组成的元组。
                (
                    // 键。
                    idx,
                    // 测试元素。
                    CheckedCloneDrop {
                        // panic 标志。
                        panic_in_clone,
                        // 尚未 drop。
                        dropped: false,
                        // 需 drop 的数据。
                        need_drop: vec![idx],
                    },
                ),
                // 哈希函数：取元组第一个分量。
                |(k, _)| *k,
            );
        }

        // 断言表中有 7 个元素。
        assert_eq!(table.len(), 7);

        // 在作用域线程中触发 clone panic。
        thread::scope(|s| {
            // 生成子线程执行测试。
            let result = s.spawn(|| {
                // 布防标志数组：第 3 个元素布防。
                let armed_flags = [
                    DISARMED, DISARMED, ARMED, DISARMED, DISARMED, DISARMED, DISARMED,
                ];
                // 在线程内用自定义分配器创建空表。
                let mut scope_table = RawTable::new_in(MyAlloc {
                    // 用共享计数器构造内部状态。
                    _inner: Arc::new(MyAllocInner {
                        // 克隆计数器供内部使用。
                        drop_count: dropped.clone(),
                    }),
                });
                // 按布防标志插入 7 个元素。
                for (idx, &panic_in_clone) in armed_flags.iter().enumerate() {
                    // 索引转为 u64 作为键。
                    let idx = idx as u64;
                    // 插入键值对。
                    scope_table.insert(
                        // 哈希。
                        idx,
                        // 值：键与测试元素组成的元组。
                        (
                            // 键。
                            idx,
                            // 测试元素。
                            CheckedCloneDrop {
                                // panic 标志。
                                panic_in_clone,
                                // 尚未 drop。
                                dropped: false,
                                // 需 drop 的数据。
                                need_drop: vec![idx + 100],
                            },
                        ),
                        // 哈希函数：取元组第一个分量。
                        |(k, _)| *k,
                    );
                }
                // 从线程表克隆到外层表，期间第 3 个元素的 clone 会 panic。
                table.clone_from(&scope_table);
            });
            // 断言子线程因 panic 而退出。
            assert!(result.join().is_err());
        });

        // 让我们检查所有迭代器都能正常工作并且不返回任何元素
        // （尤其是 `RawIterRange`，它不依赖表中元素的数量，
        // 而是直接查看控制字节）
        //
        // SAFETY: 我们确信 `RawTable` 的寿命长于
        // 返回的 `RawIter / RawIterRange` 迭代器。
        // 断言表已清空（长度为 0）。
        assert_eq!(table.len(), 0);
        // 断言迭代器不产出任何元素。
        assert_eq!(unsafe { table.iter().count() }, 0);
        // 断言内部范围迭代器也不产出任何元素。
        assert_eq!(unsafe { table.iter().iter.count() }, 0);

        // 遍历所有桶索引，确认表中没有任何元素。
        for idx in 0..table.num_buckets() {
            // 索引转为 u64 作为键。
            let idx = idx as u64;
            // 断言找不到该键。
            assert!(
                // 查找键。
                table.find(idx, |(k, _)| *k == idx).is_none(),
                // 断言失败信息（保持原样的字符串字面量）。
                "Index: {idx}"
            );
        }

        // 所有分配器的克隆都应该已被 drop。
        // 断言计数器为 1（只剩外层的这一个）。
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }
}
