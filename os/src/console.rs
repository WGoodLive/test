use crate::sbi::console_putchar; // 字符输入
use core::fmt::{self, Write};

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes(){
            // console_putchar('-' as usize); // 每个字符都用了这个，不论用户还是内核
            console_putchar(c as usize);
        }
        Ok(())
    }
}

pub fn print(args:fmt::Arguments){
    Stdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?)); // 内核态换行会用，用户态换行用他自己的println!
    }
}

