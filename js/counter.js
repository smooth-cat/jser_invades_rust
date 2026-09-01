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