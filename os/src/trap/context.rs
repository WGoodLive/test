use riscv::register::sstatus::{self,Sstatus,SPP};

#[repr(C)] // 按C语言的内存布局来定义结构体
/// Trap上下文
pub struct TrapContext {
    pub x: [usize; 32], // 32个通用寄存器
    pub sstatus: Sstatus, // 状态寄存器
    pub sepc: usize, // 异常程序计数器
}

impl TrapContext{
    pub fn set_sp(&mut self,sp:usize){
        self.x[2] = sp;
    }

    /// ## 上下文初始化
    /// - 寄存器初始化了，但是没更新  
    /// - sstatue变成了用户态  
    /// - sepc重新变成了entry  
    pub fn app_init_context(entry:usize,sp:usize) ->Self{
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self{
          x:[0;32],
          sstatus,
          sepc:entry,  
        };
        cx.set_sp(sp);
        cx
    }
}