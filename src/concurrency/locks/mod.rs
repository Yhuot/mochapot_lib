mod mocha_lock;
mod mocha_lock_new;
mod coffee_lock;
use std::sync::atomic::{AtomicI32, Ordering};

pub use mocha_lock::MochaLock;
pub use coffee_lock::CoffeeLock;

use crate::helper_functions::wake_all_by_memory;

struct ImportantResult<'a, T> {
    lock: &'a AtomicI32,
    value: T
}

impl<T> ImportantResult<'_, T> {
    pub fn peek(&self) -> &T {
        &self.value
    }

    pub fn extract(self) -> T {
        self.lock.store(0, Ordering::Release);
        wake_all_by_memory(self.lock.as_ptr() as *const i32);
        self.value
    }
}