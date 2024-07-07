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

    .section .text
    .globl __alltraps
    .globl __restore
.align 2  # 后续代码按 2^2=4 字节对齐byte对齐，这是 RISC-V 特权级规范的要求,

# -------------------用户栈-> 内核栈的保存------------------
__alltraps: # symbol：符号
    # 这里储存的方式也跟TrapContext结构体的字段对应！
    # 原子指令 
    # csrrw reg1,CSR,reg2
    # 1. 将CSR状态寄存器 里面的值放在reg1寄存器
    # 2. 再将reg2寄存器里面的值写入CSR状态寄存器
    # sscratch: 向异常处理程序提供一个字的临时存储
    csrrw sp, sscratch, sp 
    # => 将sp指针与sscratch值互换

    addi sp, sp, -34*8
    # => 栈负增长，将sp先下移34个寄存器

    sd x1, 1*8(sp)
    # x1(ra：返回地址)，一般放在栈最上面 
    # riscv64：一个寄存器8byte
    # => 将x1寄存器的值 写入 M[sp + 1*8byte] 
    sd x3, 3*8(sp)
    # x3在riscv里面是全局变量，静态变量的基地址
    # x4是线程指针
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
    # => 将sstatus trap特权级存到to临时寄存器

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

    mv a0, sp 
    # => sp里面是现在的，trap之后的新栈顶值，放在a0作为(函数参数/返回值)

    call trap_handler

# -------------------内核栈 -> 用户栈的恢复 ------------------

__restore:
    # mv sp,a0 # 内核栈的栈帧sp，存放的用户栈的信息
    
    # 恢复trap之前的状态

    # 返回的sstatue写入t0
    ld t0, 32*8(sp)
    # 把t0值写入sstatues寄存器中 
    csrw sstatus, t0
    
    # 把sepc(返回的IP)写入t1
    ld t1, 33*8(sp)
    csrw sepc, t1

    # 返回的可能是用户栈fp(栈帧)写入t2
    ld t2, 2*8(sp)
    csrw sscratch, t2



    # 返回地址ra 写入x1寄存器中
    ld x1, 1*8(sp)
    # gp全局指针 写入x3寄存器中
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    
    # 内核释放内核栈
    addi sp, sp, 34*8
    # sp <-互换-> sscratch，效果sp:用户栈，sscratch：内核栈(临时寄存器，会被清)
    csrrw sp, sscratch, sp
    sret

//  addi sp, sp, 34*8返回之后不就是上个函数的栈帧？
//  为啥还要csrrw sp, sscratch, sp
//  因为sp可能是内核栈，sscratch是用户栈
//  sret:本身并不保证一定会返回到用户态，根据sstatus的SPP状态决定