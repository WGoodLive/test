// 系统调用库
use core::arch::asm;
// 系统调用号
const SYSCALL_WRITE:usize = 64;
const SYSCALL_EXIT:usize = 93;
const SYSCALL_YIELD:usize = 124;
const SYSCALL_GET_TIME:usize = 169;

// args:[usize;3]
// 一个包含三个无符号整数的数组
fn syscall(id:usize,args:[usize;3]) -> isize { 
    let mut ret:isize;
    unsafe{
        asm!(
            "ecall", // 环境切换
            inlateout("x10") args[0] => ret, 
            //x10 = a0输入参数/函数返回值
            //inlateout说明a0不仅作为(args[0])输入，还作为输出ret(上面的变量)
            in("x11") args[1],// args[1]输入到寄存器x11(a1)
            in("x12") args[2],
            in("x17") id // 系统调用号输入到x17
        );
    } 
    ret
}

/// 功能：将内存中缓冲区中的数据写入文件。  
/// 参数：  
/// `fd` 表示待写入文件的文件描述符id；  
/// `buf` 不仅含有字符串首地址，还蕴含长度信息;  
/// 返回值：返回成功写入的长度。  
/// `syscall ID`：64  
/// e.g.`buf.as_ptr`：指针；`buf.len()`:字符串长度  
pub fn sys_write(fd: usize, buf: &[u8]) -> isize {
    syscall(SYSCALL_WRITE, [fd, buf.as_ptr() as usize, buf.len()])
}

// 功能：退出应用程序并将返回值告知批处理系统。
/// 参数：`exit_code` 表示应用程序的返回值。
/// 返回值：该系统调用不应该返回。
/// syscall ID：93
pub fn sys_exit(xstate:i32)->isize{
    syscall(SYSCALL_EXIT, [xstate as usize, 0, 0])
}

pub fn sys_yield() -> isize {
    syscall(SYSCALL_YIELD, [0, 0, 0])
}

pub fn sys_get_time() -> isize {
    syscall(SYSCALL_GET_TIME, [0, 0, 0])
}


