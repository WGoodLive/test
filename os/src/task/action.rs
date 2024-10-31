//! 配合信号量的，信号处理例程

use super::signal::{SignalFlags, MAX_SIG};


/// 每进程的处理函数地址的集合
#[derive(Clone)]
pub struct SignalActions{
    pub table:[SignalAction;MAX_SIG+1]
}

/// 函数例程处理的封装
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    pub handler: usize,
    pub mask: SignalFlags,
}

impl Default for SignalAction {
    fn default() -> Self {
        Self {
            handler: 0,
            mask: SignalFlags::from_bits(40).unwrap(),
        }
    }
}

impl Default for SignalActions {
    fn default() -> Self {
        Self {
            table: [SignalAction::default(); MAX_SIG + 1],
        }
    }
}