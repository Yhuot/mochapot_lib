#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{INFINITE, WaitOnAddress, WakeByAddressSingle, WakeByAddressAll};

use libc::{syscall, SYS_futex};

use linux_raw_sys::general::{
    FUTEX_WAIT, FUTEX_WAKE,
};

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn futex_wait(addr: *const i32, expected: i32) {
    loop {
        let res = syscall(
            SYS_futex,
            addr,
            FUTEX_WAIT,
            expected,
            std::ptr::null::<libc::timespec>(),
        );

        if res == 0 {
            // Successfully slept and got woken up
            return;
        }

        let err = *libc::__errno_location();

        match err {
            libc::EINTR => {
                // Interrupted by signal → retry
                continue;
            }
            libc::EAGAIN => {
                // Value changed before sleeping → don't sleep
                return;
            }
            _ => {
                // Real error (invalid addr, etc.)
                panic!("futex_wait failed with errno {}", err);
            }
        }
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn futex_wake(addr: *const i32, n: i32) {
    let res = syscall(
        SYS_futex,
        addr,
        FUTEX_WAKE,
        n,
    );

    if res < 0 {
        let err = *libc::__errno_location();
        panic!("futex_wake failed with errno {}", err);
    }
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
        #[cfg(target_os = "macos")]
        {
            compile_error!("macOS is not supported. Use Linux (<3) or (eugh) Windows.)");
        }
    }
}

#[allow(unused)]
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
        #[cfg(target_os = "macos")]
        {
            compile_error!("macOS is not supported. Use Linux (<3) or (eugh) Windows.)");
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
        #[cfg(target_os = "macos")]
        {
            compile_error!("macOS is not supported. Use Linux (<3) or (eugh) Windows.)");
        }
    }
}