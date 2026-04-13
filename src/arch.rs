use std::marker::PhantomData;
use std::ops::DerefMut;
use std::ptr::NonNull;
use std::{ops::Deref};
use std::sync::atomic::{AtomicI32};
use std::sync::atomic::Ordering;

use crate::helper_functions::{wait_for_memory, wake_all_by_memory};


struct ArchLock {
    reader_count: AtomicI32, // the amount of readers, do i really have to explain?
    lock_state: AtomicI32, // 0 = no writer, unlocked; 1 = no writer, unlocked, has readers though; 2 = yes writer, locked.
}

impl ArchLock {

    // little reminder: wait_for_memory(ptr, expected) stops the thread for as long as the value in the ptr actually is expected, it DOESN'T wait until the value equates to expected

    fn is_write_locked(&self) -> bool {
        self.lock_state.load(Ordering::Acquire) == 2
    }

    fn reader_count(&self) -> i32 {
        self.reader_count.load(Ordering::Acquire)
    }

    fn free_reader(&self) {
        //AcqRel because gotta check and write at the same time so...
        if self.reader_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            // this *looks* irresponsible, but this code can only run if reader count == 1 prior to the check, which means no writers, and certainly lock state is 1, which is now 0 :D
            self.lock_state.store(0, Ordering::Release);
            wake_all_by_memory(self.lock_state.as_ptr() as *const i32);
        }
    }

    fn request_read(&self) {
        loop {

            // AcqRel because once again, we must read and write at once, better make it certain
            let state = match self.lock_state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(n) => n,
                Err(n) => n,
            };

            // if there is no writer, return.
            if state != 2 {
                self.reader_count.fetch_add(1, Ordering::Release);
                return;
            }

            // if there is a writer, wait until it's actually safe to read...
            wait_for_memory(self.lock_state.as_ptr() as *const i32, state);
        }
    }

    fn request_write(&self) {
        loop {

            // AcqRel because once again, we must read and write at once, better make it certain
            let state = match self.lock_state.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire) {
                Ok(n) => n,
                Err(n) => n,
            };

            // state is only ever 0 if there are no readers and no writer.
            if state == 0 {
                // if code got to this point, reader count is zero and the lock has been acquired for reading :D, returning.
                return
            }

            // if there IS an active writer... just wait bruh.
            wait_for_memory(self.lock_state.as_ptr() as *const i32, state);
        }
    }

    fn free_writer(&self) {
        // attempt to unlock the arch.
        if self.lock_state.compare_exchange(2, 0, Ordering::Release, Ordering::Relaxed).is_ok() {
            // if the arch was in fact locked, notify a possible waiter.
            wake_all_by_memory(self.lock_state.as_ptr() as *const i32);
        }
    }
}

struct InnerArch<T> {
    lock: ArchLock,
    ref_count: AtomicI32,
    value: T
}

impl<T> InnerArch<T> {

    fn new(data: T) -> NonNull<InnerArch<T>>{
        let inner = InnerArch {
            lock: ArchLock { reader_count: AtomicI32::new(0), lock_state: AtomicI32::new(0) },
            ref_count: AtomicI32::new(1),
            value: data
        };
        let boxed = Box::new(inner);
        let raw = Box::into_raw(boxed);
        let nn = NonNull::new(raw);
        nn.expect("What in the name of fuck.")
    }

    // fn swap(&mut self, new_value: &mut T) {
    //     mem::swap(&mut self.value, new_value);
    // }

    fn raise(&self) -> i32{
        self.ref_count.fetch_add(1, Ordering::AcqRel)
    }

    fn lower(&self) -> i32{
        self.ref_count.fetch_sub(1, Ordering::AcqRel)
    }
}

pub struct ArchReader<'a, T> { 
    arch: &'a Arch<T>,
    phantom: PhantomData<*const ()>
}


impl<'a, T> ArchReader<'a, T> {
    fn spin(arch: &'a Arch<T>) -> Self{
        unsafe {
            arch.pointer.as_ref().lock.request_read();
            return Self { arch: arch, phantom: Default::default() }
        }
    }

    pub fn to_writer(self) -> ArchWriter<'a, T>{
        let arch_ref = self.arch;
        std::mem::drop(self);
        return ArchWriter::spin(arch_ref);
    }
}

impl<'a, T> Drop for ArchReader<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.arch.pointer.as_ref().lock.free_reader();
        }
    }
}

impl<'a, T> Deref for ArchReader<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.arch.pointer.as_ref().value
        }
    }
}

pub struct ArchWriter<'a, T> { 
    arch: &'a Arch<T>,
    phantom: PhantomData<*const ()>
}


impl<'a, T> ArchWriter<'a, T> {
    fn spin(arch: &'a Arch<T>) -> Self{
        unsafe {
            arch.pointer.as_ref().lock.request_write();
            return Self { arch: arch, phantom: Default::default() }
        }
    }

    pub fn to_reader(self) -> ArchReader<'a, T> {
        let arch_ref = self.arch;
        std::mem::drop(self);
        return ArchReader::spin(arch_ref);
    }
}

impl<'a, T> Drop for ArchWriter<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.arch.pointer.as_ref().lock.free_writer();
        }
    }
}

impl<'a, T> Deref for ArchWriter<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.arch.pointer.as_ref().value
        }
    }
}

impl<'a, T> DerefMut for ArchWriter<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut (*self.arch.pointer.as_ptr()).value
        }
    }
}

pub struct Arch<T> {
    pointer: NonNull<InnerArch<T>>,
}

unsafe impl<T: Send> Send for Arch<T> {}
unsafe impl<T: Send> Sync for Arch<T> {}

impl<T> Arch<T> {

    pub fn get(&self) -> T where T: Copy {
        *self.reader()
    }

    pub fn get_clone(&self) -> T where T: Clone {
        (*self.reader()).clone()
    }

    pub fn swap(&self, new_value: &mut T) {
        let mut writer = self.writer();
        std::mem::swap(&mut *writer, new_value);
    }

    pub fn meddle<F, R>(&self, f: F) -> R
        where
            F: for<'a> FnOnce(&'a mut T) -> R
    {
        let mut writer = self.writer();
        let result = f(&mut *writer);
        result
    }

    pub fn new(data: T) -> Arch<T> {
        Arch { pointer: InnerArch::new(data) }
    }

    pub fn extend(&self) -> Arch<T> {
        unsafe {
            self.pointer.as_ref().raise();
        }
        Arch { pointer: self.pointer }
    }

    pub fn reader(&self) -> ArchReader<'_, T> {
        // start off Guard as inactive
        return ArchReader::spin(&self);
    }

    pub fn writer(&self) -> ArchWriter<'_, T> {
        // start off Guard as inactive
        return ArchWriter::spin(&self);
    }

    pub fn is_write_locked(&self) -> bool {
        unsafe { self.pointer.as_ref().lock.is_write_locked() }
    }

    pub fn reader_count(&self) -> i32 {
        unsafe { self.pointer.as_ref().lock.reader_count() }
    }
}

impl<T> Drop for Arch<T> { 
    fn drop(&mut self) {
        unsafe {
            if self.pointer.as_ref().lower() <= 1 {
                drop(Box::from_raw(self.pointer.as_ptr()));
            }
        }
    }
}

impl<T> Clone for Arch<T> {
    fn clone(&self) -> Self {
        self.extend()
    }
}