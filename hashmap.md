## new

1. HashMap::default()

   1. 根据 with_hasher_in 两个参数分别调用 

      1. DefaultHashBuilder::default() 生成一个 `{ inner: RandomState }`
         1. 创建 **实例级 / 每个 `RandomState`** per_hasher_seed
         2. 创建 **进程级 / 全局级别** global_seed
      2. Global::default() 实际上调用的就是一个空
         因为 \#[derive(Default)] 实际上把每个字段生成为 xxx: Xxx::default()
         Global 没有任何字段

   2.  调用 with_hasher_in 并返回

      1. hash_builder

      2. `table: RawTable::new_in(alloc)`

         ```rust
         // 包含以下字段
         Self {
             // 内部表使用预定义的空状态。
             table: RawTableInner::NEW,
             // 保存传入的分配器。
             alloc,
             // PhantomData 标记，表示拥有 T。
             marker: PhantomData,
         }
         ```

      3. `RawTableInner`

         ```rust
         // 包含以下字段
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
         ```

最终产生的 HashMap 结构
```js
{
  // hash 种子
  // 具有 build_hasher() 来生成一个具有 Hasher 特征的结构体
  // key 的 Hash 特征 决定如何通过 Hasher 来实现 hash 算法
  hash_builder: {
    inner: {
      // 单个 Map 的 hash 种子
      per_hasher_seed: u64,
      // 全局的 hash 种子，包含获取全局种子的 get 方法
			global_seed: GlobalSeed,
		}
  },
  // 包含 表 和 内存分配器
  table: {
    // 真正 table 相关的字段
    table: {
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
    },
    // 内存分配器。
    alloc,
  }  
}
```

## Insert

1. 内存再分配/扩容 reserve  (可能会做)
   1. additional > self.table.growth_left 需要扩容
      1. 再分配 reserve_rehash ~ reserve_rehash_inner 
         1. 获取 new_items = items + 1
         2. 获取当前存储容量上限 full_capacity = bucket_mask_to_capacity
            1. 桶数不超过 8，就是 bucket_mask，即  n - 1
            2. 反之采用 `n * 7 / 8` 即上限为桶数的  `7/8`
         3. 如果 new_items 没容量一半，采用原地重 hash 清理墓碑
            1. 
         4. 反之，扩容 `resize_inner(alloc, usize::max(new_items, full_capacity + 1),`
            这里取现有项和容量上限+1 的最大值
            1. 准备新表 prepare_resize 
               1. 初始化新表 RawTableInner::fallible_with_capacity
                  1. 根据 capacity 计算桶数  capacity_to_buckets
                  2. 创建新表 RawTableInner::new_uninitialized
                     1. 计算申请大小 Layout 和 ctrl， table_layout,calculate_layout_for(buckets)
                        1. table_layout在创建时就确认，搜 table: RawTable< 可找到
                           size 是 是(K, V) 的大小，
                           ctrl_align 取 layout 和 Group.WIDTH 最大值
                        2. Group.WIDTH 是系统可用于分组逻辑运算的宽度，
                           通常是 128bit 64bit, mac 是 64bit,
                        3. 计算 ctrl =`(size * 桶数 + ctrl_align - 1) & !(ctrl_align - 1)`
                           本质是对 `size * 桶数` 向上取整到 ctrl_align 的倍数
                        4. 总大小 len = `ctrl_offset + 桶数 + Group::WIDTH`
                           一个控制项正好是 1 字节；Group::WIDTH 用于避免探测时越界
                        5. 返回 Layout::from_size_align_unchecked(len, ctrl_align) 布局
                     2. 申请内存 do_alloc ->  alloc.allocate(layout)
                     3. 是否分配到更大空间
                        1. 是，重新计算得到的空间可以放多少桶
                           重新计算 ctrl
                        2. 否，啥也不做
                     4. 将空间转为 u8 指针
                     5. 根据现有内容返回新表
                  3. 给新表 ctrl 填充 Empty(1111_1111) result.ctrl_slice().fill_empty(); 
                  4. 返回新表
               2. 返回一个具有释放函数的新表
            2. TODO: 将旧表中的 数据，重新 hash 放新表对应位置
            3. 交换新旧表 `mem::swap(self, &mut new_table);`
               这里利用 Deref 机制将 Scope 内部的表与 self 交换
               这样 Scope 释放时内部释放对象就变为了旧表
   2. 否则不需要，什么都不做
2. 查找元素 find_or_find_insert_index_inner
   1. 获取 hash 高 7 位(h2)Tag::full
   2. 获取探测序列
      1. 获取初始位置 `hash % 桶数`，`这里会用 hash & self.bucket_mask` 效果一样
      2. 初始步长 0

   3. 开始循环
      1. 从 probe_seq.pos 取 group
         1. 通过 crtl(pos) 获取 `*mut Tag` 裸指针
         2. 通过 Group::load `*mut Tag` **从低到高 读一组 Tag 出来**，
            mac 是 64bit, x86是 128bit

      2. Group 与 `repeat(h2)` 进行比对
         1. 默认通过 hasZero 算法 `(cmp - 0x01_01...) & !cmp & 0x80_80...`
            初筛可能为 0 的字节，同时能确定该组具有至少一个 0 字节
         2. **但是在 mac 即 neon 指令集下可以按字节匹配 group 与 repeat(h2)**
            **匹配的字节为 FF 不匹配的为 00**
      3. 最后通过 BITMASK_ITER_MASK(不同平台有区别)，
         最终达到的效果是：
         **匹配字节为 80，不匹配字节为 00**
      4. BitMaskIter.next 每次取最低的 具有 80 的字节的 index (低到高)
         1. index 表示的就是第 n 个匹配上的桶
         2. index 处桶中 key 与当前 key 是否匹配， 匹配则直接
            **return Ok(index)**
      5. 接下来就是没找到
      6. 尝试从 Group 中寻找可插入的槽位 find_insert_index_in_group
         1. `Group & 0x80_80...` 
         2.  找第一个空桶或墓碑位置 `bit = lowest_set_bit()`
            1. insert_index = Some(pos + bit)
            2. insert_index = None

      7. 已找到 insert_index
         1. **当前组 有空桶(代表没有存该数据)，则 return Err(fix_insert_index(insert_index))**
      
            1. 修复桶数不足一组的情况 fix_insert_index
      
               1. 由于表会在控制字节最后填充一组 Empty 字节
      
               2. 有如下情况
                  ```rust
                  [KV1，空，空，KV2][空，空，空，空，空，空，空，空]
                               👆🏻  👆🏻
                              [ 从 KV2 开始探测            ]
                              [ 填充的第一个空桶被误解认为是  ]
                              [ 模后 KV1 位置被当成插入位置  ]
                  ```
      
               3. 也就是说满桶被误认为了空桶
      
               4. 这时需要从 KV1 开始重新探测，找到插入位置
      
               5. 而且必定能找到插入位置，因为 bucket_mask 保证真正存储的数据比申请的桶数少
      
         2. 无空桶，继续按组查找，有可能在后面组发现目标
      
      8. 跳转到下一组  move_next
      
         1. 步长 + 1组的宽度
         2. pos += 步长
         3. 所以每跳一下，跨度就多一组，第一次跳时跳1组(即跳到邻组)

3.  find_or_find_insert_index 

   1. Ok(index) => Ok(桶)
   2. Err(index) => Err(桶)

4. 根据最终结果执行：

   1. Ok 使用 mem::replace 替换原来桶中的 value，**返回 Some(V)**
   2. Err
      1. insert_tagged_at_index
         1. 修改 table.items、table.growth_left
         2. 写入控制字节 h2 set_ctrl(index, new_ctrl)
         3. 写入键值对 bucket.write(KV)

      2. **return None**


## 三角数列遍历 单位(组)

#### 证明三角数列在 n 次遍历内不出现重复

假设 a < b < n 

从第 a 次到第 b次 后发生了碰撞

第 a 次的索引: `start+1+2...+a`

第 b 次的索引: `start+1+2...+a ... +b`

两次之间差了 `a+1 + a+2 + ... + b` 组

差了: `(a+1+b)/2 * (b−a)` 等差数列求和

第 a 次 和 第 b 次 发生了碰撞，意味着

`(a+b+1)(b−a)/2` 是 n 的倍数

`(a+b+1)(b−a)` 是 2n 的倍数

又因为 `(a+b+1) - (b-a) = 2a + 1` 是奇数

也就是说这两个因子，一个是偶数一个是奇数

只有偶数因子是 2n 的倍数

1. `b-a` 是偶数因子
   那么 `b-a` 是 2n 的倍数，才能发生碰撞
   但是 `a < b < n`, 所以在  n 次遍历内不会发生碰撞

2. `a+b+1` 是偶数因子

   那么 `a+b+1` 是 2n 的倍数，才能发生碰撞

   但是 `a < b < n`, 所以在  n 次遍历内不会发生碰撞

## get



## 主体逻辑

1. 以键值对为一个桶存在内存中
2. 基于key字节码，生成 一一对应 的数字，把它当做 kv对 下标
3. hash 将 key 处理成 u64, 
   1. 当大集合映射小集合时，hash碰撞
   2. 2^64 个 桶太大了【主要矛盾】
4. 采用按需分配内存，按 4、8、16... 2^n 进行内存分配
5. 让碰撞更猛烈，  容积 对 hash 取余 这样所有数据就能落在按需分配的容器里了
6. 解决碰撞：向后顺延，假设当前位置有值且不是要插入的key，就向后顺延
7. 




## 第三方包补充

foldhash seed.rs
```rust
pub(crate) fn gen_per_hasher_seed() -> u64 {
    // 我们使用栈指针来初始化每个 hasher 的种子，以确保
    // 不同线程拥有不同的种子；额外的好处是，
    // 栈地址随机化会给我们带来进一次的非确定性。
    // 局部变量，其地址将作为初始熵源。
    let mut per_hasher_seed = 0;
    // 获取局部变量的栈地址，并将其转换为 u64。
    let stack_ptr = core::ptr::addr_of!(per_hasher_seed) as u64;
    // 用栈地址初始化种子。
    per_hasher_seed = stack_ptr;

    // 如果标准库可用，我们使用线程局部状态来确保
    // RandomState 大概率不同，即使调用栈相同也是如此。
    #[cfg(feature = "std")]
    {
        // 引入 Cell，用于线程局部可变状态。
        use std::cell::Cell;
        thread_local! {
            // 每个线程独立的非确定性状态，初始值为 0。
            static PER_HASHER_NONDETERMINISM: Cell<u64> = const { Cell::new(0) };
        }

        // 读取并更新当前线程的非确定性状态。
        PER_HASHER_NONDETERMINISM.with(|cell| {
            // 读取该线程上一次保存的值。
            let nondeterminism = cell.get();
            // 将栈地址与线程局部非确定性状态混合。
            per_hasher_seed = folded_multiply(per_hasher_seed, ARBITRARY1 ^ nondeterminism);
            // 保存新的状态，供该线程后续调用使用。
            cell.set(per_hasher_seed);
        })
    };

    // 如果没有标准库，我们就使用全局原子变量，而不是线程局部状态。
    //
    // PER_HASHER_NONDETERMINISM 的加载和更新存在竞态，
    // 但在实践中没关系——两个不同线程不可能拥有相同的栈位置，
    // 所以它们几乎必然会生成不同的种子，并为
    // PER_HASHER_NONDETERMINISM 提供不同的可能更新。
    // 如果我们使用正确的 fetch_add 原子更新，
    // 那么出现严重竞争的可能性会更大。
    //
    // 为了获得最好的平台支持，我们使用 usize 而不是 64 位原子。
    #[cfg(not(feature = "std"))]
    {
        // 引入原子类型和内存顺序。
        use core::sync::atomic::{AtomicUsize, Ordering};
        // 全局非确定性状态，初始为 0。
        static PER_HASHER_NONDETERMINISM: AtomicUsize = AtomicUsize::new(0);

        // 以宽松顺序读取全局状态，并转换为 u64。
        let nondeterminism = PER_HASHER_NONDETERMINISM.load(Ordering::Relaxed) as u64;
        // 将栈地址与全局非确定性状态混合。
        per_hasher_seed = folded_multiply(per_hasher_seed, ARBITRARY1 ^ nondeterminism);
        // 以宽松顺序写回新的全局状态。
        PER_HASHER_NONDETERMINISM.store(per_hasher_seed as usize, Ordering::Relaxed);
    }

    // 额外进行一次混合次骤，以确保得到良好的随机位。
    folded_multiply(per_hasher_seed, ARBITRARY2)
}
```

