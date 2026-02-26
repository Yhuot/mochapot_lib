#[allow(unused_imports)]
use windows_sys::Win32::System::Threading::INFINITE;
#[allow(unused_imports)]
use windows_sys::Win32::System::Threading::WaitOnAddress;
#[allow(unused_imports)]
use windows_sys::Win32::System::Threading::WakeByAddressSingle;
#[allow(unused_imports)]
use windows_sys::Win32::System::Threading::WakeByAddressAll;

use libc::{syscall, SYS_futex};

use linux_raw_sys::general::{
    FUTEX_WAIT, FUTEX_WAKE,
};

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn futex_wait(addr: *const i32, val: i32) {
    syscall(
        SYS_futex,
        addr,
        FUTEX_WAIT,
        val,
        std::ptr::null::<libc::timespec>(),
    );
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn futex_wake(addr: *const i32, n: i32) {
    syscall(
        SYS_futex,
        addr,
        FUTEX_WAKE,
        n,
    );
}

#[cfg(target_os = "linux")]
pub fn wait_for_memory(addr: *const i32, expected: i32) {
    unsafe {
        futex_wait(
            addr,
            expected,
        )
    }
}

#[cfg(target_os = "windows")]
pub fn wait_for_memory(addr: *const i32, expected: i32) {
    unsafe {
        WaitOnAddress(
            addr as *const _,
            &expected as *const _ as *const _,
            4,
            INFINITE,
        );
    }
}

#[cfg(target_os = "linux")]
pub fn wake_by_memory(addr: *const i32, n: i32) {
    unsafe {
        futex_wake(
            addr,
            n,
        )
    }
}

#[cfg(target_os = "windows")]
pub fn wake_by_memory(addr: *const i32, n: i32) {
    unsafe {
        WakeByAddressSingle(
            addr as *const _
        );
    }
}

#[cfg(target_os = "linux")]
pub fn wake_all_by_memory(addr: *const i32) {
    unsafe {
        use std::i32;

        futex_wake(
            addr,
            i32::MAX,
        )
    }
}

#[cfg(target_os = "windows")]
pub fn wake_all_by_memory(addr: *const i32, n: i32) {
    unsafe {
        WakeByAddressAll(
            addr as *const _
        );
    }
}