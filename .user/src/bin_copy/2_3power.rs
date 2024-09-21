#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

const SIZE: usize = 10;
const P: u32 = 3;
const STEP: usize = 100000;
const MOD: u32 = 10007;
// 我们应用的兼容性比较受限，当应用用到较多特性时很可能就不再兼容 Qemu[后面会改善]
#[no_mangle]
fn main() -> i32 {
    // 定义一个长度为SIZE的u32类型的数组pow
    let mut pow = [0u32; SIZE];
    // 定义一个usize类型的变量index，初始值为0
    let mut index: usize = 0;
    // 将pow数组的第一个元素赋值为1
    pow[index] = 1;
    // 从1开始循环，直到STEP
    for i in 1..=STEP {
        // 获取pow数组的最后一个元素
        let last = pow[index];
        // 将index加1，并对SIZE取模，得到新的index
        index = (index + 1) % SIZE;
        // 将新的index位置的元素赋值为last乘以P再对MOD取模
        pow[index] = last * P % MOD;
        // 如果i能被10000整除，则打印P的i次方对MOD取模的结果
        if i % 10000 == 0 {
            println!("{}^{}={}(MOD {})", P, i, pow[index], MOD);
        }
    }
    // 打印测试结果
    println!("Test power OK!");
    // 返回0
    0
}
