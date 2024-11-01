//! 大致懂了，由于缺少guard的实现，进度搁置

use thread::Runtime;

mod thread;

pub static mut RUNTIME:usize = 0;

fn main() {
    let mut runtime = Runtime::new();
    runtime.init();
}
