## 功能实现

1. task.rs

   在`TaskControlBlock`中添加了以下内容：

   + `pub task_sys_calls: [u32; MAX_SYSCALL_NUM]`用来记录当前任务的系统调用数
   + `pub task_start: usize`：用来记录任务的开始调度时间
   + `pub task_begin: bool`：判断该任务是否第一次调度

2. syscall/mod.rs

   添加了一行`TASK_MANAGER.inc_sys_call_time(syscall_id)`用来增加系统调用数

3. task/mod.rs

   初始化实例时初始化新增内容，为TASK_MANAGER实现了几个函数，`get_current_task_status`、`inc_sys_call_time`、`get_current_task_sys_calls`、`get_current_task_start`，用来获取修改Taskinfo所需要的信息

   在任务第一次被调度时设置`task_start`和`task_begin`



## 简答作业

1. rustsbi版本：0.3.0-alpha.2。

+ bad_address尝试向0x0地址写入内容，而该地址是不能被应用使用的。
+ bad_instruction中sret是当CPU完成Trap处理准备返回的时候执行的指令，而当前程序是用户态，不能使用该指令
+ bad_register中尝试获取sstatus的值，但是sstatus是supervisor模式下的csr寄存器，在user模式下不能直接访问

2. 

   1. 此时a0代表的值是分配Trap上下文之后的内核栈栈顶。_restore的作用是恢复上下文，那么可以在系统调用返回时使用，也可以在应用程序切换的时候使用
   2. 特殊处理了三个CSR：sstatus、sepc、sscratch。sstatus的spp字段记录了Trap发生前CPU处在哪个特权级，在这里spp字段的值是U，代表Trap发生前处于用户态；sepc记录的是Trap发生之前执行的最后一条指令的地址，在这里的作用是指示返回用户态后该执行的指令地址；sscratch在这里保存的是用户栈的栈顶地址，在进入用户态后用于标识。
   3. 对于x4（tp）寄存器，在_alltrap中不会被使用到，不需要保存，也就不需要恢复；对于x2（sp），我们需要用到它原来的值来进行用户栈和内核栈的交换，因此不需要保存，也不需要恢复
   4. 该指令是交换二者的值，在_alltrap中，sp被修改为指向内核栈，而sscratch指向用户栈，这里进行交换，现在sp重新指向用户栈栈顶，sscratch指向内核栈栈顶
   5. 状态切换发生在sret指令，sret具有从内核态到用户态的执行环境切换能力。
   6. 该指令是交换sscratch和sp的值，在这一行之前，sp指向用户栈，sscratch指向内核栈栈顶，现在，sp指向内核栈，sscratch指向用户栈栈顶
   7. 从U态进入S态的指令是`call trap_handler`，这条指令调用trap_handler函数，在trap_handler函数中，会继续调用syscall，如果`scause.cause()`是`Exception::UserEnvCall`，也就是`ecall`，在这里调用具体的系统调用函数，`ecall`指令导致进入内核态。

   

3. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

   我的同学zz关于task调度时间计算的问题

4. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

   https://riscv.org/wp-content/uploads/2017/05/riscv-privileged-v1.10.pdf

5. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

6. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。