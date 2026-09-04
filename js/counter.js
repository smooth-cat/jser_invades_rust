/*------------------------ 计数器 ------------------------*/
class Counter {
  count = 0;
  /** 自增 */
  inc() {
    return ++this.count;
  }
  /** 自减 */
  sub() {
    return --this.count;
  }
}