//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of `PID_ALLOCATOR` allocates pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod id;
mod manager;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
#[allow(rustdoc::private_intra_doc_links)]
mod task;

use crate::{config::MAX_SYSCALL_NUM, fs::{open_file, OpenFlags}, mm::{MapPermission, PageTableEntry, VPNRange, VirtAddr, VirtPageNum}};
use alloc::sync::Arc;
pub use context::TaskContext;
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use id::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
pub use manager::add_task;
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
    Processor,
};
/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        panic!("All applications completed!");
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    // drop file descriptors
    inner.fd_table.clear();
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

///get current task's status
pub fn get_current_task_status() -> TaskStatus {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.task_status
}

///increase current tasks's syscall times
pub fn inc_sys_call_time(syscall_id: usize) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.task_sys_calls[syscall_id] += 1;
}

///获取当前任务的系统调用情况
pub fn get_current_task_sys_calls() -> [u32;MAX_SYSCALL_NUM] {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.task_sys_calls
}

///或者当前任务的开始调度时间
pub fn get_current_task_start() -> usize {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.task_start 
}

///获取当前应用的页表
pub fn get_page_table_entry(vpn: VirtPageNum) -> Option<PageTableEntry> {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.memory_set.translate(vpn) 
}

///将给出的虚拟地址空间加入到地址空间中，自然实现了虚拟地址向物理地址的映射
pub fn add_new_mem_area(start_va: VirtAddr ,end_va: VirtAddr, perm: MapPermission) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.memory_set.insert_framed_area(start_va, end_va, perm);
}

///将给出的虚拟地址范围从应用地址空间中删除映射关系
pub fn unmap_mem_area(start: usize, len: usize) -> isize{
    //没有给出直接删除一段的函数，那么只能一个一个unmap
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let vpn_st = VirtAddr::from(start).floor();
    let vpn_ed = VirtAddr::from(start + len).ceil();
    let vpn_range = VPNRange::new(vpn_st, vpn_ed);
    for vpn in vpn_range {
        if let Some(pte) = task_inner.memory_set.translate(vpn) {
            if !pte.is_valid() {
                return -1;
            }
            task_inner.memory_set.erase_virt_map(vpn);
        }
    }
    0
}

///设置当前任务的优先级
pub fn set_current_task_prio(prio: isize) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.task_prio = prio;
}


lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("ch6b_initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}
