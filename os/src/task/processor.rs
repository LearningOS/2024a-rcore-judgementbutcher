//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::config::MAX_SYSCALL_NUM;
use crate::mm::{MapPermission, PageTableEntry, VPNRange, VirtAddr, VirtPageNum};
use crate::sync::UPSafeCell;
use crate::timer::get_time_ms;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }

    ///get current task's status
    pub fn get_current_task_status(&self) -> TaskStatus {
        let task_inner = self.current.as_ref().unwrap().inner_exclusive_access();
        task_inner.task_status
    }

    ///increase current tasks's syscall times
    pub fn inc_sys_call_time(&mut self, syscall_id: usize) {
       let mut task_inner = self.current.as_mut().unwrap().inner_exclusive_access(); 
       task_inner.task_sys_calls[syscall_id] += 1;
    }

    ///获取当前任务的系统调用情况
    pub fn get_current_task_sys_calls(&self) -> [u32;MAX_SYSCALL_NUM] {
        let task_inner = self.current.as_ref().unwrap().inner_exclusive_access();
        task_inner.task_sys_calls
    }

    ///或者当前任务的开始调度时间
    pub fn get_current_task_start(&self) -> usize {
       let task_inner = self.current.as_ref().unwrap().inner_exclusive_access();
       task_inner.task_start 
    }

    ///获取当前应用的页表
    pub fn get_page_table_entry(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
       let task_inner = self.current.as_ref().unwrap().inner_exclusive_access();
       task_inner.memory_set.translate(vpn) 
    }

    ///将给出的虚拟地址空间加入到地址空间中，自然实现了虚拟地址向物理地址的映射
    pub fn add_new_mem_area(&mut self, start_va: VirtAddr ,end_va: VirtAddr, perm: MapPermission) {
       let mut task_inner = self.current.as_mut().unwrap().inner_exclusive_access();
       task_inner.memory_set.insert_framed_area(start_va, end_va, perm);
    }

    ///将给出的虚拟地址范围从应用地址空间中删除映射关系
    pub fn unmap_mem_area(&mut self, start: usize, len: usize) -> isize{
        //没有给出直接删除一段的函数，那么只能一个一个unmap
        let mut task_inner = self.current.as_mut().unwrap().inner_exclusive_access();
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
}

lazy_static! {
    /// Manage current task's ControlBlock
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            //在这里进行进程的调度，所以设置时间也在这里
            if task_inner.task_begin == false {
                task_inner.task_start = get_time_ms();
                task_inner.task_begin = true;
            }
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
