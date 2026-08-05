section .text
global square_sum           ; 导出 square_sum 函数符号，供 C 语言或其它模块调用

; -----------------------------------------------------------------------------
; 函数名: square_sum
; 功能: 计算两个 64 位整数的平方和 (a^2 + b^2)
; 参数:
;   RDI -> 第一个参数 a
;   RSI -> 第二个参数 b
; 返回值:
;   RAX -> 平方和结果 (a^2 + b^2)
; -----------------------------------------------------------------------------
square_sum:
    ; === 1. 建立栈帧 (Prologue) ===
    push rbp                ; 将调用者的 RBP 寄存器压栈保存
    mov rbp, rsp            ; 将当前栈指针 RSP 赋值给 RBP，建立新的栈帧基准

    ; === 2. 执行计算逻辑 ===
    ; --- 计算 a^2 ---
    mov rax, rdi            ; 将第一个参数 a (RDI) 复制到 RAX 中，准备做乘法
    imul rax, rdi           ; 执行有符号乘法: RAX = RAX * RDI (即 a^2)

    ; --- 计算 b^2 ---
    mov rbx, rsi            ; 将第二个参数 b (RSI) 复制到 RBX 中
    imul rbx, rsi           ; 执行有符号乘法: RBX = RBX * RSI (即 b^2)

    ; --- 计算 a^2 + b^2 ---
    add rax, rbx            ; 将 b^2 (RBX) 加到 a^2 (RAX) 上，结果存在 RAX 中

    ; === 3. 恢复栈帧并返回 (Epilogue) ===
    pop rbp                 ; 弹出并恢复调用者原本的 RBP
    ret                     ; 读取栈顶的返回地址并跳转回去 (返回值在 RAX 中)