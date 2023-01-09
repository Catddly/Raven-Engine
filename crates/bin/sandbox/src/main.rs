extern crate log as glog;

use raven_engine::prelude::*;

pub struct Sandbox;

impl Sandbox {
    pub fn new() -> Self {
        Self {

        }
    }
}

impl App for Sandbox {
    fn init(&mut self) -> anyhow::Result<()> {
        glog::info!("User app init!");

        Ok(())
    }

    fn tick(&mut self, _dt: f32) {
        
    }

    fn shutdown(self) where Self: Sized {
        glog::info!("User app shutdown!");
    }
}

raven_main!{ Sandbox::new() }