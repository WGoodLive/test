use std::{fs::{read_dir, File, OpenOptions}, io::{Read, Seek, SeekFrom, Write}, sync::{Arc, Mutex}};
use easy_fs::{BlockDevice, EasyFileSystem};  
use clap::{Arg, App};

const BLOCK_SZ: usize = 512;
/// 将一个文件系统，看成一个大文件
struct BlockFile(Mutex<File>);

impl BlockDevice for BlockFile {
    fn read_block(&self,block_id:usize,buf:&mut [u8]) {
        let mut file = self.0.lock().unwrap();
        // seek移动指针 SeekFrom::Start((block_id*BLOCK_SZ)as u64)：指针开始的位置
        file.seek(SeekFrom::Start((block_id*BLOCK_SZ)as u64))
        .expect("Error when seeking");
        // 开始读，并判断返回值
        assert_eq!(file.read(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.write(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }

    
}

fn easy_fs_pack() -> std::io::Result<()>{
    let matches = App::new("EasyFileSystem packer")
        .arg(Arg::with_name("source")
            .short("s")
            .long("source")
            .takes_value(true)
            .help("Executable source dir(with backslash)")
        )
        .arg(Arg::with_name("target")
            .short("t")
            .long("target")
            .takes_value(true)
            .help("Executable target dir(with backslash)")
        )
        .get_matches();
    let src_path = matches.value_of("source").unwrap();
    let target_path = matches.value_of("target").unwrap();
    println!("src_path = {}\ntarget_path = {}", src_path, target_path);

    // 创造文件的磁盘镜像
    let block_file = Arc::new(BlockFile(Mutex::new({
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}{}", target_path, "fs.img"))?; // target_path\fs.img
        f.set_len(8192 * 512).unwrap();
        f
    })));

    // 给磁盘创建文件系统
    let efs = EasyFileSystem::create(
        block_file.clone(),
        8192,
        1,
    );

    // 得到根目录
    let root_inode = Arc::new(EasyFileSystem::root_inode(&efs));

    // 收集应用程序名称
    let apps: Vec<_> = read_dir(src_path)
        .unwrap()
        .into_iter()
        .map(|dir_entry| {
            let mut name_with_ext = dir_entry.unwrap().file_name().into_string().unwrap();
            name_with_ext.drain(name_with_ext.find('.').unwrap()..name_with_ext.len()); // 把文件后面的.rs删除了
            name_with_ext
        })
        .collect();
    
    // 从app中加载相应的elf文件到文件系统中(以前是读入内存里面)
    for app in apps {
        let mut host_file = File::open(format!("{}{}", target_path, app)).unwrap();
        let mut all_data: Vec<u8> = Vec::new();
        host_file.read_to_end(&mut all_data).unwrap();
        let inode = root_inode.create(app.as_str()).unwrap();
        inode.write_at(0, all_data.as_slice());
    }

    /// 输出文件名
    for app in root_inode.ls() {
        println!("{}", app);
    }
    Ok(())
    // 在这个函数执行完之后(内存此时是Linux)，drop会把每个页，写入target/fs.img中
}
fn main() {
    easy_fs_pack().expect("Error when packing easy-fs!");
}

#[test]
fn efs_test() -> std::io::Result<()> {
    let block_file = Arc::new(BlockFile(Mutex::new({
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("target/fs.img")?;
        f.set_len(8192 * 512).unwrap();
        f
    })));
    EasyFileSystem::create(block_file.clone(), 4096, 1);
    let efs = EasyFileSystem::open(block_file.clone());
    let root_inode = EasyFileSystem::root_inode(&efs);
    root_inode.create("filea");
    root_inode.create("fileb");
    for name in root_inode.ls() {
        println!("{}", name);
    }
    let filea = root_inode.find("filea").unwrap();
    let greet_str = "Hello, world!";
    filea.write_at(0, greet_str.as_bytes());
    // let mut buffer: [u8; 512] = [0u8; 512];
    let mut buffer = [0u8; 233];
    let len = filea.read_at(0, &mut buffer);
    assert_eq!(greet_str, core::str::from_utf8(&buffer[..len]).unwrap(),);
    println!("{}:{}",buffer.len(),core::str::from_utf8(&buffer[..len]).unwrap());

    let mut random_str_test = |len: usize| {
        filea.clear();
        assert_eq!(filea.read_at(0, &mut buffer), 0,);
        let mut str = String::new();
        use rand;
        // random digit
        for _ in 0..len {
            str.push(char::from('0' as u8 + rand::random::<u8>() % 10));
        }
        filea.write_at(0, str.as_bytes());
        let mut read_buffer = [0u8; 127];
        let mut offset = 0usize;
        let mut read_str = String::new();
        loop {
            let len = filea.read_at(offset, &mut read_buffer);
            if len == 0 {
                break;
            }
            offset += len;
            read_str.push_str(core::str::from_utf8(&read_buffer[..len]).unwrap());
        }
        assert_eq!(str, read_str);
    };

    random_str_test(4 * BLOCK_SZ);
    // random_str_test(8 * BLOCK_SZ + BLOCK_SZ / 2);
    random_str_test(100 * BLOCK_SZ);
    // // 太大不行，栈溢出
    // random_str_test(70 * BLOCK_SZ + BLOCK_SZ / 7);
    // random_str_test((12 + 128) * BLOCK_SZ);
    // random_str_test(400 * BLOCK_SZ);
    // random_str_test(1000 * BLOCK_SZ);
    // random_str_test(2000 * BLOCK_SZ);

    Ok(())
}

