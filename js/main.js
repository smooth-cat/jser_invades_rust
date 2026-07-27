/*------------------------ 计数器 ------------------------*/
class Counter {
  count = 0;
  /** 添加 */
  inc() {
    return ++this.count;
  }
  /** 减去 */
  sub() {
    return --this.count;
  }
}

/*------------------------ moba 游戏 ------------------------*/
/**
 * 1. 英雄实现 hp、attack 基础方法
 * 2. 辅助实现 heal 治疗
 * 3. 刺客实现 can_kill、kill 方法
 */
/** 英雄 */
class Hero {
  hp = 100;
  /** 普通攻击 */
  attack(target) {
    target.hp -= 1;
  }
}

/** 辅助 */
class Support extends Hero {
  /** 回血 */
  heal() {
    this.hp = Math.min(this.hp + 5, 100);
  }
}

/** 刺客 */
class Assassin extends Hero {
  canKill = false;
  /** 击杀 */
  kill(target) {
    if(canKill) {
      target.hp = 0;
    }
  }
}