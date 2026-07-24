/*------------------------ 基础数据类型 ------------------------*/
// string
let name = 'hello'
// number
let age = 18
// boolean
let flag = true
// undefined
let score = undefined
// null
let count = null
let name2 = name

/*------------------------ 常量 ------------------------*/
const PERSON = {
  // age 可以改变
  age: 10
}
/*------------------------ 打包工具注入常量 ------------------------*/
if(__DEV__) {
  console.log('开发环境');
}