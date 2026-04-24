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

writer_state -> -1..n

    if writer_state is 0, no queued writer access;
    if writer_state is > 0, aka n, there are n queued writers.
    if writer_state is -1, this is an unlikely but possible case, though priority is for writers, this was the only way to avoid a clusterfuck level fuckass lowkeynuenely annoying problem.

    the way this works is, upon attempting to acquire writer access, a writer will flip the reader_state, making it so readers cannot be acquired.
    then, it will add to writer_state with a fetch_add(1, AcqRel), whatever that function returns, is added to 1, re result of that
    will be the request ID, from then, the writer will wait on memory for writer_state to change, every time it changes,
    the ID will also be lowered on the waiting thread, when it reaches 0, the thread attempts to acquire a lock.


*/

struct MochaLockLock {
    reader_state: AtomicUsize,
    writer_state: AtomicIsize,
}

impl MochaLockLock {

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
            match self.writer_state.compare_exchange(0, -1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    // if the lock was, in fact, free of writer intent
                    // we add the reader to the pile:
                    let res = self.reader_state.fetch_update(Ordering::AcqRel, Ordering::Acquire,
                        |state|{
                            if state == 0 {
                                // if 0, return 2, meaning first reader!
                                Some(2)
                            } else if state == 1 {
                                // shouldn't be possible, but still, i might be insane.
                                None
                            } else {
                                // if > 1, increase reader count :D
                                Some(state+1)
                            }
                        }
                    );
                    // we then set the writer state back to what we found:
                    self.writer_state.store(0, Ordering::Release);
                    // and also wake all mfs that were waiting on that -1 we set before.
                    wake_all_by_memory(self.writer_state.as_ptr() as *const i32);
                    match res {
                        Ok(_) => {
                            // if code got to this point, reader suceeded.
                            // we can then return and allow the reader to be acquired
                            return
                        },
                        Err(_) => {
                            // again, shouldn't be possible, but if the cpu get's whacky or some random cosmic ray decided to flip a bit or idk, maybe the universe itself wanted this to happen:
                            // we continue the loop to try again.
                            continue
                        },
                    }
                },
                Err(value) => {
                    // if there IS writer intent, we fookin wait.
                    wait_for_memory(self.writer_state.as_ptr() as *const i32, value as i32);
                },
            }
        }
    }

    // can only be called from state = 1, therefore, easy.
    fn force_read(&self) {
        self.reader_state.store(2, Ordering::Release);
    }

    fn try_start_write_intent(&self) -> isize {
        match self.writer_state.fetch_update(
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
        }
    }

    // can be called from ANY state
    fn request_write(&self, conversion: bool) {
        // by doing this we both acquire an ID and signify writer intent.
        let mut counter = self.try_start_write_intent();
        if conversion {
            self.free_reader();
        }
        loop {
            if counter == -1 {
                counter = self.try_start_write_intent();
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