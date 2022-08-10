use std::time::Duration;

use raven_core::thread::ThreadPool;

#[test]
fn thread_pool_works() {
    let mut pool = ThreadPool::new();

    pool.spawn_workers();

    std::thread::sleep(Duration::from_secs(1));
    pool.terminate_block();
    println!("workers in the thread pool are terminated!");
}