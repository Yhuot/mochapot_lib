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

pub fn wait_for_memory(addr: *const i32, expected: i32) {
    unsafe {
        #[cfg(target_os = "linux")]
        {
            futex_wait(
                addr,
                expected,
            );
            return
        }
        #[cfg(target_os = "windows")]
        {
            WaitOnAddress(
                addr as *const _,
                &expected as *const _ as *const _,
                4,
                INFINITE,
            );
        }
    }
}

pub fn wake_by_memory(addr: *const i32, n: i32) {
    unsafe {
        #[cfg(target_os = "linux")]
        {
            futex_wake(
                addr,
                n,
            )
        }
        #[cfg(target_os = "windows")]
        {
            WakeByAddressSingle(
                addr as *const _
            );
        }
    }
}

pub fn wake_all_by_memory(addr: *const i32) {
    unsafe {
        #[cfg(target_os = "linux")]
        {
            use std::i32;

            futex_wake(
                addr,
                i32::MAX,
            );
            return
        }
        #[cfg(target_os = "windows")]
        {
            WakeByAddressAll(
                addr as *const _
            );
        }
    }
}