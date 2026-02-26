use std::{collections::HashMap, mem, ops::{Deref, DerefMut}, ptr, sync::{Arc, LazyLock, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}}, thread::{self, Thread, ThreadId}};

static THREAD_COUNTER: AtomicU64 = AtomicU64::new(1);
static THREAD_MAP: LazyLock<Arc<Mutex<HashMap<ThreadId, u64>>>> = LazyLock::new(
    || {
        let map = Arc::new(Mutex::new(HashMap::new()));
        map.lock().unwrap().insert(thread::current().id(), 1);
        map
    }
);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct MochaThread<T> {
    // 0 -> core
    // 1 -> join handler
    // 2 -> is running
    // 3 -> thread id
    // 4 -> can join
    data: *mut (Thread, Option<thread::JoinHandle<T>>, AtomicBool, u64, AtomicBool),
}

impl<T> MochaThread<T> {
    pub fn num_id(&self) -> u64 {
        unsafe {
            (*self.data).3
        }
    }

    pub fn is_running(&self) -> bool {
        unsafe {
            (*self.data).2.load(Ordering::SeqCst)
        }
    }

    pub fn can_join(&self) -> bool {
        unsafe {
            (*self.data).1.is_some()
        }
    }

    pub fn join(self) -> T {
        unsafe {
            if (*self.data).4.swap(false, Ordering::SeqCst) {
                let join_handle = ptr::read(&(*self.data).1);
                (*self.data).1 = None;
                join_handle.expect("Cannot join MochaThread without a handle!").join().unwrap()
            } else {
                panic!("MochaThread has already been joined or cannot be joined!");
            }
        }
    }
}

impl<T> Deref for MochaThread<T> {
    type Target = Thread;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &(*self.data).0
        }
    }
}

impl<T> DerefMut for MochaThread<T>{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut (*self.data).0
        }
    }
}

pub fn current() -> MochaThread<()> {
    let mut map = THREAD_MAP.lock().unwrap();
    let thread_id: u64 = map.get(&thread::current().id()).cloned().unwrap_or_else(
        || {
            let id =  THREAD_COUNTER.fetch_add(1, Ordering::SeqCst);
            map.insert(thread::current().id(), id.clone());
            id
        }
    );

    let e = Box::new((thread::current(), None, AtomicBool::new(true), thread_id, AtomicBool::new(false)));
    MochaThread {
        data: Box::into_raw(e),
    }
}  

pub fn spawn<F, T>(f: F) -> MochaThread<T> where F: (FnOnce() -> T) + Send + 'static, T: Send + 'static {

    let thread_id: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let dummy_thread_id = thread_id.clone();

    let thread_core: Arc<Mutex<Option<thread::Thread>>> = Arc::new(Mutex::new(None));
    let dummy_thread_core = thread_core.clone();

    let running: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let dummy_running = running.clone();


    let join_handle = thread::spawn(
        move || {
            
            dummy_thread_id.store(THREAD_COUNTER.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
            THREAD_MAP.lock().unwrap().insert(thread::current().id(), dummy_thread_id.load(Ordering::SeqCst));

            *(dummy_thread_core.lock().unwrap()) = Some(thread::current());

            dummy_running.store(true, Ordering::SeqCst);

            mem::drop(dummy_thread_id);
            mem::drop(dummy_thread_core);
            mem::drop(dummy_running);

            let result = f();

            unsafe {(*current().data).2.store(false, Ordering::SeqCst)};

            result
        }
    );


    let e = Box::new((Arc::try_unwrap(thread_core).unwrap().into_inner().unwrap().unwrap(), Some(join_handle), Arc::try_unwrap(running).unwrap(), thread_id.load(Ordering::SeqCst), AtomicBool::new(true)));
    MochaThread{ data: Box::into_raw(e) }
}