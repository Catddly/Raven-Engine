use std::any::{Any, TypeId};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventTypeHash(pub TypeId);

impl From<TypeId> for EventTypeHash {
    fn from(value: TypeId) -> Self {
        Self(value)
    }
}

pub trait Event: 'static {
    fn type_hash() -> EventTypeHash;

    fn type_name() -> &'static str;
}

pub type CallbackFn<'event, E> = fn(&'event E);
// pub type CallbackMethod<'event, 'obj, T, E> = fn(&'obj T, &'event E);

// pub struct CallbackFnBoxed<'event, E: Event>(pub Box<CallbackFn<'event, E>>);
// pub struct CallbackMethodBoxed<'event, 'obj, T: 'static, E: Event>(pub Box<CallbackMethod<'event, 'obj, T, E>>);

pub struct EventFuncPool<'event>(Vec<EventCallbackFn<'event>>);

impl<'event> EventFuncPool<'event> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add_callback(&mut self, callback: EventCallbackFn<'event>) {
        self.0.push(callback);
    }

    pub fn trigger_all(&self, event: UntypedEvent<'event>)
    {
        let type_name = event.event_type_name;
        
        match type_name {
            "EMessage" => {
                let concrete = event.untyped.downcast_ref::<EMessage>().unwrap();
                for callback in self.0.iter() {
                    if let EventCallbackFn::EMessageCallback(call) = callback {
                        call(concrete);
                    } else {
                        panic!("Unmatched type! (event: EMessage, callback: {})", type_name);
                    }
                }
            }
            "EHello" => {
                let concrete = event.untyped.downcast_ref::<EHello>().unwrap();
                for callback in self.0.iter() {
                    if let EventCallbackFn::EHelloCallback(call) = callback {
                        call(concrete);
                    } else {
                        panic!("Unmatched type! (event: EHello, callback: {})", type_name);
                    }
                }
            }
            _ => panic!("Unknown event type: {}", type_name),
        }
    }
}

pub struct UntypedEvent<'event> {
    event_type_name: &'static str,
    untyped: &'event dyn Any,
}

impl<'event> UntypedEvent<'event> {
    pub fn new<E: Event>(event: &'event E) -> Self {
        Self {
            event_type_name: E::type_name(),
            untyped: event,
        }
    } 
}

#[derive(Clone)]
pub struct EMessage {
    pub count: u32,
    pub msg: String,
}

impl Event for EMessage {
    fn type_hash() -> EventTypeHash {
        TypeId::of::<EMessage>().into()
    }

    fn type_name() -> &'static str {
        "EMessage"
    }
}

pub struct EMessageStorage(Vec<EMessage>);

#[derive(Clone)]
pub struct EHello {
    pub hello_str: String,
}

impl Event for EHello {
    fn type_hash() -> EventTypeHash {
        TypeId::of::<EHello>().into()
    }

    fn type_name() -> &'static str {
        "EHello"
    }
}

pub struct EHelloStorage(Vec<EHello>);

pub enum EventStorage {
    EMessageStorage(EMessageStorage),
    EHelloStorage(EHelloStorage),
}

pub enum EventCallbackFn<'event> {
    EMessageCallback(CallbackFn<'event, EMessage>),
    EHelloCallback(CallbackFn<'event, EHello>),
}

impl<'event> From<CallbackFn<'event, EMessage>> for EventCallbackFn<'event> {
    fn from(value: CallbackFn<'event, EMessage>) -> Self {
        Self::EMessageCallback(value)
    }
}

impl<'event> From<CallbackFn<'event, EHello>> for EventCallbackFn<'event> {
    fn from(value: CallbackFn<'event, EHello>) -> Self {
        Self::EHelloCallback(value)
    }
}

// pub enum EventCallbackMethod<'event, 'obj, T> {
//     EMessageCallback(CallbackMethod<'event, 'obj, T, EMessage>),
//     EHelloCallback(CallbackMethod<'event, 'obj, T, EHello>),
// }

// impl<'event, 'obj, T> From<CallbackMethod<'event, 'obj, T, EMessage>> for EventCallbackMethod<'event, 'obj, T> {
//     fn from(value: CallbackMethod<'event, 'obj, T, EMessage>) -> Self {
//         Self::EMessageCallback(value)
//     }
// }

// impl<'event, 'obj, T> From<CallbackMethod<'event, 'obj, T, EHello>> for EventCallbackMethod<'event, 'obj, T> {
//     fn from(value: CallbackMethod<'event, 'obj, T, EHello>) -> Self {
//         Self::EHelloCallback(value)
//     }
// }