## 导读

提高操作系统性能与效率：

- 通过提前加载应用程序到内存，减少应用程序切换开销
- 通过协作机制支持程序主动放弃处理器，提高系统执行效率
- 通过抢占机制支持程序被动放弃处理器，保证不同程序对处理器资源使用的公平性，也进一步提高了应用对 I/O 事件的响应效率

内存容量在逐渐增大，处理器的速度也在增加，外设 I/O 性能方面的进展不大。这就使得以往内存只能放下一个程序的情况得到很大改善，但处理器的空闲程度加大了。于是科学家就开始考虑在内存中尽量同时驻留多个应用，这样处理器的利用率就会提高(在加载新的应用时，cpu基本在空闲，所以采用预加载)。

### 本章目标

实现这两种方案（并联）：

1. **多道程序**:内存中尽量同时驻留多个应用，这样处理器的利用率就会提高。但只有一个程序执行完毕后或主动放弃执行，处理器才能执行另外一个程序。

2. **分时共享（Time Sharing）** 或 **抢占式多任务（Multitasking）** ，也可合并在一起称为 **分时多任务** ：一个程序只能运行一段时间（可以简称为一个时间片, Time Slice）就一定会让出处理器。

**对于本章来说，多道程序和分时多任务系统都有一些共同的特点**：在内存中同一时间可以驻留多个应用，而且所有的应用都是在系统启动的时候分别加载到内存的不同区域中。

> [!note]
>
> **批处理与多道程序的区别是什么？**
>
> 对于批处理系统而言，它在一段时间内可以处理一批程序，但内存中只放一个程序，处理器一次只能运行一个程序，只有在一个程序运行完毕后再把另外一个程序调入内存，并执行。即批处理系统不能交错执行多个程序。
>
> 对于支持多道程序的系统而言，它在一段时间内也可以处理一批程序，但内存中可以放多个程序，一个程序在执行过程中，可以主动（协作式）或被动（抢占式）地放弃自己的执行，让另外一个程序执行。即支持多道程序的系统可以交错地执行多个程序，这样系统的利用率会更高。

获取多道程序的代码：

```bash
git clone https://github.com/rcore-os/rCore-Tutorial-v3.git
cd rCore-Tutorial-v3
git checkout ch3-coop
```

获取分时多任务系统的代码：

```bash
git clone https://github.com/rcore-os/rCore-Tutorial-v3.git
cd rCore-Tutorial-v3
git checkout ch3
```

### 任务图

#### 多道程序

##### 锯齿螈多道程序操作系统

![多道程序操作系统](./assets/jcy-multiprog-os-detail.png)

通过上图，大致可以看出Qemu把包含多个app的列表和MultiprogOS的image镜像加载到内存中，RustSBI（bootloader）完成基本的硬件初始化后，跳转到MultiprogOS起始位置，MultiprogOS首先进行正常运行前的初始化工作，即建立栈空间和清零bss段，然后通过改进的 AppManager 内核模块从app列表中把所有app都加载到内存中，并按指定顺序让app在用户态一个接一个地执行。app在执行过程中，会通过系统调用的方式得到MultiprogOS提供的OS服务，如输出字符串等

##### 始初龙协作式多道程序操作系统

![始初龙协作式多道程序操作系统 -- CoopOS总体结构](./assets/more-task-multiprog-os-detail.png)

通过上图，大致可以看出相对于MultiprogOS，CoopOS进一步改进了 AppManager 内核模块，把它拆分为负责加载应用的 Loader 内核模块和管理应用运行过程的 TaskManager 内核模块。 TaskManager 通过 task 任务控制块来管理应用程序的执行过程，支持应用程序主动放弃 CPU  并切换到另一个应用继续执行，从而提高系统整体执行效率。应用程序在运行时有自己所在的内存空间和栈，确保被切换时相关信息不会被其他应用破坏。如果当前应用程序正在运行，则该应用对应的任务处于运行（Running）状态；如果该应用主动放弃处理器，则该应用对应的任务处于就绪（Ready）状态。操作系统进行任务切换时，需要把要暂停任务的上下文（即任务用到的通用寄存器）保存起来，把要继续执行的任务的上下文恢复为暂停前的内容，这样就能让不同的应用协同使用处理器了。

#### 分时多任务

##### 腔骨龙分时多任务操作系统

![腔骨龙分时多任务操作系统 -- TimesharingOS总体结构](./assets/time-task-multiprog-os-detail.png)

通过上图，大致可以看出相对于CoopOS，TimesharingOS最大的变化是改进了 Trap_handler 内核模块，支持时钟中断，从而可以抢占应用的执行。并通过进一步改进 TaskManager 内核模块，提供任务调度功能，这样可以在收到时钟中断后统计任务的使用时间片，如果任务的时间片用完后，则切换任务。从而可以公平和高效地分时执行多个应用，提高系统的整体效率.

位于 `ch3` 分支上的腔骨龙分时多任务操作系统 – TimesharingOS 的源代码如下所示：

这里

```
./os/src
Rust        18 Files   511 Lines
Assembly     3 Files    82 Lines

├── bootloader
│   └── rustsbi-qemu.bin
├── LICENSE
├── os
│   ├── build.rs
│   ├── Cargo.toml
│   ├── Makefile
│   └── src
│       ├── batch.rs(移除：功能分别拆分到 loader 和 task 两个子模块)
│       ├── config.rs(新增：保存内核的一些配置)
│       ├── console.rs
│       ├── entry.asm
│       ├── lang_items.rs
│       ├── link_app.S
│       ├── linker-qemu.ld
│       ├── loader.rs(新增：将应用加载到内存并进行管理)
│       ├── main.rs(修改：主函数进行了修改)
│       ├── sbi.rs(修改：引入新的 sbi call set_timer)
│       ├── sync
│       │   ├── mod.rs
│       │   └── up.rs
│       ├── syscall(修改：新增若干 syscall)
│       │   ├── fs.rs
│       │   ├── mod.rs
│       │   └── process.rs
│       ├── task(新增：task 子模块，主要负责任务管理)
│       │   ├── context.rs(引入 Task 上下文 TaskContext)
│       │   ├── mod.rs(全局任务管理器和提供给其他模块的接口)
│       │   ├── switch.rs(将任务切换的汇编代码解释为 Rust 接口 __switch)
│       │   ├── switch.S(任务切换的汇编代码)
│       │   └── task.rs(任务控制块 TaskControlBlock 和任务状态 TaskStatus 的定义)
│       ├── timer.rs(新增：计时器相关)
│       └── trap
│           ├── context.rs
│           ├── mod.rs(修改：时钟中断相应处理)
│           └── trap.S
├── README.md
├── rust-toolchain
└── user
    ├── build.py(新增：使用 build.py 构建应用使得它们占用的物理地址区间不相交)
    ├── Cargo.toml
    ├── Makefile(修改：使用 build.py 构建应用)
    └── src
        ├── bin(修改：换成第三章测例)
        │   ├── 00power_3.rs
        │   ├── 01power_5.rs
        │   ├── 02power_7.rs
        │   └── 03sleep.rs
        ├── console.rs
        ├── lang_items.rs
        ├── lib.rs
        ├── linker.ld
        └── syscall.rs
```

## 开始搞事情

本章：batch.rs被移除，

1. 应用的加载这部分功能分离出来在 `loader` 子模块中实现，
2. 应用的执行和切换功能则交给 `task` 子模块。

### 多道程序

所有应用的 ELF 格式执行文件都经过 `objcopy` 工具丢掉所有 ELF header 和符号变为二进制镜像文件，随后以同样的格式通过在操作系统内核中嵌入 `link_user.S` 文件

`link_user.S`是储存app在`.data`里面的内容的地址，方便复制到`.test`里面

因为是静态链接,`user/bin`里面的应用被加载到绝对地址。如果，你在复制的时候不按照约定



为啥`user/`没有设置`mv sp ??`：因为应用程序实在Linux系统下的应用程序，他在运行时，已经被分配栈了

也发现了，目前使用的代码没有在`heap`分配的变量/目前还没有使用过堆



