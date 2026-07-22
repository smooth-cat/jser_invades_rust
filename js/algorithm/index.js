import * as util from '../util/index.js';
export function fib(n) {
  if(n < 2) {
    return n;
  }
  return util.calc.add(fib(n - 1), fib(n - 2));
}