function return_closure(params) {
  let n = 0;
  const closure = () => {
    n += 1;
    return n;
  };
  n = 10;
  console.log(n); // 10
  return closure;
}

const add = return_closure();
console.log(add()); // 11

function mock_rust_fn() {
  let n = 0;
  // 模拟 rust 的 FnMut, 用对象捕获数据
  const closure = {
    n: n,
    add() {
      this.n += 1;
      return this.n;
    }
  };
  n = 10;
  console.log('js 模拟 FnMut: ', n);
  return closure.add.bind(closure);
}
const add2 = mock_rust_fn();
console.log('js 模拟 FnMut: ', add2());