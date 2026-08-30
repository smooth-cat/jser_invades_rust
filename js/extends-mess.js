class Base {
  run() {
    this.foo(); 
  }
  foo() {
    console.log("base: foo");
  }
}

class Mid extends Base {
  foo() {
    console.log("mid: foo");
  }
}

class Leaf extends Mid {
  foo() {
    console.log("leaf: foo"); 
  }
}

new Leaf().run();

