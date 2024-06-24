// 在运行时生成 link_app.S 汇编文件
// 当你运行 cargo build 或 cargo run 时，Cargo 会首先运行 build.rs（如果存在）以执行任何必要的构建步骤，然后编译并链接你的 Rust 代码。
// build.rs 脚本并不是由 main.rs 直接调用的；相反，它是在 Cargo 构建过程中自动执行的。
// 可以使用标准库 因为他可以是第三方构建
// build.rs好像编译，的时候就自动执行了
// 仿佛你改一步，他重新执行一次（实时更新）

use std::fs::{read_dir,File};
use std::io::{Write,Result};

fn main(){
    println!("cargo:rerun-if-changed=../user/src/");
    println!("cargo:rerun-if-changed={}", TARGET_PATH);
    insert_app_data().unwrap();
}

static TARGET_PATH: &str = "../user/target/riscv64gc-unknown-none-elf/release/";

fn insert_app_data() -> Result<()>{
    let mut f = File::create("src/link_app.S").unwrap();
    let mut apps:Vec<_> = read_dir("../user/src/bin").unwrap().into_iter()
    .map(|dir_entry|{
        // map实现的功能：
        // String(XXname.rs) ->  String(XXname)
        let mut  name_with_ext = 
        dir_entry.unwrap().file_name().into_string().unwrap();
        name_with_ext.drain(name_with_ext.find('.').unwrap()..name_with_ext.len());
        name_with_ext
        // drain:去掉索引里的东西，其他的留下
    }).collect();
    apps.sort();
    //writeln!(f, "{}", String)?; 覆盖文件的方式写入

    writeln!(
        f, // 下面的r#说明是多行字符串，
r#"
    .align 3 // 说明是2^3=8字节对齐
    .section .data
    .global _num_app
_num_app:
    .quad {}"#,
            apps.len()
        )?;
for i in 0..apps.len(){
    writeln!(f,r#"    .quad app_{}_start"#,i)?;
}
writeln!(f, r#"    .quad app_{}_end"#, apps.len() - 1)?;
for (idx, app) in apps.iter().enumerate() {
    println!("app_{}: {}", idx, app);
    writeln!(
        f,
        r#"
    .section .data
    .global app_{0}_start
    .global app_{0}_end
app_{0}_start:
    .incbin "{2}{1}.bin"
app_{0}_end:"#,
        idx, app, TARGET_PATH
    )?;
}
    Ok(())
}
