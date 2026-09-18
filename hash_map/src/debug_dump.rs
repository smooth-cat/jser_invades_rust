//! [学习用] HashMap 堆内存可视化工具。
//!
//! HashMap 在堆上申请的是**一整块连续内存**，布局如下（见 `RawTableInner`）：
//!
//! ```text
//! [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, CTa_0, ..., CTa_m
//!                              ^ ctrl 指针（数据区结束 = 控制区开始）
//! ```
//!
//! - **数据区**：`buckets` 个 `(K, V)`，从高地址往低地址排，桶 0 紧挨着 ctrl 区；
//! - **控制区**：每个桶对应 1 字节控制字节 `ctrl[i]`，描述桶 i 的状态：
//!   * `0xFF` EMPTY   —— 空桶，从未存过数据；
//!   * `0x80` DELETED —— 墓碑（tombstone），存过但被删除，防止探测提前断链；
//!   * `0x00..=0x7F` FULL —— 有数据！值就是该桶元素 hash 的**高 7 位**（h2），
//!     SIMD 查找时用「一整组字节 × repeat(h2)」并行比对快速筛出候选桶；
//! - **镜像区**：末尾 `Group::WIDTH` 个字节是 `ctrl[0..WIDTH]` 的副本，
//!   供 SIMD 按组读取时安全越界。
//!
//! 用法：在 `insert` / `remove` 之后调用，观察每一步内存的变化：
//!
//! ```ignore
//! println!("{}", map.debug_dump("insert('a', 10)"));
//! ```

use core::fmt::{Debug, Write as _};
use core::hash::{BuildHasher, Hash};
use core::mem::size_of;
use core::slice;

use stdalloc::borrow::ToOwned;
use stdalloc::format;
use stdalloc::string::String;
use stdalloc::vec::Vec;

use crate::alloc::Allocator;
use crate::control::Group;
use crate::map::{HashMap, make_hash};

/// 空桶（EMPTY）的控制字节值。
const EMPTY: u8 = 0xFF;
/// 墓碑（DELETED）的控制字节值。
const DELETED: u8 = 0x80;
/// 表头与内容行共用的区块分隔符（两行逐字符一致，保证对齐不受终端渲染影响）。
const SEPARATOR: &str = " → ";

/// 每个 FULL 桶的详情：桶下标、控制字节、(K, V) 文本、完整 hash、home 桶。
type FullDetail = (usize, u8, String, u64, usize);

impl<K, V, S, A> HashMap<K, V, S, A>
where
  K: Hash + Debug,
  V: Debug,
  S: BuildHasher,
  A: Allocator,
{
  /// 把表在堆上的**桶数据 + 控制字节**转成一段一眼能看懂的文本。
  ///
  /// 只读内存，不会修改表的状态，可以在任意 `insert` / `remove` 之后调用。
  /// 参数 `note` 用于标注当前快照对应哪一步操作，例如 `insert('a', 10)`。
  ///
  /// 数据区格的显示：FULL 桶打印有效值 `(K, V)`；EMPTY / 墓碑槽**按字节**
  /// 打印堆内存的真实情况（未初始化 / 已析构残留），详见下方 slot_hex 的说明。
  pub fn debug_dump(&self, note: &str) {
    let mut out = String::new();

    let buckets = self.table.num_buckets();
    let items = self.table.len();
    let capacity = self.table.capacity();
    let growth_left = capacity - items;
    let width = Group::WIDTH;

    // 表头：这一步操作后的元信息。
    let _ = writeln!(
      out,
      "╔════════════════════════════════════════════════════════════╗"
    );
    let _ = writeln!(out, "║ HashMap 堆内存快照  操作: {note}");
    let _ = writeln!(
      out,
      "╚════════════════════════════════════════════════════════════╝"
    );

    // 空表单例：内存还没分配，ctrl 是悬垂指针，指向的是静态 EMPTY 组。
    if buckets <= 1 && items == 0 {
      let _ = writeln!(
        out,
        "buckets=1(虚拟), items=0, 堆内存【未分配】—— ctrl 悬垂指针指向静态 EMPTY 组"
      );
      let _ = writeln!(out);
      print_legend(&mut out, width);
      println!("{out}");
    }

    let mask = buckets - 1;
    let item_size = size_of::<(K, V)>();
    let alloc_size = self.table.allocation_size();
    // 数据区字节数向上取整到 ctrl_align 后才是控制区起点。
    let ctrl_offset = alloc_size - (buckets + width);
    // SAFETY: 表已分配，data_end 指向控制区起点（也就是数据区末尾）。
    let ctrl_ptr = self.table.data_end().as_ptr().cast::<u8>() as usize;
    let data_start = ctrl_ptr - ctrl_offset;

    let _ = writeln!(
      out,
      "buckets(桶数)={buckets}  items(元素)={items}  growth_left(还能插)={growth_left}  \
             capacity(上限)={capacity}"
    );
    let _ = writeln!(
      out,
      "bucket_mask={mask}  Group::WIDTH={width}  每桶={item_size}B  堆块={alloc_size}B"
    );
    let _ = writeln!(
      out,
      "堆地址: 数据区 [{:#x} ~ {:#x})  ctrl(控制区起点)={:#x}",
      data_start, ctrl_ptr, ctrl_ptr
    );
    let _ = writeln!(out);

    // ─── 一条线：数据区(真实地址序=桶号倒序) → ctrl 区(左→右 ctrl[0..buckets]) → 镜像区 ───
    // 三个区块分别构建，方便表头按各自显示宽度补齐，做到完美对齐。
    let mut data_cells = String::new();
    let mut details: Vec<FullDetail> = Vec::new();

    // 数据区：槽位 s 的真实地址 = data_start + s × 桶大小，里面住着桶号 buckets-1-s。
    // 读取桶 i 数据区的原始字节（十六进制，空格分隔）。
    // SAFETY: 表已分配，i < buckets。
    // 注意：EMPTY 槽是未初始化内存、DELETED 槽是已析构元素的残留字节——本工具按
    // 【字节】读取并打印它们，仅供学习观察真实的堆内存情况，只对无堆资源的类型
    // （如 (char, i32)）严格安全；若元素含堆指针，残留字节属于悬垂数据，
    // 不得当作有效值使用（full 桶仍按 (K, V) 有效值打印）。
    // 全零字节视为"空"，省略打印。
    let slot_hex = |i: usize| -> String {
      if item_size == 0 {
        return String::new();
      }
      // SAFETY: 槽位起始地址有效，共 item_size 字节；按字节读取未初始化/残留内存
      // 仅用于演示打印（见上方说明）。
      let bytes =
        unsafe { slice::from_raw_parts(self.table.bucket(i).as_ptr().cast::<u8>(), item_size) };
      if bytes.iter().all(|b| *b == 0) {
        return String::new();
      }
      let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
      hex.join(" ")
    };
    // 拼接状态标签 + 真实字节（无字节可显示时只打印标签）。
    let with_hex = |label: &str, i: usize| -> String {
      let hex = slot_hex(i);
      if hex.is_empty() {
        label.to_owned()
      } else {
        format!("{label}|{hex}")
      }
    };

    for s in 0..buckets {
      let i = buckets - 1 - s;
      // SAFETY: 表已分配，i < buckets < buckets + WIDTH，控制字节已初始化。
      let byte = unsafe { *(ctrl_ptr as *const u8).add(i) };
      let content = if byte == EMPTY {
        // 空桶：数据区从未写入（未初始化），直接按字节打印真实情况。
        with_hex("空", i)
      } else if byte == DELETED {
        // 墓碑：数据已 drop，字节是残留位模式，按字节打印真实情况。
        with_hex("墓碑", i)
      } else if byte & 0x80 == 0 {
        // FULL：桶是满的，数据已初始化，可以安全读出真实 (K, V)。
        // SAFETY: i < buckets；控制字节为 FULL ⇒ 桶内数据已初始化。
        let kv = unsafe { self.table.bucket(i).as_ref() };
        let hash = make_hash(&self.hash_builder, &kv.0);
        let home = (hash as usize) & mask;
        details.push((i, byte, format!("{kv:?}"), hash, home));
        format!("{kv:?}")
      } else {
        "??".to_owned()
      };
      let _ = write!(data_cells, "[桶{i}:{content}]");
    }

    // 控制区：左→右就是真实地址顺序 ctrl[0] → ctrl[buckets-1]，ctrl[i] 描述桶 i。
    let mut ctrl_cells = String::new();
    for i in 0..buckets {
      // SAFETY: 表已分配，i < buckets，控制字节已初始化。
      let byte = unsafe { *(ctrl_ptr as *const u8).add(i) };
      if byte == DELETED {
        let _ = write!(ctrl_cells, "[{:02X}(墓)]", byte);
      } else {
        let _ = write!(ctrl_cells, "[{:02X}]", byte);
      }
    }

    // 镜像区：SIMD 按组读取会越界，所以把开头 WIDTH 个控制字节复制到了尾部。
    let mut mirror_cells = String::from("[");
    for j in 0..width {
      // SAFETY: 镜像区共 WIDTH 字节，j < WIDTH，已初始化。
      let byte = unsafe { *(ctrl_ptr as *const u8).add(buckets + j) };
      let _ = write!(mirror_cells, "{:02X} ", byte);
    }
    // 收掉末尾空格并闭合中括号。
    mirror_cells.pop();
    mirror_cells.push(']');

    // 表头：标签按各区块的显示宽度补齐空格，分隔符与内容行完全相同 → 逐字符对齐。
    let mut header = String::new();
    header.push_str(&pad("数据区", display_width(&data_cells)));
    header.push_str(SEPARATOR);
    header.push_str(&pad("ctrl区", display_width(&ctrl_cells)));
    header.push_str(SEPARATOR);
    header.push_str("镜像区");
    let _ = writeln!(out, "─── 堆内存真实排列 (低地址 → 高地址) ───");
    let _ = writeln!(out, "{header}");
    let _ = writeln!(
      out,
      "{data_cells}{SEPARATOR}{ctrl_cells}{SEPARATOR}{mirror_cells}"
    );
    let _ = writeln!(out);

    // FULL 桶 hash 详情：控制字节里存的 h2 到底对不对应这个元素。
    if !details.is_empty() {
      details.sort_by_key(|d| d.0);
      let _ = writeln!(out, "─── FULL 桶 hash 详情 ───");
      for (i, byte, kv, hash, home) in &details {
        let hint = if home == i {
          "正好在 home 桶"
        } else {
          "home 被占 → 探测顺延"
        };
        let _ = writeln!(
          out,
          "桶[{i}] {kv}: hash={hash:#018x}  ctrl存的高7位 h2={:#04x}  home={home} ({hint})",
          byte & 0x7F
        );
      }
      let _ = writeln!(out);
    }

    print_legend(&mut out, width);
    println!("{out}");
  }
}

/// 图例：解释控制字节的含义与探测规则。
fn print_legend(out: &mut String, width: usize) {
  let _ = writeln!(out, "─── 图例 ───");
  let _ = writeln!(
    out,
    "  数据区格: FULL 打印有效值 (K,V); EMPTY/墓碑槽按【真实字节】打印堆内存(全零省略)"
  );
  let _ = writeln!(
    out,
    "    (EMPTY = 未初始化内存; 墓碑 = 已析构的残留字节, 仅供观察)"
  );
  let _ = writeln!(out, "  ctrl[i] 是 1 字节控制字节, 描述桶 i:");
  let _ = writeln!(
    out,
    "    0xFF EMPTY     空桶: 从未存过数据, 探测遇到它就可以停"
  );
  let _ = writeln!(
    out,
    "    0x80 DELETED   墓碑: 存过后被删, 数据已 drop; 只有前后连续 FULL ≥ {width} 才留墓碑, 否则清成 EMPTY"
  );
  let _ = writeln!(
    out,
    "    0x00~0x7F FULL 有数据: 值 = 该桶元素 hash 的高 7 位(h2), SIMD 用它一次并行筛一组"
  );
  let _ = writeln!(
    out,
    "  定位: home = hash低位 & bucket_mask, 桶被占则按三角数列 1,3,6,10... 跳组顺延"
  );
}

/// 计算字符串在终端里的显示宽度（ASCII 按 1 列，其余按 2 列）。
fn display_width(s: &str) -> usize {
  s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

/// 按显示宽度给标签补齐空格，使表头标签与其下方区块的内容列对齐。
fn pad(label: &str, width: usize) -> String {
  let mut label = label.to_owned();
  for _ in display_width(&label)..width {
    label.push(' ');
  }
  label
}
