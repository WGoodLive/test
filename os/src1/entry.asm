# 段声明部分
    .section .text.entry
    # 申请一个段 这部分代码放在.text位置，这位置默认最低，首先执行(main),这部分的名字叫.entry
    .globl _start
    # 声明公共符号 _start
_start:
    # li t1, 100 # t1 =100 
    la sp, boot_stack_top # 把栈底部(高地址)加载到sp sp总是指向栈空的位置
    call rust_main # 调用rust提供的内核调用入口，main.rs中实现

    .section .bss.stack 
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096*16 # 超出这个空间 可能会栈溢出
    .globl boot_stack_top
boot_stack_top:
    
    
