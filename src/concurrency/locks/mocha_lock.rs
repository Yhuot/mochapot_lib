use std::marker::PhantomData;
use std::ops::DerefMut;
#[cfg(feature = "async")]
use std::{future::Future, pin::Pin};
use std::ptr::NonNull;
use std::{ops::Deref};
use std::sync::atomic::{AtomicI32, Ordering, AtomicUsize};
#[cfg(feature = "serde")]
use serde::{Serialize, Serializer, Deserialize, Deserializer};

use crate::helper_functions::{wait_for_memory, wake_all_by_memory};


/*

Alright, gather around, no one is even supposed to be reading this, but a certain someone insisted i documented this abomination properly.

This lock relies on it's state, ref counting, etc, it's almost simple:

explaining the state:
    reader_count: AtomicI32 refers to the amount of ACTIVE readers, if this is anything other than 0, no writing is happening, EVER.
    transitioning_reader_count: 
                                AtomicI32 refers to the amount of inactive readers trying to become writers, if this is above 0,
                                both free_writer and free_reader will set state to 3 instead of 0, signaling for the waiting transitions to continue, one at a time.
    lock_state: 
                                AtomicI32 refers to the current state of the lock, 
                                a state of 0 means nothing is happening, no surprises there, 
                                a state of 1 means there are 1->n readers active currently.
                                a state of 2 means there is ONE writer, new readers cannot spawn, and no active readers should exist.
                                a state of 3 means that either a THE LAST reader or THE ONLY writer got freed while there were still 
                                inactives trying to be upgraded, this is a signal for them to upgrade one at a time and become writers.

this lock has not been proven 100% correct, because i simply cannot prove such thing, use at your own risk, and if you'll use it,
    use the public API, the MochaLock::new(), reader(), writer(), meddle(), observe(), get(), etc, the inner workings should not even
    be exposed, how are you even reading this?

explanation on the public documentation: Yes, i have used AI to make the public documentation, stone me, shoot me, skin me, i don't care,
i am a lazy bitch, if i find something to eb wrongly documented, i will edit it, but honestly, documentation is for smart people, not me.

HOWEVER, none of the actual logic and code are vibe coded, for no Artificial Intelligence could possibly hope to stand a chance against my glorious Genuine Stupidity.

*/

struct MochaLockLock {
    reader_count: AtomicUsize,
    lock_state: AtomicUsize, 
    transitioning_reader_count: AtomicUsize,
}

impl MochaLockLock {

    fn free_reader(&self) {
        if self.reader_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            let new_state = if self.transitioning_reader_count.load(Ordering::Acquire) > 0 {3} else {0};
            self.lock_state.store(new_state, Ordering::Release);
            wake_all_by_memory(self.lock_state.as_ptr() as *const i32);
        }
    }

    fn request_read(&self) {
        loop {
            let transitioning_reader_count = self.transitioning_reader_count.load(Ordering::Acquire);
            if transitioning_reader_count == 0 {
                let state = match self.lock_state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(n) => n,
                    Err(n) => n,
                };

                if state < 2 {
                    self.reader_count.fetch_add(1, Ordering::Release);
                    return;
                }

                wait_for_memory(self.lock_state.as_ptr() as *const i32, state as i32);
            }

            wait_for_memory(self.transitioning_reader_count.as_ptr() as *const i32, transitioning_reader_count as i32);
        }
    }

    fn force_read(&self) {
        loop {
            match self.lock_state.compare_exchange(2, 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    self.reader_count.fetch_add(1, Ordering::Release);
                    wake_all_by_memory(self.lock_state.as_ptr() as *const i32);
                    return
                },
                Err(state) => {
                    wait_for_memory(self.lock_state.as_ptr() as *const i32, state as i32);
                },
            }
        }
    }

    fn request_write(&self) {
        loop {
            match self.lock_state.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return,
                Err(n) => {
                    wait_for_memory(self.lock_state.as_ptr() as *const i32, n as i32);
                },
            };
        }
    }

    fn force_write(&self) {
        //println!("Someone tried to upgrade...");
        self.transitioning_reader_count.fetch_add(1, Ordering::Release);
        self.free_reader();
        loop {
            match self.lock_state.compare_exchange(3, 2, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    //println!("State was 3!");
                    self.transitioning_reader_count.fetch_sub(1, Ordering::Release);
                    //println!("Lowered transitioning reader count!");
                    wake_all_by_memory(self.transitioning_reader_count.as_ptr() as *const i32);
                    //println!("Woke up everyone trying to read from it!");
                    return
                },
                Err(state) => {
                    //println!("State is {state}! waiting for that to change :D");
                    wait_for_memory(self.lock_state.as_ptr() as *const i32, state as i32);
                },
            }
        }
    }

    fn free_writer(&self) {
        let next_state = if self.transitioning_reader_count.load(Ordering::Acquire) > 0 {3} else {0};
        if self.lock_state.compare_exchange(2, next_state, Ordering::Release, Ordering::Relaxed).is_ok() {
            wake_all_by_memory(self.lock_state.as_ptr() as *const i32);
        }
    }
}

struct InnerMochaLock<T> {
    lock: MochaLockLock,
    ref_count: AtomicI32,
    value: T
}

impl<T> InnerMochaLock<T> {
    fn new(data: T) -> NonNull<InnerMochaLock<T>>{
        let inner = InnerMochaLock {
            lock: MochaLockLock { reader_count:AtomicUsize::new(0), lock_state:AtomicUsize::new(0), transitioning_reader_count:AtomicUsize::new(0) },
            ref_count: AtomicI32::new(1),
            value: data
        };
        let boxed = Box::new(inner);
        let raw = Box::into_raw(boxed);
        let nn = NonNull::new(raw);
        nn.expect("What in the name of fuck.")
    }

    fn raise(&self) -> i32{
        self.ref_count.fetch_add(1, Ordering::AcqRel)
    }

    fn lower(&self) -> i32{
        self.ref_count.fetch_sub(1, Ordering::AcqRel)
    }
}

/// A read guard for [`MochaLock`].
///
/// This type provides shared (immutable) access to the protected value.
/// Multiple readers may coexist simultaneously, as long as no writer is active.
///
/// The guard releases the read lock when dropped.
pub struct MochaLockReader<'a, T> { 
    mocha_lock: &'a MochaLock<T>,
    active: bool,
    phantom: PhantomData<*const ()>
}

impl<'a, T> MochaLockReader<'a, T> {
    fn spin(mocha_lock: &'a MochaLock<T>) -> Self{
        unsafe {
            mocha_lock.pointer.as_ref().lock.request_read();
            return Self { mocha_lock: mocha_lock, active: true, phantom: Default::default() }
        }
    }

    fn force(mocha_lock: &'a MochaLock<T>) -> Self{
        unsafe {
            mocha_lock.pointer.as_ref().lock.force_read();
            return Self { mocha_lock: mocha_lock, active: true, phantom: Default::default() }
        }
    }

    /// Attempts to upgrade this reader into a writer.
    ///
    /// This operation does **not guarantee immediate promotion**. During the transition:
    /// - Existing readers may still be active.
    /// - Other readers may also request upgrades concurrently.
    ///
    /// However, once the upgrade process begins, no new readers or writers will be allowed
    /// until all pending transitions are resolved.
    ///
    /// Returns a [`MochaLockWriter`] guard once the upgrade completes.
    pub fn to_writer(mut self) -> MochaLockWriter<'a, T>{
        let mocha_lock_ref = self.mocha_lock;
        self.active = false;
        std::mem::drop(self);
        return MochaLockWriter::force(mocha_lock_ref);
    }
}

impl<'a, T> Drop for MochaLockReader<'a, T> {
    fn drop(&mut self) {
        unsafe {
            if self.active { self.mocha_lock.pointer.as_ref().lock.free_reader() }
        }
    }
}

impl<'a, T> Deref for MochaLockReader<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.mocha_lock.pointer.as_ref().value
        }
    }
}

/// A write guard for [`MochaLock`].
///
/// This type provides exclusive (mutable) access to the protected value.
/// While a writer is active, no readers or other writers may access the value.
///
/// The guard releases the write lock when dropped.
pub struct MochaLockWriter<'a, T> { 
    mocha_lock: &'a MochaLock<T>,
    active: bool,
    phantom: PhantomData<*const ()>
}

impl<'a, T> MochaLockWriter<'a, T> {
    fn spin(mocha_lock: &'a MochaLock<T>) -> Self{
        unsafe {
            mocha_lock.pointer.as_ref().lock.request_write();
            return Self { mocha_lock: mocha_lock, active: true, phantom: Default::default() }
        }
    }

    fn force(mocha_lock: &'a MochaLock<T>) -> Self{
        unsafe {
            mocha_lock.pointer.as_ref().lock.force_write();
            return Self { mocha_lock: mocha_lock, active: true, phantom: Default::default() }
        }
    }

    /// Downgrades this writer into a reader.
    ///
    /// This operation atomically transitions from exclusive access to shared access,
    /// allowing other readers to proceed afterward.
    ///
    /// Returns a [`MochaLockReader`] guard.
    pub fn to_reader(mut self) -> MochaLockReader<'a, T> {
        let mocha_lock_ref = self.mocha_lock;
        self.active = false;
        std::mem::drop(self);
        return MochaLockReader::force(mocha_lock_ref);
    }
}

impl<'a, T> Drop for MochaLockWriter<'a, T> {
    fn drop(&mut self) {
        unsafe {
            if self.active { self.mocha_lock.pointer.as_ref().lock.free_writer() }
        }
    }
}

impl<'a, T> Deref for MochaLockWriter<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.mocha_lock.pointer.as_ref().value
        }
    }
}

impl<'a, T> DerefMut for MochaLockWriter<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut (*self.mocha_lock.pointer.as_ptr()).value
        }
    }
}

/// A custom reader-writer lock with support for upgrading readers into writers.
///
/// `MochaLock` allows:
/// - Multiple concurrent readers, or
/// - A single exclusive writer
///
/// It also supports transitioning readers into writers, though this may involve
/// coordination with other upgrading readers.
///
/// # ⚠️ Warning
/// This implementation explicitly states it is not formally proven to be correct.
/// Use with caution in production environments.
pub struct MochaLock<T> {
    pointer: NonNull<InnerMochaLock<T>>,
}

unsafe impl<T: Send> Send for MochaLock<T> {}
unsafe impl<T: Send> Sync for MochaLock<T> {}

impl<T> MochaLock<T> {

    /// Returns a copy of the inner value.
    ///
    /// This acquires a read lock internally.
    ///
    /// Requires `T: Copy`.
    pub fn get(&self) -> T where T: Copy {
        *self.reader()
    }

    /// Returns a cloned copy of the inner value.
    ///
    /// This acquires a read lock internally.
    ///
    /// Requires `T: Clone`.
    pub fn get_clone(&self) -> T where T: Clone {
        (*self.reader()).clone()
    }

    /// Swaps the inner value with the provided one.
    ///
    /// This acquires a write lock.
    pub fn swap(&self, new_value: &mut T) {
        let mut writer = self.writer();
        std::mem::swap(&mut *writer, new_value);
    }

    /// Asynchronously mutates the inner value using the provided function.
    ///
    /// This acquires a write lock and holds it for the duration of the future.
    #[cfg(feature = "async")]
    pub async fn async_meddle<F, R>(&self, f: F) -> R
        where
            F: for<'a> FnOnce(&'a mut T)
                -> Pin<Box<dyn Future<Output = R> + 'a>>,
    {
        let mut writer = self.writer();
        f(&mut *writer).await
    }

    /// Mutates the inner value using the provided function.
    ///
    /// This acquires a write lock for the duration of the closure.
    pub fn meddle<F, R>(&self, f: F) -> R
        where
            F: for<'a> FnOnce(&'a mut T) -> R
    {
        let mut writer = self.writer();
        let result = f(&mut *writer);
        result
    }

    /// Asynchronously reads the inner value using the provided function.
    ///
    /// This acquires a read lock and holds it for the duration of the future.
    #[cfg(feature = "async")]
    pub async fn async_observe<F, R>(&self, f: F) -> R
        where
            F: for<'a> FnOnce(&'a T)
                -> Pin<Box<dyn Future<Output = R> + 'a>>,
    {
        let reader = self.reader();
        f(&*reader).await
    }

    /// Reads the inner value using the provided function.
    ///
    /// This acquires a read lock for the duration of the closure.
    pub fn observe<F, R>(&self, f: F) -> R
        where
            F: for<'a> FnOnce(&'a T) -> R
    {
        let reader = self.reader();
        f(&*reader)
    }

    /// Creates a new `MochaLock` containing the given value.
    pub fn new(data: T) -> MochaLock<T> {
        MochaLock { pointer: InnerMochaLock::new(data) }
    }

    /// Creates another handle to the same underlying lock.
    ///
    /// This increases the internal reference count.
    pub fn extend(&self) -> MochaLock<T> {
        unsafe {
            self.pointer.as_ref().raise();
        }
        MochaLock { pointer: self.pointer }
    }

    /// Acquires a read lock, returning a [`MochaLockReader`].
    pub fn reader(&self) -> MochaLockReader<'_, T> {
        return MochaLockReader::spin(&self);
    }

    /// Acquires a write lock, returning a [`MochaLockWriter`].
    pub fn writer(&self) -> MochaLockWriter<'_, T> {
        return MochaLockWriter::spin(&self);
    }
}

impl<T> Drop for MochaLock<T> { 
    fn drop(&mut self) {
        unsafe {
            if self.pointer.as_ref().lower() <= 1 {
                drop(Box::from_raw(self.pointer.as_ptr()));
            }
        }
    }
}

impl<T> Clone for MochaLock<T> {
    fn clone(&self) -> Self {
        self.extend()
    }
}

impl<T> Default for MochaLock<T> where T: Default {
    fn default() -> Self {
        MochaLock::new(Default::default())
    }
}

#[cfg(feature = "serde")]
impl<T: Serialize> Serialize for MochaLock<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {   
        self.reader().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: Deserialize<'de>> Deserialize<'de> for MochaLock<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(MochaLock::new(T::deserialize(deserializer)?))
    }
}