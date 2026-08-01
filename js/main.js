const a = {
  name: 'a',
  b: null
};

const b = {
  name: 'b',
  a: null
};
a.b = b;
b.a = a;

/**
 * JS 根据 根可达性来释放内存
 * 根节点群包括：globalThis、栈帧中的变量s、等
 * [ globalThis, const a, const b ]: GC Roots
 *  ↓
 * globalThis
 *  ↓
 *  a <-> b
 */
globalThis.a = a;


setTimeout(() => {
  /**
   * 释放 a <-> b
   * [ globalThis ]: GC Roots
   *  ↓
   * globalThis  
   *  
   *  a <-> b
   */
  globalThis.a = undefined;
}, 1000);