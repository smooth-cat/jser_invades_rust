/*------------------------ 链表 ------------------------*/
function box_03_linked_list() {
  class Node {
    constructor(element) {
      this.element = element;
      this.next = null;
    }
  }
  class List {
    head = null;
    tail = null;
    push_head(value) {
      const node = new Node(value);
      // 有往前补
      if (this.head) {
        node.next = this.head;
      }
      // 无作尾巴
      else {
        this.tail = node;
      }
      this.head = node;
    }

    push_tail(value) {
      const node = new Node(value);
      if (this.tail) {
        this.tail.next = node;
      } else {
        this.head = node;
      }
      this.tail = node;
    }

    static from_iter(arr) {
      const list = new List();
      for (const v of arr) {
        list.push_tail(v);
      }
      return list;
    }

    len() {
      let pointer = this.head;
      let count = 0;
      while (pointer) {
        count++;
        pointer = pointer.next;
      }
      return count;
    }
  }

  List.from_iter([1, 2, 3, 4, 5]);
}


function rc_04_cycle_leak() {
  class Node {
    constructor(age) {
      this.age = age;
      this.next = null;
    }
  }

  const node1 = new Node(1);
  const node2 = new Node(2);

  node1.next = node2;
  node2.next = node1;

  // 代码运行结束依然释放 roots = [...node1,node2] 被标记为垃圾
}