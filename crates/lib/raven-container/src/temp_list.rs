use std::cell::{UnsafeCell};

use arrayvec::ArrayVec;

struct TempListInner<T> {
    array: ArrayVec<T, 8>,
    next_ptr: Option<TempList<T>>,
}

impl<T> Default for TempListInner<T> {
    fn default() -> Self {
        Self {
            array: ArrayVec::new(),
            next_ptr: None,
        }
    }
}

pub struct TempList<T> {
    inner: UnsafeCell<Box<TempListInner<T>>>,
}

impl<T> TempList<T> {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub fn add(&self, value: T) -> &T {
        let chunk = unsafe {
            &mut *self.inner.get()
        };

        match chunk.array.try_push(value) {
            // this chunk is full
            Err(err) => {
                let mut new_chunk = Box::new(TempListInner {
                    array: ArrayVec::new(),
                    next_ptr: None,
                });
                new_chunk.array.push(err.element());

                std::mem::swap(&mut new_chunk, chunk);
                chunk.next_ptr = Some(TempList {
                    inner: UnsafeCell::new(new_chunk)
                });

                &chunk.array[0]
            },
            Ok(()) => {
                &chunk.array[chunk.array.len() - 1]
            }
        }
    } 
}