#[cfg(feature = "async")]
use std::pin::Pin;

use std::{marker::PhantomData, ops::{Deref, DerefMut}, ptr::NonNull, sync::atomic::{AtomicIsize, AtomicUsize, Ordering}};

#[cfg(feature = "serde")]
use serde::{Serialize, Serializer, Deserialize, Deserializer};

use crate::helper_functions::{wait_for_memory, wake_all_by_memory};

/*

This one is significantly simpler than it's siblings (except BeanLock):

state -> -n..n

if state is 0, no readers or writers;
if state is 1, there is a writer;
if state is -1, there is at least one writer queued;
if state is > 1, aka n, there are n-1 readers;
if state is < -1, aka n, there are (-n)-1 readers waiting to be dropped before writers can start.
*/

struct CoffeeLockLock {
    state: AtomicIsize
}

impl CoffeeLockLock {
    // can only be called from state > 1 OR state < 0.
    fn free_reader(&self) {
        match self.state.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| if state.abs() == 2 {Some(0)} else {None} ) {
            Ok(_) => {wake_all_by_memory(self.state.as_ptr() as *const i32);},
            Err(state) => {
                if state > 0 {
                    self.state.fetch_sub(1, Ordering::Release);
                } else {
                    self.state.fetch_add(1, Ordering::Release);
                }
            },
        }
    }

    // can be called from ANY state.
    fn request_read(&self) {
        loop {
            match self.state.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return,
                Err(result) => {
                    match result
                    {
                        1 => {
                            // if there is a writer, wait.
                            wait_for_memory(self.state.as_ptr() as *const i32, 1);
                            continue
                        },
                        n if n > 1  => {
                            // if there are more readers, just add to the pile :D
                            match self.state.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| if state > 0 {Some(state+1)} else {None})
                            // why update? because between the condition passing as true, and this actual call, a writer can decide to flip everything upside down, a simple fetch_add() would break the invariants.
                            {
                                Ok(_) => {
                                    // if we get here, means it was positive, meaning no degladiating writers, we can proceed
                                    return
                                },
                                Err(state) => {
                                    // if it fails, means a writer requested writing access right after the condition passed as true, this aspiring reader must now wait.
                                    wait_for_memory(self.state.as_ptr() as *const i32, state as i32);
                                    continue
                                },
                            }
                        },
                        n => {
                            // if state is negative, wait until it's not.
                            wait_for_memory(self.state.as_ptr() as *const i32, n as i32);
                            continue
                        }
                    }
                },
            }
        }
    }

    // can only be called from state = 1, therefore, easy.
    fn force_read(&self) {
        self.state.store(2, Ordering::Release);
    }

    // can be called from ANY state
    fn request_write(&self) {
        // why we do this? to tell any readers to wait until the writers are done with their thing.
        _ = self.state.fetch_update(
            Ordering::AcqRel, 
            Ordering::Acquire, 
            |state| Some(-(state.abs()))
        );
        loop {
            match self.state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return,
                Err(n) => {
                    wait_for_memory(self.state.as_ptr() as *const i32, n as i32);
                },
            };
        }
    }

    // can only be called from state = 1
    fn free_writer(&self) {
        self.state.store(0, Ordering::Release);
        wake_all_by_memory(self.state.as_ptr() as *const i32);
    }
}


struct InnerCoffeeLock<T> {
    lock: CoffeeLockLock,
    ref_count: AtomicUsize,
    value: T
}

impl<T> InnerCoffeeLock<T> {
    fn new(data: T) -> NonNull<InnerCoffeeLock<T>>{
        let inner = InnerCoffeeLock {
            lock: CoffeeLockLock { state: AtomicIsize::new(0) },
            ref_count: AtomicUsize::new(1),
            value: data
        };
        let boxed = Box::new(inner);
        let raw = Box::into_raw(boxed);
        let nn = NonNull::new(raw);
        nn.expect("What in the name of fuck.")
    }

    fn raise(&self) -> usize{
        self.ref_count.fetch_add(1, Ordering::AcqRel)
    }

    fn lower(&self) -> usize{
        self.ref_count.fetch_sub(1, Ordering::AcqRel)
    }
}

/// A read guard for [`CoffeeLock`].
///
/// This type provides shared (immutable) access to the protected value.
/// Multiple readers may coexist simultaneously, as long as no writer is active.
///
/// The guard releases the read lock when dropped.
pub struct CoffeeLockReader<'a, T> { 
    coffee_lock: &'a CoffeeLock<T>,
    phantom: PhantomData<*const ()>
}

impl<'a, T> CoffeeLockReader<'a, T> {
    fn spin(coffee_lock: &'a CoffeeLock<T>) -> Self{
        unsafe {
            coffee_lock.pointer.as_ref().lock.request_read();
            return Self { coffee_lock: coffee_lock, phantom: Default::default() }
        }
    }

    fn force(coffee_lock: &'a CoffeeLock<T>) -> Self{
        unsafe {
            coffee_lock.pointer.as_ref().lock.force_read();
            return Self { coffee_lock: coffee_lock, phantom: Default::default() }
        }
    }
}

impl<'a, T> Drop for CoffeeLockReader<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.coffee_lock.pointer.as_ref().lock.free_reader()
        }
    }
}

impl<'a, T> Deref for CoffeeLockReader<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.coffee_lock.pointer.as_ref().value
        }
    }
}

/// A write guard for [`CoffeeLock`].
///
/// This type provides exclusive (mutable) access to the protected value.
/// While a writer is active, no readers or other writers may access the value.
///
/// The guard releases the write lock when dropped.
pub struct CoffeeLockWriter<'a, T> { 
    coffee_lock: &'a CoffeeLock<T>,
    active: bool,
    phantom: PhantomData<*const ()>
}

impl<'a, T> CoffeeLockWriter<'a, T> {
    fn spin(coffee_lock: &'a CoffeeLock<T>) -> Self{
        unsafe {
            coffee_lock.pointer.as_ref().lock.request_write();
            return Self { coffee_lock: coffee_lock, active: true, phantom: Default::default() }
        }
    }

    /// Downgrades this writer into a reader.
    ///
    /// Returns a [`CoffeeLockReader`] guard.
    pub fn to_reader(mut self) -> CoffeeLockReader<'a, T> {
        let coffee_lock_ref = self.coffee_lock;
        self.active = false;
        std::mem::drop(self);
        return CoffeeLockReader::force(coffee_lock_ref);
    }
}

impl<'a, T> Drop for CoffeeLockWriter<'a, T> {
    fn drop(&mut self) {
        unsafe {
            if self.active { self.coffee_lock.pointer.as_ref().lock.free_writer() }
        }
    }
}

impl<'a, T> Deref for CoffeeLockWriter<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.coffee_lock.pointer.as_ref().value
        }
    }
}

impl<'a, T> DerefMut for CoffeeLockWriter<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut (*self.coffee_lock.pointer.as_ptr()).value
        }
    }
}

/// A custom reader-writer lock.
///
/// `CoffeeLock` allows:
/// - Multiple concurrent readers, or
/// - A single exclusive writer
/// 
/// Writers take priority over incoming readers by transitioning the lock into a
/// "writer intent" phase. During this phase, new readers are blocked until all
/// existing readers have exited and a writer acquires the lock.
/// 
/// This implementation does NOT support atomic reader-to-writer upgrades.
/// 
/// This implementation does not have fairness, or queues, Readers and Writers will always race for access.
///
/// # ⚠️ Warning
/// This implementation explicitly states it is not formally proven to be correct.
/// Use with caution in production environments.
pub struct CoffeeLock<T> {
    pointer: NonNull<InnerCoffeeLock<T>>,
}

unsafe impl<T: Send> Send for CoffeeLock<T> {}
unsafe impl<T: Send> Sync for CoffeeLock<T> {}

impl<T> CoffeeLock<T> {

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

    /// Creates a new `CoffeeLock` containing the given value.
    pub fn new(data: T) -> CoffeeLock<T> {
        CoffeeLock { pointer: InnerCoffeeLock::new(data) }
    }

    /// Creates another handle to the same underlying lock.
    ///
    /// This increases the internal reference count.
    pub fn extend(&self) -> CoffeeLock<T> {
        unsafe {
            self.pointer.as_ref().raise();
        }
        CoffeeLock { pointer: self.pointer }
    }

    /// Acquires a read lock, returning a [`CoffeeLockReader`].
    /// 
    /// Readers may be blocked not only by an active writer, but also when a writer has
    /// requested access and is waiting for existing readers to drain.
    pub fn reader(&self) -> CoffeeLockReader<'_, T> {
        return CoffeeLockReader::spin(&self);
    }

    /// Acquires a write lock, returning a [`CoffeeLockWriter`].
    /// When a writer requests access, the lock enters a "writer intent" phase, preventing
    /// new readers from acquiring the lock until the writer has completed.
    pub fn writer(&self) -> CoffeeLockWriter<'_, T> {
        return CoffeeLockWriter::spin(&self);
    }
}

impl<T> Drop for CoffeeLock<T> { 
    fn drop(&mut self) {
        unsafe {
            if self.pointer.as_ref().lower() <= 1 {
                drop(Box::from_raw(self.pointer.as_ptr()));
            }
        }
    }
}

impl<T> Clone for CoffeeLock<T> {
    fn clone(&self) -> Self {
        self.extend()
    }
}

impl<T> Default for CoffeeLock<T> where T: Default {
    fn default() -> Self {
        CoffeeLock::new(Default::default())
    }
}

#[cfg(feature = "serde")]
impl<T: Serialize> Serialize for CoffeeLock<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {   
        self.reader().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: Deserialize<'de>> Deserialize<'de> for CoffeeLock<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(CoffeeLock::new(T::deserialize(deserializer)?))
    }
}