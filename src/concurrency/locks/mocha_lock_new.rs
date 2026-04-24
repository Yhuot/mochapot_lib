#[cfg(feature = "async")]
use std::pin::Pin;

use std::{marker::PhantomData, ops::{Deref, DerefMut}, ptr::NonNull, sync::atomic::{AtomicI32, AtomicIsize, AtomicUsize, Ordering}};

#[cfg(feature = "serde")]
use serde::{Serialize, Serializer, Deserialize, Deserializer};

use crate::{concurrency::locks::ImportantResult, helper_functions::{wait_for_memory, wake_all_by_memory}};


/*
reader_state -> 0..n

    if reader_state is 0, no readers or writers;
    if reader_state is 1, there is a writer;
    if reader_state is > 1, aka n, there are n-1 readers;

writer_state -> 0..n

    if writer_state is 0, no queued writer access;
    if writer_state is > 0, aka n, there are n queued writers.

    the way this works is, upon attempting to acquire writer access, a writer will flip the reader_state, making it so readers cannot be acquired.
    then, it will add to writer_state with a fetch_add(1, AcqRel), whatever that function returns, is added to 1, re result of that
    will be the request ID, from then, the writer will wait on memory for writer_state to change, every time it changes,
    the ID will also be lowered on the waiting thread, when it reaches 0, the thread attempts to acquire a lock.


*/

struct MochaLockLock {
    reader_state: AtomicUsize,
    writer_state: AtomicIsize,
    lock_of_importance: AtomicI32 // was gonna use bool, but it's the same space, processor likely doesn't care.
}

impl MochaLockLock {

    fn important<F, T>(&self, operation: F) -> ImportantResult<'_, T> 
        where F: Fn() -> T
    {
        loop {
            match self.lock_of_importance.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    let res = ImportantResult{lock: &self.lock_of_importance, value: operation()};
                    self.lock_of_importance.store(0, Ordering::Release);
                    wake_all_by_memory(self.lock_of_importance.as_ptr() as *const i32);
                    return res
                },
                Err(state) => wait_for_memory(self.lock_of_importance.as_ptr() as *const i32, state),
            }
        }
    }

    // can only be called from reader_state > 1.
    fn free_reader(&self) {
        match self.reader_state.compare_exchange(2, 0, Ordering::AcqRel, Ordering::Acquire) {
            // If Ok(), means this was the last reader, 
            Ok(_) => {wake_all_by_memory(self.reader_state.as_ptr() as *const i32);},
            // if not okay, means there were multiple readers, decrease count and move tf on.
            Err(state) => {self.reader_state.fetch_sub(1, Ordering::Release);},
        }
    }

    // can be called from ANY state.
    fn request_read(&self) {
        loop {
            match self.writer_state.fetch_update(Ordering::AcqRel, Ordering::Acquire, 
                |ws| {
                    if ws == 0 {
                        // if writer_state is 0, that's our greenlight right there.
                        Some(0)
                    } else {
                        None
                    }
                }
            ) {
                Ok(_) => todo!(),
                Err(_) => todo!(),
            }
        }
    }

    // can only be called from state = 1, therefore, easy.
    fn force_read(&self) {
        self.reader_state.store(2, Ordering::Release);
    }

    // can be called from ANY state
    fn request_write(&self, conversion: bool) {
        // by doing this we both acquire an ID and signify writer intent.
        let mut counter = match self.writer_state.fetch_update(
            Ordering::AcqRel, 
            Ordering::Acquire,
            |state| {
                if state == -1 {
                    // if this state is -1, wait until it isn't.
                    // because there is a reader who just decided to read.
                    None
                } else {
                    // if this state is not -1, add to it, and let us continue.
                    Some(state+1)
                }
            }
        ) {
            // if
            Ok(count) => count,
            Err(_) => -1,
        };
        if conversion {
            self.free_reader();
        }
        loop {
            if counter == -1 {

            }
            if counter == 0 {
                loop {
                    match self.reader_state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
                        Ok(_) => {
                            // if it does succeed at acquiring the lock, meaning, no active writers, return.
                            return
                        },
                        Err(reader_count) => {
                            // if it does not, we just wait until there are no readers:
                            wait_for_memory(self.reader_state.as_ptr() as *const i32, reader_count as i32);
                        }
                    }
                }
            }
            wait_for_memory(self.writer_state.as_ptr() as *const i32, counter as i32);
            // if we get to this point, means that the writer state count has changed, meaning we diminish our own, one step closer to 0
            counter -= 1;
        }
    }

    // can only be called from state = 1
    fn free_writer(&self) {
        // free the lock, duh
        self.reader_state.store(0, Ordering::Release);
        // subtract from the writer_state.
        self.writer_state.fetch_sub(1, Ordering::Release);
        wake_all_by_memory(self.writer_state.as_ptr() as *const i32);
        wake_all_by_memory(self.reader_state.as_ptr() as *const i32);
    }
}