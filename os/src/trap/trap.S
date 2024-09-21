.altmacro # 没有这句话 .rept伪指令无效
# RISC-V 架构中常规的数据处理和访存类指令只能操作通用寄存器而不能操作 CSR


# -----------------------------宏-----------------------
# .macro 宏名字 参数
#      宏方法体
# .endm

.macro SAVE_GP n
    sd x\n, \n*8(sp)  #将xn寄存器的值 写入 M[sp + n*8byte] 
.endm

.macro LOAD_GP n
    ld x\n,\n*8(sp) # 将M[sp + n*8byte]的数值 写入 xn寄存器 
.endm

# ---------------------------方法体-----------------------
    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 2  # 后续代码按 2^2=4 字节对齐byte对齐，这是 RISC-V 特权级规范的要求,

# -------------------用户栈-> 内核栈的保存------------------
__alltraps: # symbol：符号
    # 这里储存的方式也跟TrapContext结构体的字段对应！
    # 原子指令 
    # csrrw reg1,CSR,reg2
    csrrw sp, sscratch, sp 
    # => 将sp指针与sscratch值互换
    # 换完之后，sp->trap_context,sscratch -> user_kernel
    sd x1, 1*8(sp)
    # x1(ra：返回地址)，一般放在栈最上面 
    # riscv64：一个寄存器8byte
    # => 将x1寄存器的值 写入 M[sp + 1*8byte] 
    sd x3, 3*8(sp)
    # x3在riscv里面是全局变量，静态变量的基地址
    
    # x2好像是栈地址，之后保存
    # x4是线程指针，目前用不到
    # 他俩都不类似其他语言的fp：fp是函数的堆栈基地址


    .set n, 5
    # => n=5
    .rept 27 # => repetition:下面代码重复27次(x5 - x31)
        SAVE_GP %n
        .set n, n+1 # => n++
    .endr

    # csrr reg1,CSR
    # 将CSR状态寄存器 里面的值放在reg1寄存器 
    csrr t0, sstatus
    # => 将sstatus trap特权级存到t0临时寄存器

    csrr t1, sepc
    # sepc(对于同步异常，指向触发异常的指令； 对于中断，指向中断处理后应恢复执行的指令)
    # => sepc放在t1临时寄存器中

    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # => sepc、sstatus被放在了栈中33、32位置

    csrr t2, sscratch
    # => 把trap之前的sp位置存在t2临时寄存器(上面sscratch与sp被交换过)
    
    sd t2, 2*8(sp)
    # => 类似其他语言的fp，栈帧，存放在2号位

    # 加载内核的stap到t0
    ld t0,34*8(sp)
    # 加载trap_handle进入t1寄存器
    ld t1,36*8(sp)
    # 加载内核的sp给sp
    ld sp,35*8(sp)

    # t0 -> satp
    # 进入内核地址空间了
    csrw satp,t0
    sfence.vma
    jr t1   # jump to t1寄存器保存的函数位置

# -------------------内核栈 -> 用户栈的恢复 ------------------

__restore:
    # a0:用户栈的Trap_context
    # a1:用户栈的起始物理地址（token）
    csrw satp,a1
    # 进入了用户地址空间
    sfence.vma
    csrw sscratch,a0
    mv sp,a0

    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr

    # 返回用户栈
    ld sp, 2*8(sp)
    sret