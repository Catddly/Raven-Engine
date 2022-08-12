mod job;

use std::{
    sync::atomic::AtomicBool, 
    thread::{JoinHandle, self}, 
    sync::{atomic::Ordering, Arc}, collections::VecDeque,
    cell::RefCell,
};

use super::ThreadSafeQueue;
use job::Job;

//pub type JobFunc = job::JobFunc;
pub use job::JobHandle;

thread_local! {
    /// Thread local lazy static work queue for each worker thread in thread pool.
    /// If this thread is not a worker thread, its LOCAL_QUEUE will be None.
    static LOCAL_QUEUE: RefCell<Option<VecDeque<Job>>> = RefCell::new(None);
}

pub struct ThreadPool
{
    /// Shared by all the worker threads.
    /// Worker thread can steal jobs from this queue.
    global_queue: Arc<ThreadSafeQueue<Job>>,
    stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
    num_workers: usize,
}

impl ThreadPool
{
    /// Create a new thread pool with four worker threads.
    pub fn new(num_workers: usize) -> Self {
        assert!(num_workers <= num_cpus::get());

        Self { 
            global_queue: Arc::new(ThreadSafeQueue::new()),
            stop: Arc::new(AtomicBool::new(false)), 
            handles: Vec::new(),
            num_workers,
        }
    }

    /// Spawn the worker threads.
    /// Until you call this function, no thread will be created by the thread pool.
    pub fn spawn_workers(&mut self) {
        for index in 0..self.num_workers {
            let stop = self.stop.clone();
            let stop_err = self.stop.clone();
            let global = self.global_queue.clone();

            // spawn worker threads and store its handles
            self.handles.push(thread::Builder::new()
            .name(format!("Worker {}", index))
            .spawn(move || {
                // initialize thread local work queue.
                LOCAL_QUEUE.with(|queue| {
                    *queue.borrow_mut() = Some(VecDeque::new());

                    if let Some(q) = &*queue.borrow() {
                        println!("Local work queue len: {}", q.len());
                    }
                });
                
                while !stop.load(Ordering::Relaxed) {
                    let local_executed = LOCAL_QUEUE.with(|local| {
                        // this thread is a worker thread
                        if let Some(local) = &mut *local.borrow_mut() {
                            // try pop task from local work queue
                            if let Some(ref mut task) = local.pop_front() {
                                println!("Execute task from local queue!");
                                task.execute();
                                return true;
                            }
                        }
                        return false;
                    });

                    if local_executed {
                        continue;
                    }

                    if let Some(ref mut task) = global.pop() { // try pop task from global work queue
                        println!("Execute task from global queue!");
                        task.execute();
                    } else {
                        std::thread::yield_now();
                    }
                }

                println!("{} is terminated", std::thread::current().name().unwrap());

                // drop thread local work queue.
                LOCAL_QUEUE.with(|queue| {
                    let q = queue.borrow_mut().take().expect("Worker thread's local work queue must exist!");
                    drop(q);
                });
            })
            .unwrap_or_else(move |err| {
                stop_err.store(true, Ordering::Relaxed);
                panic!("Failed to create worker threads for thread pool! with {}", err);
            }));
        }
    }

    /// Add jobs to the thread pool which will be consumed by the worker threads.
    pub fn add_job<F>(&self, f: F) -> JobHandle
    where
        F : FnOnce() -> () + Send + 'static,
    {
        assert!(!self.handles.is_empty(), "No worker threads in this thread pool!");

        let job = Job::new(Box::new(f));
        let job_handle = job.handle();

        LOCAL_QUEUE.with(|queue| {
            if let Some(q) = &mut *queue.borrow_mut() {
                q.push_back(job);
                println!("add job to local queue!");
            } else {
                self.global_queue.push(job);
                println!("add job to global queue!");
            }
        });
        job_handle
    }

    /// Try pop one job from the thread pool and execute it in current thread.
    /// This can be useful to avoid some deadlock scenarios when some tasks are waiting other tasks to finish,
    /// Or can help mitigate the burden of the thread pool.
    pub fn help_once(&mut self) {
        // try pop task from queue
        if let Some(ref mut task) = self.global_queue.pop() {
            task.execute();
        }
    }

    /// Terminate all worker threads in the thread pool.
    /// This function will not interupt the thread, thread will terminate until current work is done.
    pub fn terminate(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Terminate all worker threads in the thread pool.
    /// This function will not interupt the thread, thread will terminate until current work is done.
    /// And this function will wait until all the worker threads are joined. (i.e. this function will block the thread who called this function until current jobs are done)
    pub fn terminate_block(&mut self) {
        self.stop.store(true, Ordering::Relaxed);

        let handles = self.handles.drain(..);

        for handle in handles {
            handle.join().expect("Worker thread had been poisoned!");
        }

        // just forget about the rest of the jobs
        println!("Left Jobs: {}", self.global_queue.len());
    }

    /// Terminate all worker threads in the thread pool.
    /// This function will block the thread who called this function and wait all the jobs are done.
    pub fn terminate_until_finished(&mut self) {
        // self spin to block this thread
        // this will cause move intensive thread contension, do not this!
        // use CondVar to indicate this thread that queue is empty.
        while !self.global_queue.is_empty() {
            std::thread::yield_now();
        }

        self.terminate_block();
    }
}

impl Default for ThreadPool
{
    fn default() -> Self {
        let num_workers = num_cpus::get() / 2;

        Self {
            global_queue: Arc::new(ThreadSafeQueue::new()),
            stop: Arc::new(AtomicBool::new(false)),
            handles: Vec::new(),
            num_workers,
        }
    }
}