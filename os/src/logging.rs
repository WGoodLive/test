

use log::{Level, LevelFilter, Log, Metadata, Record, RecordBuilder};

struct Mylogger;

impl Log for Mylogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // metadata.level() >= Level::Trace
        true
    }

    fn log(&self, record: &Record) {
        let color = match record.level() {
            Level::Error => 31, // Red
            Level::Warn => 93,  // BrightYellow
            Level::Info => 34,  // Blue
            Level::Debug => 32, // Green
            Level::Trace => 90, // BrightBlack
        };
        println!("\u{1B}[{}m[{:>5}] {}\u{1B}[0m",color,record.level(),record.args());
    }
    /* 
    两者的主要区别在于转义字符的表示方式：
    \u{1B} 是 Unicode 转义字符，它使用四个十六进制数字来表示一个字节，即 U+001B。
    \x1b 是传统的 ASCII 转义字符，它使用两个十六进制数字来表示一个字节，即 0x1B。

    在大多数现代系统中，这两种转义字符都可以工作，因为终端通常支持 ANSI escape codes。
    \u{1B} 是 Unicode 转义字符，它可以在 UTF-8 编码的字符串中正常工作，
    \x1b 是 ASCII 转义字符，它只能在 ASCII 或兼容 ASCII 的编码中正常工作。
    */
    
    fn flush(&self) {
        
    }
}

pub fn init_Log(){
    // static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&Mylogger).unwrap();  // 自定义日志
    log::set_max_level(match option_env!("LOG") {
        // 什么也不输入，就不打印日志
        // make run Log=Trace 过滤这个等级
        Some("ERROR") => LevelFilter::Error,
        Some("WARN") => LevelFilter::Warn,
        Some("INFO") => LevelFilter::Info,
        Some("DEBUG") => LevelFilter::Debug,
        Some("TRACE") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    }); // 过滤等级
}

