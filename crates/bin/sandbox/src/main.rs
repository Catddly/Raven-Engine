extern crate log as glog;

use raven_engine::prelude::*;

pub struct Sandbox;

impl App for Sandbox {
    fn init(&mut self) -> anyhow::Result<()> {
        glog::info!("User app init!");

        Ok(())
    }

    fn tick_logic(&mut self, _dt: f32) {
        let input_api = input::get();
        let res = input_api.read().is_keyboard_just_pressed(input::VirtualKeyCode::P);

        if res {
            glog::debug!("P is pressed!");
        }
    }

    fn shutdown(&mut self) {
        glog::info!("User app shutdown!");
    }
}

raven_main!{ Sandbox }