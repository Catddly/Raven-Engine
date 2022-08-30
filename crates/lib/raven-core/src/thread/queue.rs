//use std::{collections::VecDeque, sync::Mutex};

// Lock-Base Thread Safe Queue.
// TODO: we want a faster version of this.
// 
// When the number of threads increase, the push() and pop() performance may be bad on this implemetation.
// Consider use CAS implementation (lock-free).
//#[deprecated = "Use CAS Version for performance (crossbeam_deque)"]
// struct ThreadSafeQueue<T> {
//     queue: Mutex<VecDeque<T>>,
// }

// Unsafe: Since we ensure ThreadSafeQueue is thread safe, it can be sent to another thread.
//unsafe impl<T> Send for ThreadSafeQueue<T> {}
// Unsafe: Since we ensure ThreadSafeQueue is thread safe, it can be read by multiple thread at the same time.
//unsafe impl<T> Sync for ThreadSafeQueue<T> {}

// impl<T> ThreadSafeQueue<T> {
//     pub fn new() -> Self {
//         Self {
//             queue: Mutex::new(VecDeque::new()),
//         }
//     }

//     pub fn push(&self, v: T) {
//         let mut guard = self.queue.lock().expect("Thread safe queue mutex had been poisoned.");
//         guard.push_back(v);
//     }

//     pub fn pop(&self) -> Option<T> {
//         let mut guard = self.queue.lock().expect("Thread safe queue mutex had been poisoned.");
//         guard.pop_front()
//     }

//     pub fn len(&self) -> usize {
//         let guard = self.queue.lock().expect("Thread safe queue mutex had been poisoned.");
//         guard.len()
//     }

//     pub fn is_empty(&self) -> bool {
//         let guard = self.queue.lock().expect("Thread safe queue mutex had been poisoned.");
//         guard.is_empty()
//     }
// }