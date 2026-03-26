//! Process management syscalls
use crate::{config::{PAGE_SIZE, TRAP_CONTEXT_BASE}, mm::{MapPermission, PageTable, PTEFlags, VirtAddr, translated_byte_buffer}, task::{TASK_MANAGER, change_program_brk, current_user_token, exit_current_and_run_next, query_syscall_count, suspend_current_and_run_next}, timer::get_time_us};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
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
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let pa_vec = translated_byte_buffer(
        current_user_token(),
        ts as *const u8,
        core::mem::size_of::<TimeVal>()
    );

    let sec = us / 1_000_000;
    let usec = us % 1_000_000;

    let time_val = TimeVal { sec, usec };

    let time_val_bytes = unsafe {
        core::slice::from_raw_parts(
            &time_val as *const _ as *const u8,
            core::mem::size_of::<TimeVal>()
        )
    };

    let mut start = 0;
    for buffer in pa_vec {
        let end = start + buffer.len();
        buffer.copy_from_slice(&time_val_bytes[start..end]);
        start = end;
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token();
    let translate_one = |ptr: *const u8| -> Option<(PTEFlags, &'static mut u8)> {
        let page_table = PageTable::from_token(token);
        let va = VirtAddr::from(ptr as usize);
        let pte = page_table.translate(va.floor())?;
        let flags = pte.flags();
        if !flags.contains(PTEFlags::U) {
            return None;
        }
        let ppn = pte.ppn();
        Some((flags, &mut ppn.get_bytes_array()[va.page_offset()]))
    };
    match trace_request {
        0 => {
            if let Some((flags, byte)) = translate_one(id as *const u8) {
                if flags.contains(PTEFlags::R) {
                    return *byte as isize;
                }
            }
            -1
        }
        1 => {
            if let Some((flags, byte)) = translate_one(id as *const u8) {
                if flags.contains(PTEFlags::W) {
                    *byte = data as u8;
                    return 0;
                }
            }
            -1
        }
        2 => {
            return query_syscall_count(id);
        }
        _ => return -1,
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap");
    if
        start % PAGE_SIZE != 0 ||
        prot & !0x7 != 0 ||
        prot & 0x7 == 0 ||
        len > 10_000usize {
        return -1;
    }

    let end= start + len;
    if end > TRAP_CONTEXT_BASE || start >= end {
        return -1;
    }

    let mut permission = MapPermission::U;
    if prot & 0x1 != 0 {
        permission |= MapPermission::R;
    }
    if prot & 0x2 != 0 {
        permission |= MapPermission::W;
    }
    if prot & 0x4 != 0 {
        permission |= MapPermission::X;
    }
    TASK_MANAGER.insert_framed_area_current(start.into(), end.into(), permission)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    let end = start + len;
    if
        end > TRAP_CONTEXT_BASE ||
        start >= end ||
        start & (PAGE_SIZE -1) != 0||
        len > 10_000usize {
        return -1;
    }

    TASK_MANAGER.uninsert_framed_area_current(start.into(), end.into())
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
