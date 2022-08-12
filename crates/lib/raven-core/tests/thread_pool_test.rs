use raven_core::thread::ThreadPool;

#[test]
fn thread_pool_works() {
    println!("main thread name: {}", std::thread::current().name().unwrap());

    let mut pool = ThreadPool::new(2);

    pool.spawn_workers();

    let jobs: Vec<_> = (0..100).map(|i| {
        pool.add_job(move || {
            // static mut COUNT: i32 = 10;

            // unsafe {
            //     if COUNT > 0 {
            //         pool.add_job(|| {
            //             println!("Hello from my worker thread pushed job!");
            //         });
            //     }
            // }

            println!("[{}] Hello from {}", i, std::thread::current().name().unwrap());
        })
    }).collect();

    println!("Jobs all added!");
    
    while !jobs[50].is_complete() {
        pool.help_once();
    }
    println!("Job(50) is completed!");

    //pool.terminate_block();
    pool.terminate_until_finished();

    println!("thread pool is terminated!");

    for job in jobs {
        assert_eq!(job.is_complete(), true);
        //println!("is complete: {}", job.is_complete());
    }
}