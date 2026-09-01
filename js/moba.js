/*------------------------ moba 游戏 ------------------------*/
/**
 * 有两个英雄：
 * 1. 辅助
 *   - 普通攻击
 *   - 治疗
 *
 * 2. 刺客
 *   - 普通攻击
 *   - 刺杀
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