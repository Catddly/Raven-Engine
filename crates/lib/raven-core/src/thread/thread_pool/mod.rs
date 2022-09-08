mod job;
mod worker;

use std::{
    sync::atomic::AtomicBool,  
    sync::{atomic::Ordering, Arc},
};

use crossbeam_deque::{Injector as GlobalQueue};
use job::Job;
use worker::Worker;

pub use job::JobHandle;

pub struct ThreadPool {
    /// Shared by all the worker threads.
    /// Worker thread can steal jobs from this queue.
    global_queue: Arc<GlobalQueue<Job>>,
    /// Used to control the all the worker threads.
    stop: Arc<AtomicBool>,
    /// All the worker threads.
    workers: Vec<Worker>,
}

impl ThreadPool {
    /// Create a new thread pool with four worker threads.
    pub fn new(num_workers: usize) -> Self {
        assert!(num_workers <= num_cpus::get());

        let workers = Vec::with_capacity(num_workers);

        Self { 
            global_queue: Arc::new(GlobalQueue::new()),
            stop: Arc::new(AtomicBool::new(false)), 
            workers,
        }
    }

    /// Spawn the worker threads.
    /// Until you call this function, no thread will be created by the thread pool.
    /// If you call this function while the thread pool is running, you will terminate the old worker threads and spawn new workers.
    pub fn spawn_workers(&mut self) {
        if !self.workers.is_empty() {
            self.terminate_block();
        }
        self.workers.clear();

        // spawn workers
        for i in 0..self.workers.capacity() {
            let global = self.global_queue.clone();

            // spawn worker threads and store its handles
            let worker = Worker::new(global, format!("Worker {}", i));
            self.workers.push(worker);
        }

        // setup coworkers env and launch worker threads
        for i in 0..self.workers.capacity() {
            let stop = self.stop.clone();

            let stealers: Vec<_> = self.workers.iter()
                .filter(|w| *w.name() != format!("Worker {}", i))
                .map(|w| w.stealer())
                .collect();

            self.workers[i].launch(stealers, stop);
        }
    }

    /// Add jobs to the thread pool which will be consumed by the worker threads.
    pub fn add_job<F>(&self, f: F) -> JobHandle
    where
        F : FnOnce() -> () + Send + 'static,
    {
        assert!(!self.workers.is_empty(), "No worker threads in this thread pool!");

        let job = Job::new(Box::new(f));
        let job_handle = job.handle();
        self.global_queue.push(job);
        job_handle
    }

    /// Try pop one job from the thread pool and execute it in current thread.
    /// This can be useful to avoid some deadlock scenarios when some tasks are waiting other tasks to finish,
    /// Or can help mitigate the burden of the thread pool.
    pub fn help_once(&mut self) {
        // try pop task from global queue
        if let Some(ref mut task) = self.global_queue.steal().success() {
            task.execute();
        }
    }

    /// Terminate all worker threads in the thread pool.
    /// This function will not interupt the thread, thread will terminate until current work is done.
    pub fn terminate(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Terminate all worker threads in the thread pool.
    /// This function will not interupt the thread, thread will terminate until current work is done.
    /// And this function will wait until all the worker threads are joined. (i.e. this function will block the thread who called this function until current jobs are done)
    pub fn terminate_block(&mut self) {
        self.stop.store(true, Ordering::SeqCst);

        let handles = self.workers.drain(..);

        for handle in handles {
            handle.terminate();
        }
        // just forget about the rest of the jobs here
    }

    /// Terminate all worker threads in the thread pool.
    /// This function will block the thread who called this function and wait all the jobs are done.
    pub fn terminate_until_finished(&mut self) {
        while !self.global_queue.is_empty() {
            self.help_once();
        }

        loop {
            let mut still_working = false;

            for worker in &self.workers {
                if !worker.is_finished() {
                    still_working |= true;
                    break;
                }
            }

            if !still_working {
                break;
            } else {
                self.help_once();
            }
        }

        self.terminate_block();
    }
}

impl Default for ThreadPool {
    fn default() -> Self {
        let num_workers = num_cpus::get() / 2;

        let workers = Vec::with_capacity(num_workers);

        Self {
            global_queue: Arc::new(GlobalQueue::new()),
            stop: Arc::new(AtomicBool::new(false)),
            workers,
        }
    }
}