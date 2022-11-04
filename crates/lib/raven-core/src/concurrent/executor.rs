use std::{future::Future, panic::catch_unwind, pin::Pin, task::{Poll, Context}};

use smol::{Task, Executor};
use async_io;
use once_cell::sync::Lazy;

/// Always pending future.
struct AlwaysPending;

impl AlwaysPending {
    fn new() -> Self {
        AlwaysPending
    }
}

impl Unpin for AlwaysPending {}

impl Future for AlwaysPending {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
} 

pub fn spawn<T: Send + 'static>(future: impl Future<Output = T> + 'static + Send) -> Task<T> {
    static GLOBAL_EXECUTORS: Lazy<Executor<'_>> = Lazy::new(|| {
        let num_cpus = num_cpus::get();
        let num_threads = if num_cpus >= 4 {
            num_cpus / 2
        } else {
            1
        };

        // Debug purposed
        let num_threads = 1;

        for i in 0..num_threads {
            std::thread::Builder::new()
                .name(format!("Executor {}", i))
                .spawn(|| {
                    loop {
                        catch_unwind(|| async_io::block_on(GLOBAL_EXECUTORS.run(AlwaysPending::new()))).ok();
                    }
                })
                .expect("Failed to spawn executor threads!");
        }

        Executor::new()
    });

    GLOBAL_EXECUTORS.spawn(future)
}