.altmacro

# sn(0-11)：保存寄存器，其中s0常保存fp
# a0,a1：函数参数/返回值
.macro SAVE_SN n
    sd s\n,(\n+2)*8(a0) # sn -> M[(n+2)*8+a0]
.endm

.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)# sn <- M[(n+2)*8+a1]
.endm


    .section .text
    .globl __switch
__switch:
    # 阶段 [1]
    # __switch(
    #     current_task_cx_ptr: *mut TaskContext,
    #     next_task_cx_ptr: *const TaskContext
    # )
    # 在函数参数上做到就行

    # 阶段 [2]
    # save kernel stack of current task
    sd sp,8(a0)
    # save ra & s0~s11 of current execution
    sd ra, 0(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr

    # 阶段 [3]
    # restore ra & s0~s11 of next execution
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # restore kernel stack of next task
    ld sp, 8(a1)
    # 阶段 [4]
    ret 
    # 没执行sret.
    # ret用于函数调用返回，mret和sret用于从高特权模式切换回陷入前的地特权模式
