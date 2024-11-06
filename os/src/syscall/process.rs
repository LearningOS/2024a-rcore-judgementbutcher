//! Process management syscalls

use core::mem::size_of;

use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE}, mm::{translated_byte_buffer, MapPermission, VPNRange, VirtAddr}, task::{
        change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, TASK_MANAGER
    }, timer::{get_time_ms, get_time_us}
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    //we need to write something to an address area
    let dst_vec = translated_byte_buffer(current_user_token(), 
    _ts as *const u8, size_of::<TimeVal>());

    let ref time_val = TimeVal{
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let src_ptr = time_val as *const TimeVal;
    for(idx, dst) in dst_vec.into_iter().enumerate() {
        let unit_len = dst.len();
        unsafe{
            dst.copy_from_slice(core::slice::from_raw_parts(
                src_ptr.wrapping_byte_add(idx * unit_len) as *const u8, 
                unit_len)
            );
        }
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let dst_vec = translated_byte_buffer(current_user_token(), 
    _ti as *const u8, size_of::<TaskInfo>());

    let ref task_info = TaskInfo {
        status: TASK_MANAGER.get_current_task_status(),
        syscall_times: TASK_MANAGER.get_current_task_sys_calls(),
        time: get_time_ms() - TASK_MANAGER.get_current_task_start()
    };
    let src_ptr = task_info as *const TaskInfo;
    for(idx, dst) in dst_vec.into_iter().enumerate() {
        let unit_len = dst.len();
        unsafe {
            dst.copy_from_slice(core::slice::from_raw_parts(
                src_ptr.wrapping_byte_add(idx * unit_len) as *const u8, 
                unit_len));
        }
    }
    0
}

///思路没转变过来，正确的思路应该是如果能在TaskManager里面把事情做完，就不要在外面写逻辑
/// 外面只需要调用内部写好的函数就行

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if _start & (PAGE_SIZE - 1) != 0 
    || _port & !0x7 != 0
    || _port & 0x7 == 0 {
        return -1;
    }
    let vpn_st = VirtAddr::from(_start).floor();
    let vpn_ed= VirtAddr::from(_start + _len).ceil();
    let vpn_range = VPNRange::new(vpn_st, vpn_ed);
    //检查要映射的虚拟地址空间，如果已经有被分配过的，返回-1表示失败
    for vpn in vpn_range {
        if let Some(pte) = TASK_MANAGER.get_page_table_entry(vpn) {
            if pte.is_valid() {
                return -1;
            }
        }
    }
    TASK_MANAGER.add_new_mem_area(
        vpn_st.into(), 
        vpn_ed.into(), 
        MapPermission::from_bits_truncate((_port << 1) as u8) | MapPermission::U);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    if _start &(PAGE_SIZE - 1) != 0 {
        return -1;
    }
    TASK_MANAGER.unmap_mem_area(_start, _len)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
