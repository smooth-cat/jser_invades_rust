package main

import ("fmt")

// Hero 英雄基础结构体
type Hero struct {
	HP int
}

// NewHero 创建一个默认初始血量为 100 的 Hero
func NewHero() Hero {
	return Hero { HP: 100 }
}

// Attack 普通攻击：消耗目标 1 点血量
func (h *Hero) Attack(target *Hero) {
	target.HP -= 1
}

// Support 辅助：嵌套 Hero 组合其属性和方法
type Support struct {
	Hero
}

// NewSupport 创建辅助角色
func NewSupport() Support {
	return Support {
		Hero: NewHero(),
	}
}

// Heal 回血：恢复 5 点血量，上限 100
func (s *Support) Heal() {
	s.HP += 5
	if s.HP > 100 {
		s.HP = 100
	}
}

// Assassin 刺客：嵌套 Hero 组合其属性和方法
type Assassin struct {
	Hero
	CanKill bool
}

// NewAssassin 创建刺客角色
func NewAssassin() Assassin {
	return Assassin {
		Hero:    NewHero(),
		CanKill: false,
	}
}

// Kill 击杀：若满足条件则直接清空目标血量
func (a *Assassin) Kill(target *Hero) {
	if a.CanKill {
		target.HP = 0
	}
}

func main() {
	// 1. 初始化角色
	support := NewSupport()
	assassin := NewAssassin()

	// 2. 假设辅助受伤，测试普通攻击与回血
	fmt.Printf("初始辅助血量: %d\n", support.HP) // 100

	// 刺客普通攻击辅助（继承自 Hero 的方法）
	assassin.Attack(&support.Hero)
	fmt.Printf("受到普通攻击后血量: %d\n", support.HP) // 99

	// 辅助技能回血
	support.Heal()
	fmt.Printf("回血后血量: %d\n", support.HP) // 100

	// 3. 测试刺客的斩杀技能
	fmt.Println("--- 刺客释放技能 ---")
	
	// CanKill 为 false 时，尝试斩杀
	assassin.Kill(&support.Hero)
	fmt.Printf("未开启触发条件时的血量: %d\n", support.HP) // 100

	// 开启 CanKill 条件后再次斩杀
	assassin.CanKill = true
	assassin.Kill(&support.Hero)
	fmt.Printf("触发击杀后的血量: %d\n", support.HP) // 0
}