pub fn demo() {
  // ECS + Sparse Set 存储
  // 解决 Vec<Option<T>> 的空洞问题和 HashMap 的缓存不友好问题
  //
  // 核心结构（每个组件类型一套）：
  //   sparse: Vec<usize>        sparse[entity_id] → dense 中的下标（无组件时为 EMPTY）
  //   dense:  Vec<T>            实际数据，紧密排列，遍历零空洞
  //   entities: Vec<Entity>     entities[dense_index] → 该槽位属于哪个实体（删除时反查用）

  type Entity = usize;
  const EMPTY: usize = usize::MAX; // 哨兵值：表示"该实体没有这个组件"

  // ═══════════════════════════════════════
  // SparseSet<T>：通用稀疏集
  // ═══════════════════════════════════════

  struct SparseSet<T> {
    sparse: Vec<usize>,    // entity_id → dense index
    dense: Vec<T>,         // 紧密存储的组件数据
    entities: Vec<Entity>, // dense index → entity_id（反向映射）
  }

  impl<T> SparseSet<T> {
    fn new() -> Self {
      SparseSet {
        sparse: Vec::new(),
        dense: Vec::new(),
        entities: Vec::new(),
      }
    }

    // 确保 sparse 数组能容纳该 entity_id
    fn ensure_capacity(&mut self, entity: Entity) {
      if entity >= self.sparse.len() {
        self.sparse.resize(entity + 1, EMPTY);
      }
    }

    // 该实体是否拥有此组件
    fn contains(&self, entity: Entity) -> bool {
      entity < self.sparse.len() && self.sparse[entity] != EMPTY
    }

    // 插入组件
    fn insert(&mut self, entity: Entity, component: T) {
      self.ensure_capacity(entity);
      if self.sparse[entity] != EMPTY {
        // 已有则覆盖
        self.dense[self.sparse[entity]] = component;
      } else {
        // 新增：追加到 dense 末尾
        self.sparse[entity] = self.dense.len();
        self.dense.push(component);
        self.entities.push(entity);
      }
    }

    // 不可变借用
    fn get(&self, entity: Entity) -> Option<&T> {
      if self.contains(entity) {
        Some(&self.dense[self.sparse[entity]])
      } else {
        None
      }
    }

    // 可变借用
    fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
      if self.contains(entity) {
        Some(&mut self.dense[self.sparse[entity]])
      } else {
        None
      }
    }

    // 删除组件（swap-remove：O(1)，无空洞）
    fn remove(&mut self, entity: Entity) -> Option<T> {
      if !self.contains(entity) {
        return None;
      }
      let idx = self.sparse[entity];
      let last = self.dense.len() - 1;

      // 把最后一个元素搬到被删位置
      self.dense.swap(idx, last);
      self.entities.swap(idx, last);

      // 更新被搬过来的那个实体的 sparse 映射
      let moved_entity = self.entities[idx];
      self.sparse[moved_entity] = idx;

      // 清除被删实体的映射
      self.sparse[entity] = EMPTY;

      // 弹出末尾（原来的被删元素）
      self.entities.pop();
      Some(self.dense.pop().unwrap())
    }

    // 遍历所有拥有此组件的实体（缓存友好：dense 连续内存）
    fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
      self.entities.iter().copied().zip(self.dense.iter())
    }
  }

  // ═══════════════════════════════════════
  // Component：纯数据
  // ═══════════════════════════════════════

  struct Health {
    hp: i32,
  }

  struct CanKill {
    enabled: bool,
  }

  #[derive(Debug, Clone, Copy, PartialEq)]
  enum HeroKind {
    Support,
    Assassin,
  }

  // ═══════════════════════════════════════
  // World：用 SparseSet 管理每种组件
  // ═══════════════════════════════════════

  struct World {
    next_id: Entity,
    health: SparseSet<Health>,
    can_kill: SparseSet<CanKill>,
    kind: SparseSet<HeroKind>,
  }

  impl World {
    fn new() -> Self {
      World {
        next_id: 0,
        health: SparseSet::new(),
        can_kill: SparseSet::new(),
        kind: SparseSet::new(),
      }
    }

    fn spawn(&mut self) -> Entity {
      let id = self.next_id;
      self.next_id += 1;
      id
    }

    // 销毁实体：从所有 SparseSet 中移除其组件
    fn despawn(&mut self, entity: Entity) {
      self.health.remove(entity);
      self.can_kill.remove(entity);
      self.kind.remove(entity);
    }

    fn health(&self, e: Entity) -> &Health {
      self.health.get(e).expect("没有 Health 组件")
    }

    fn health_mut(&mut self, e: Entity) -> &mut Health {
      self.health.get_mut(e).expect("没有 Health 组件")
    }

    fn can_kill_mut(&mut self, e: Entity) -> &mut CanKill {
      self.can_kill.get_mut(e).expect("没有 CanKill 组件")
    }

    fn kind(&self, e: Entity) -> HeroKind {
      *self.kind.get(e).expect("没有 HeroKind 组件")
    }
  }

  // ═══════════════════════════════════════
  // System：纯函数
  // ═══════════════════════════════════════

  fn attack_system(world: &mut World, _attacker: Entity, target: Entity) {
    world.health_mut(target).hp -= 1;
  }

  fn heal_system(world: &mut World, entity: Entity) {
    let h = world.health_mut(entity);
    h.hp = (h.hp + 5).min(100);
  }

  fn kill_system(world: &mut World, attacker: Entity, target: Entity) {
    if world.can_kill.get(attacker).map_or(false, |k| k.enabled) {
      world.health_mut(target).hp = 0;
    }
  }

  // 批量 System 示例：所有 Support 自动回血（遍历 dense，零空洞）
  fn regen_system(world: &mut World) {
    // 收集需要治疗的实体（避免借用冲突）
    let supports: Vec<Entity> = world
      .kind
      .iter()
      .filter(|(_, k)| **k == HeroKind::Support)
      .map(|(e, _)| e)
      .collect();

    for e in supports {
      let h = world.health_mut(e);
      h.hp = (h.hp + 2).min(100);
    }
  }

  // ═══════════════════════════════════════
  // 初始化 & 运行
  // ═══════════════════════════════════════

  struct PlayerConfig {
    kind: HeroKind,
    name: &'static str,
  }

  fn spawn_hero(world: &mut World, cfg: &PlayerConfig) -> Entity {
    let e = world.spawn();
    world.health.insert(e, Health { hp: 100 });
    world.kind.insert(e, cfg.kind);

    match cfg.kind {
      HeroKind::Assassin => {
        world.can_kill.insert(e, CanKill { enabled: false });
      }
      HeroKind::Support => {}
    }
    e
  }

  let roster: Vec<PlayerConfig> = vec![
    PlayerConfig {
      kind: HeroKind::Support,
      name: "奶妈A",
    },
    PlayerConfig {
      kind: HeroKind::Support,
      name: "奶妈B",
    },
    PlayerConfig {
      kind: HeroKind::Assassin,
      name: "刺客1",
    },
    PlayerConfig {
      kind: HeroKind::Assassin,
      name: "刺客2",
    },
    PlayerConfig {
      kind: HeroKind::Assassin,
      name: "刺客3",
    },
  ];

  let mut world = World::new();
  let players: Vec<Entity> = roster
    .iter()
    .map(|cfg| spawn_hero(&mut world, cfg))
    .collect();

  for (i, &e) in players.iter().enumerate() {
    let kind_str = match world.kind(e) {
      HeroKind::Support => "辅助",
      HeroKind::Assassin => "刺客",
    };
    println!(
      "[{}] {} - {} hp:{}",
      i,
      roster[i].name,
      kind_str,
      world.health(e).hp
    );
  }

  println!("\n--- 战斗 ---");

  attack_system(&mut world, players[2], players[0]);
  println!("刺客1 攻击 奶妈A → hp:{}", world.health(players[0]).hp); // 99

  heal_system(&mut world, players[0]);
  println!("奶妈A 治疗 → hp:{}", world.health(players[0]).hp); // 100

  world.can_kill_mut(players[3]).enabled = true;
  kill_system(&mut world, players[3], players[1]);
  println!("刺客2 击杀 奶妈B → hp:{}", world.health(players[1]).hp); // 0

  // —— 演示删除：奶妈B 死亡，从 World 中移除 ——
  println!("\n--- 删除实体（Sparse Set swap-remove）---");
  world.despawn(players[1]);
  println!(
    "奶妈B 已移除，can_kill 组件存在? {}",
    world.can_kill.contains(players[1])
  ); // false
  println!("刺客1 仍存活，hp:{}", world.health(players[2]).hp); // 100

  // —— 批量回血 System ——
  println!("\n--- regen_system：所有辅助 +2hp ---");
  attack_system(&mut world, players[2], players[0]); // 先打一下 99
  regen_system(&mut world);
  println!("奶妈A hp:{}", world.health(players[0]).hp); // 100 (99+2 上限100... 实际 99+2=101→100)

  // 遍历所有有 Health 的实体（dense 连续遍历）
  println!("\n--- 遍历所有存活实体 ---");
  for (e, h) in world.health.iter() {
    println!("  entity {} → hp:{}", e, h.hp);
  }
}