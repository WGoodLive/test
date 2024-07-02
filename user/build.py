# 用于生成linker-ld
import os;
base_address = 0x80400000
step = 0x200000
linker = "src/linker.ld"

app_id = 3
apps = os.listdir("src/bin")
apps.sort()
for app in apps:
    app = app[:app.find('.')] # 去除 .rs
    lines = []
    lines_before = []
    with open(linker,'r') as f:
        for line in f.readlines(): # 对每一行就替换0x80400000,有就替换，没有就算了
            lines_before.append(line)
            line = line.replace(hex(base_address),hex(base_address+app_id*step))
            lines.append(line)
    with open(linker,'w+') as f:
        f.writelines(lines)
        # rust bin文件夹下面的每一个文件，都会被单独编译成一个可执行文件。
        # cargo run 会让你选择一个bin文件夹下面的.rs文件
    os.system('cargo build --bin %s --release'% app) #　这里选择bin/app.rs的程序进行构建
    print('[build.py] application %s start with address %s' %(app, hex(base_address+step*app_id)))
    # 恢复现场
    with open(linker, 'w+') as f:
        f.writelines(lines_before)
    app_id = app_id + 1
