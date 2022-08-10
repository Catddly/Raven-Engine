use std::{sync::{Arc, atomic::AtomicU32}, thread, time::Instant, sync::atomic::Ordering};

use raven_core::thread::ThreadSafeQueue;

#[test]
fn thread_safe_queue_works() {
    let queue = Arc::new(ThreadSafeQueue::<u32>::new());
    let fetch_total = Arc::new(AtomicU32::new(0));
    let push_count = Arc::new(AtomicU32::new(0));
    let pop_count = Arc::new(AtomicU32::new(0));

    let push_handles : Vec<_> = (0..4).into_iter().map(|_| {
        let shared_queue = Arc::clone(&queue);
        let shared_push_count = Arc::clone(&push_count);

        thread::spawn(move || {
            for i in 0..10 {
                let now = Instant::now();
                shared_queue.push(i);
                let elapsed = now.elapsed();
                println!("queue.push() cost {} ns.", elapsed.as_nanos());
                shared_push_count.fetch_add(1, Ordering::Release);
            }
        })
    }).collect();

    let pop_handles : Vec<_> = (0..4).into_iter().map(|_| {
        let shared_queue = Arc::clone(&queue);
        let shared_pop_count = Arc::clone(&pop_count);
        let shared_fetch_total = Arc::clone(&fetch_total);

        thread::spawn(move || {
            for _ in 0..10 {
                loop {
                    let data = shared_queue.pop();
                    if let Some(v) = data {
                        println!("thread [{:?}] fetch data: {}", thread::current().id(), v);
                        shared_pop_count.fetch_add(1, Ordering::Release);
                        shared_fetch_total.fetch_add(v, Ordering::Release);
                        break;
                    }
                }
            }
        })
    }).collect();

    for handle in push_handles {
        handle.join().unwrap();
    }
    
    for handle in pop_handles {
        handle.join().unwrap();
    }

    let now = Instant::now();
    let push_count = push_count.load(Ordering::Acquire);
    let elapsed = now.elapsed();
    println!("Total push count: {} cost {} ns.", push_count, elapsed.as_nanos());
    
    let now = Instant::now();
    let pop_count = pop_count.load(Ordering::Acquire);
    let elapsed = now.elapsed();
    println!("Total pop count: {} cost {} ns.", pop_count, elapsed.as_nanos());

    assert_eq!(45 * 4, fetch_total.load(Ordering::Acquire));
}