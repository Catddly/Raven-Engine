use std::collections::{HashMap, hash_map::Entry};

use super::{event::{EventTypeHash, Event, CallbackFn, UntypedEvent, EventFuncPool, EventCallbackFn}};

pub struct EventDispatcher<'event> {
    func_pools: HashMap<EventTypeHash, EventFuncPool<'event>>,
    //receiver_pools: HashMap<ReceiverHash, HashMap<EventTypeHash, EventMethodPool<'event>>>,
}

impl<'event> EventDispatcher<'event> {
    pub fn new() -> Self {
        Self {
            func_pools: HashMap::new(),
            //receiver_pools: HashMap::new(),
        }
    }

    pub fn connect_func<E>(&mut self, callback: CallbackFn<'event, E>)
    where
        E: Event,
        CallbackFn<'event, E>: Into<EventCallbackFn<'event>>
    {
        let type_hash = E::type_hash();

        match self.func_pools.entry(type_hash) {
            Entry::Vacant(entry) => {
                let mut new_pool = EventFuncPool::new();
                new_pool.add_callback(callback.into());
                entry.insert(new_pool);
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().add_callback(callback.into());
            }
        }
    }

    // pub fn connect_method<T, E>(&mut self, receiver: &'obj T, callback: CallbackMethod<'event, T, E>)
    // where
    //     T: 'static,
    //     E: Event,
    //     CallbackMethod<'event, T, E>: Into<EventCallbackMethod<'event, T>>
    // {
    //     let untyped_receiver = UntypedReceiver::new(receiver);
    //     let receiver_hash = untyped_receiver.hash;

    //     match self.receiver_pools.entry(receiver_hash) {
    //         Entry::Vacant(entry) => {
    //             // let mut new_pool = EventFuncPool::new();
    //             // new_pool.add_callback(callback.into());
    //             // entry.insert(new_pool);
    //         }
    //         Entry::Occupied(mut entry) => {
    //             // entry.get_mut().add_callback(callback.into());
    //         }
    //     }
    // }

    pub fn trigger<E>(&self, event: &'event E)
    where
        E: Event
    {
        let type_hash = E::type_hash();
        if let Some(pool) = self.func_pools.get(&type_hash) {
            let untyped = UntypedEvent::new(event);
            pool.trigger_all(untyped);
        }
    }
}