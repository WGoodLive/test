### 待定

```asm
	csrr 拿csr寄存器哦
	bnez 与0比大小
	li 加载立即数
	sw 放内存
	ld 加载到寄存器
	auipc pc加高位寄存器
	tail 尾调用 = PC加高位寄存器，再跳转
```

shadow stack，来保证函数返回时不会被恶意程序控制导致跳错位置。Clang此前支持shadow stack，被人报了问题之后就被移除了；GCC则从来没支持过

shadow是存的用户内存映射，无法访问内核映射，但是也不直接暴露给用户（我猜的）

[15题，看看Linux怎么写的](https://rcore-os.cn/rCore-Tutorial-Book-v3/chapter3/5exercise.html)

## 开始新篇章

