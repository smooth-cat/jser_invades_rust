package main

import "fmt"

/**
 * 能力组合（mixin 式）：拼的是"行为块"，对照对象组合（拼的是数据，嵌 Hero 的老写法）
 * JS:   const CanAttack = (state) => ({ attack(t) { t.hp -= 1 } })，spread 拼接
 * Rust: trait 默认方法 + 空 impl（trait_05_mixin.rs）
 * Go 的对应物：
 *   能力块 = 持有共享 State 指针的结构体（Go 接口没有默认方法，行为只能挂在结构体上）
 *   共享 State 指针 = JS mixin 闭包捕获的 state，所有能力块操作同一份数据
 *   拼装 = 嵌入能力块（方法提升），等价于 spread
 *   多能力约束 = 接口方法集合（等价 Rust 约束里的 +）
 */

// State 前置数据：能力块只能通过共享的 State 碰数据（trait 不能有字段的 Go 对应约束）
type State struct {
	HP      int
	CanKill bool
}

// ----------------- 能力块定义（mixin 本体，行为只写一次） -----------------

// CanAttack 普通攻击能力块
type CanAttack struct{ State *State }

// Attack 攻击不修改自身，target 是任何 *State（鸭子类型：只要也是 State 数据就合法）
func (c CanAttack) Attack(target *State) {
	target.HP -= 1
}

// CanHeal 回血能力块
type CanHeal struct{ State *State }

// Heal 回血：恢复 5 点，上限 100
func (c CanHeal) Heal() {
	c.State.HP += 5
	if c.State.HP > 100 {
		c.State.HP = 100
	}
}

// CanKill 斩杀能力块
type CanKill struct{ State *State }

// Kill 击杀：满足条件则清空目标血量
func (c CanKill) Kill(target *State) {
	if c.State.CanKill {
		target.HP = 0
	}
}

// ----------------- 拼装：嵌入能力块 = spread -----------------

// Support 辅助：拼了 CanAttack + CanHeal，没拼 CanKill
type Support struct {
	*State
	CanAttack
	CanHeal
}

// NewSupport 工厂：状态只创建一份，各能力块共享同一指针（JS spread 时传同一个 state）
func NewSupport() *Support {
	state := &State{HP: 100}
	return &Support{State: state, CanAttack: CanAttack{state}, CanHeal: CanHeal{state}}
}

// Assassin 刺客：拼了 CanAttack + CanKill，没拼 CanHeal
type Assassin struct {
	*State
	CanAttack
	CanKill
}

// NewAssassin 创建刺客角色
func NewAssassin() *Assassin {
	state := &State{HP: 100}
	return &Assassin{State: state, CanAttack: CanAttack{state}, CanKill: CanKill{state}}
}

// ----------------- 多能力约束：接口方法集合 -----------------

// AttackAble 单能力接口：列出"拼了该能力的类型"应有的方法
type AttackAble interface {
	Attack(target *State)
}

// HealAble 回血能力接口
type HealAble interface {
	Heal()
}

// AttackHealAble 接口嵌入 = Rust 约束里的 +：T: CanAttack + CanHeal
type AttackHealAble interface {
	AttackAble
	HealAble
}

// DrainAttack 吸血攻击：只有同时拼了两个能力的类型才进得来
func DrainAttack(attacker AttackHealAble, target *State) {
	attacker.Attack(target)
	attacker.Heal()
}

func main() {
	fmt.Println("----------------- 拼装：嵌入能力块（方法提升后调用点和 JS 一致） -----------------")
	// 1. 初始化角色
	support := NewSupport()
	assassin := NewAssassin()

	// 初始血量（嵌入 *State 后 support.HP 直接可读）
	fmt.Printf("初始辅助血量: %d\n", support.HP) // 100

	// 2. attack 的 target 是任何 *State：Support/Assassin 都能被打
	// （对比对象组合：只能打内部 Hero，这里任何持有 State 的类型都是合法目标）
	assassin.Attack(support.State)
	fmt.Printf("受到普通攻击后血量: %d\n", support.HP) // 99

	// 辅助技能回血（拼了 CanHeal 才有这个方法）
	support.Heal()
	fmt.Printf("回血后血量: %d\n", support.HP) // 100

	// 能力隔离（编译期强制，JS 的 spread 对象做不到）：
	// support.Kill(assassin.State) // ❌ 编译错误：support.Kill undefined

	// 3. 测试刺客的斩杀技能
	fmt.Println("--- 刺客释放技能 ---")

	// 注意嵌入歧义：能力块类型名 CanKill 和 State 字段名 CanKill 撞名，
	// assassin.CanKill 会在"嵌入字段"和"提升字段"之间歧义，必须显式写 assassin.State.CanKill
	assassin.State.CanKill = false
	assassin.Kill(support.State)
	fmt.Printf("未开启触发条件时的血量: %d\n", support.HP) // 100

	// 开启 CanKill 条件后再次斩杀
	assassin.State.CanKill = true
	assassin.Kill(support.State)
	fmt.Printf("触发击杀后的血量: %d\n", support.HP) // 0

	// 4. 吸血攻击：Support 同时拼了两个能力，满足 AttackHealAble
	//（Assassin 没拼 CanHeal，传进去会编译错误：Assassin does not implement HealAble）
	DrainAttack(support, assassin.State)
	fmt.Printf("吸血攻击后 辅助血量: %d 刺客血量: %d\n", support.HP, assassin.HP) // 5 99
}
