
/// Task Context
#[derive(Copy, Clone)]
#[repr(C)]
pub struct TaskContext{
    ra:usize,
    sp:usize,
    s:[usize;12], // 这几个寄存器暂时没用上
}

impl TaskContext {
    /// 用0初始化
    pub fn zero_init()-> Self{
        Self{
            ra:0,
            sp:0,
            s:[0;12],
        }
    }

    pub fn set_s(&mut self,id:usize,value:usize){
        self.s[id] = value;
    }
    pub fn p_s(&self,id:usize)->usize{
        self.s[id]
    }

    pub fn goto_restore(kstask_ptr:usize)->Self{
        extern "C"{
            fn __restore();
        }
        Self { 
            ra: __restore as usize, 
            sp: kstask_ptr, 
            s: [0;12], 
        }
    }
}