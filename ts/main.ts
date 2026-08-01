interface Foo {
  a: string;
}
const foo: Foo = { a: 'hello' };
// @ts-ignore 越狱 拿到 undefined
const b = foo.b;

interface Foo2 {
  a?: string;
}
const foo2: Foo2 = {};

// @ts-ignore 这里 js 执行时会报错，因为执行 undefined.trim 会报错
foo2.a.trim();

// 必须使用可选链来处理 空值情况
foo2.a?.trim?.();

/*----------------- 类型协变 -----------------*/
class Animal {
  constructor(public name: string) {}
}

class Dog extends Animal {
  tail = true;
}

const dog = new Dog('dog');

function getName(animal: Animal) {
  return animal.name;
}
/*
  upcasting
  Dog 当成 Animal 
  为什么可以？
  因为 getName 只用 Animal 的字段
*/
getName(dog);