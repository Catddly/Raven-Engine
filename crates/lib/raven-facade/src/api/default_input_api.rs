use std::{ops::Deref, sync::{Arc}};

use parking_lot::RwLock;

pub use raven_input::{InputBinding, KeyCode, MouseButton, VirtualKeyCode};

use raven_input::InputManager;

#[derive(Clone)]
pub struct InputApiImpl(Option<Arc<RwLock<InputManager>>>);

unsafe impl Send for InputApiImpl {}
unsafe impl Sync for InputApiImpl {}

impl std::fmt::Debug for InputApiImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug Default InputApiInner")
    }
}

impl Deref for InputApiImpl {
    type Target = Arc<RwLock<InputManager>>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl InputApiImpl {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn init(&mut self) {
        self.0 = Some(Arc::new(RwLock::new(InputManager::new())));

    }

    pub fn shutdown(mut self) {
        if let Some(inner) = self.0.take() {
            let inner = Arc::try_unwrap(inner)
                .expect("Reference counting of input api may not be retained!");
            let inner = inner.into_inner();
            drop(inner);
        } else {
            panic!("Try to shutdown render apis before initializing!");
        }
    }
}