use std::{marker::PhantomData, sync::atomic::{AtomicI32, Ordering}};

use crate::helper_functions::{wait_for_memory, wake_all_by_memory};

/// important warning: this fuckass blocker does not free itself on drop, I REPEAT: YOU CAN PERMALOCK YOURSELF! WHY DO YOU WANT THIS? i have no clue why you'd even use this, but don't! it's that simple! do not use this shit! i don't even remember why i made this thing.
pub struct UnsafeBlocker {
    // ref count, state, state==1 = blocked
    inner: *mut (AtomicI32, AtomicI32)
}

impl Clone for UnsafeBlocker {
    fn clone(&self) -> Self {
        self.extend()
    }
}

impl UnsafeBlocker {
    
    pub fn new() -> UnsafeBlocker {
        UnsafeBlocker { inner: Box::into_raw(Box::new((AtomicI32::new(1), AtomicI32::new(0)))) }
    }

    /// same shit as clone, just use clone.
    pub fn extend(&self) -> UnsafeBlocker {
        unsafe { (*self.inner).0.fetch_add(1, Ordering::Release); }
        UnsafeBlocker { inner: self.inner }
    }

    pub fn drop (self) {}

    pub fn block(&self) -> UnsafeBlockerKey {
        unsafe {
            //add a reference, just for safety
            (*self.inner).0.fetch_add(1, Ordering::Release);
        }
        let mut key = UnsafeBlockerKey { blocker: self.inner, validity: false, _marker: Default::default() };
        unsafe {
            while (*self.inner).1.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
                wait_for_memory((*self.inner).1.as_ptr() as *const i32, 1);
            }
        }
        key.validity = true;
        return key
    }

    pub fn wait(&self) {
        unsafe {
            while (*self.inner).1.load(Ordering::Acquire) != 0 {
                wait_for_memory((*self.inner).1.as_ptr() as *const i32, 1);
            }
        }
    }

    pub fn is_blocked(&self) -> bool {
        unsafe { (*self.inner).1.load(Ordering::Acquire) == 1 }
    }
}

impl Drop for UnsafeBlocker {
    fn drop(&mut self) {
        if unsafe {(*self.inner).0.fetch_sub(1, Ordering::AcqRel )} == 1 {
            unsafe {
                drop(Box::from_raw(self.inner));
            }
        }
    }
}

pub struct UnsafeBlockerKey {
    blocker: *mut (AtomicI32, AtomicI32),
    _marker: PhantomData<*const ()>,
    validity: bool,
}

impl UnsafeBlockerKey {
    pub fn release(self) {
        unsafe {
            if self.validity {
                (*self.blocker).1.store(0, Ordering::Release);
                wake_all_by_memory((*self.blocker).1.as_ptr() as *const i32);
            }
            // just in case the key is the last thing alive who actually remembers the blocker.
            if (*self.blocker).0.fetch_sub(1, Ordering::AcqRel ) == 1 {
                drop(Box::from_raw(self.blocker));
            }
        }
    }
}

pub struct Blocker {
    // ref count, state, state==1 = blocked
    inner: *mut (AtomicI32, AtomicI32)
}

impl Clone for Blocker {
    fn clone(&self) -> Self {
        self.extend()
    }
}

impl Blocker {
    
    pub fn new() -> Blocker {
        Blocker { inner: Box::into_raw(Box::new((AtomicI32::new(1), AtomicI32::new(0)))) }
    }

    /// same shit as clone, just use clone.
    pub fn extend(&self) -> Blocker {
        unsafe { (*self.inner).0.fetch_add(1, Ordering::Release); }
        Blocker { inner: self.inner }
    }

    // this exist for the sole reason of dropping the object without waiting for scope end and without having to writer mem::drop(whatever), utterly useless but nice to have
    pub fn drop (self) {}

    pub fn block(&self) -> BlockerKey {
        unsafe {
            //add a reference, just for safety
            (*self.inner).0.fetch_add(1, Ordering::Release);
        }
        let mut key = BlockerKey { blocker: self.inner, _marker: Default::default(), validity: false };
        unsafe {
            while (*self.inner).1.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
                wait_for_memory((*self.inner).1.as_ptr() as *const i32, 1);
            }
        }
        key.validity = true;
        return key
    }

    pub fn try_block(&self) -> Option<BlockerKey> {
        unsafe {
            //add a reference, just for safety
            (*self.inner).0.fetch_add(1, Ordering::Release);
        }
        let mut key = BlockerKey { blocker: self.inner, _marker: Default::default(), validity: false };
        unsafe {
            if (*self.inner).1.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
                return None
            }else {
                key.validity = true;
                return Some(key)
            }
        }
    }

    pub fn wait(&self) {
        unsafe {
            while (*self.inner).1.load(Ordering::Acquire) != 0 {
                wait_for_memory((*self.inner).1.as_ptr() as *const i32, 1);
            }
        }
    }

    pub fn is_blocked(&self) -> bool {
        unsafe { (*self.inner).1.load(Ordering::Acquire) == 1 }
    }
}

impl Drop for Blocker {
    fn drop(&mut self) {
        if unsafe {(*self.inner).0.fetch_sub(1, Ordering::AcqRel )} == 1 {
            unsafe {
                drop(Box::from_raw(self.inner));
            }
        }
    }
}

pub struct BlockerKey {
    blocker: *mut (AtomicI32, AtomicI32),
    _marker: PhantomData<*const ()>,
    validity: bool
}

impl BlockerKey {
    pub fn release(self) {}
}

impl Drop for BlockerKey {
    fn drop(&mut self) {
        unsafe {
            if self.validity {
                (*self.blocker).1.store(0, Ordering::Release);
                wake_all_by_memory((*self.blocker).1.as_ptr() as *const i32);
            }
            // just in case the key is the last thing alive who actually remembers the blocker.
            if (*self.blocker).0.fetch_sub(1, Ordering::AcqRel ) == 1 {
                drop(Box::from_raw(self.blocker));
            }
        }
    }
}