use std::{sync::atomic::AtomicBool, thread::{JoinHandle, self}, sync::{atomic::Ordering, Arc}};

use super::ThreadSafeQueue;

pub type Job = dyn FnOnce() -> () + Send + 'static;

pub struct ThreadPool
{
    queue: Arc<ThreadSafeQueue<Box<Job>>>,
    stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
    num_workers: usize,
}

impl ThreadPool
{
    pub fn new() -> Self {
        Self { 
            queue: Arc::new(ThreadSafeQueue::new()),
            stop: Arc::new(AtomicBool::new(false)), 
            handles: Vec::new(),
            num_workers: 4,
        }
    }

    pub fn spawn_workers(&mut self) {
        for index in 0..self.num_workers {
            let stop = self.stop.clone();
            let stop_err = self.stop.clone();
            let queue = self.queue.clone();

            // spawn worker threads and store its handles
            self.handles.push(thread::Builder::new()
            .name(format!("Worker: {}", index))
            .spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    // try pop task from queue
                    if let Some(task) = queue.pop() {
                        task();
                    } else {
                        std::thread::yield_now();
                    }
                }

                println!("Thread {:?} is terminated", std::thread::current().id());
            })
            .unwrap_or_else(move |err| {
                stop_err.store(true, Ordering::Release);
                panic!("Failed to create worker threads for thread pool! with {}", err);
            }));
        }
    }

    pub fn terminate(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn terminate_block(self) {
        self.stop.store(true, Ordering::Release);

        for handle in self.handles {
            handle.join().expect("Worker thread had been poisoned!");
        }
    }
}

impl Default for ThreadPool
{
    fn default() -> Self {
        let num_workers = num_cpus::get() / 2;

        Self {
            queue: Arc::new(ThreadSafeQueue::new()),
            stop: Arc::new(AtomicBool::new(false)),
            handles: Vec::new(),
            num_workers,
        }
    }
}