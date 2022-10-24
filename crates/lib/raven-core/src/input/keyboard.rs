use std::collections::HashMap;

// TODO: use engine's own keycode
use winit::event::{VirtualKeyCode, Event, WindowEvent, ElementState};

#[derive(Copy, Clone)]
struct KeyState {
    tick_count: u32,
}

pub struct KeyboardInputState {
    input_record_map: HashMap<VirtualKeyCode, KeyState>,
}

impl KeyboardInputState {
    pub fn new() -> Self {
        Self {
            input_record_map: Default::default(),
        }
    }

    #[allow(dead_code)]
    pub fn is_button_just_pressed(&self, vk: VirtualKeyCode) -> bool {
        self.input_record_map.get(&vk).map_or(false, |state| state.tick_count == 1)
    }

    pub fn is_button_pressed(&self, vk: VirtualKeyCode) -> bool {
        self.input_record_map.contains_key(&vk)
    }

    pub fn update(&mut self, events: &[Event<'_, ()>]) {
        for event in events {
            if let Event::WindowEvent { event, .. } = event {
                if let WindowEvent::KeyboardInput { input, .. } = event {
                    if let Some(vk) = input.virtual_keycode {
                        if input.state == ElementState::Pressed {
                            self.input_record_map.entry(vk).or_insert(KeyState { tick_count: 0 });
                        } else {
                            self.input_record_map.remove(&vk);
                        }
                    }
                }
            }
        }
        
        // tick once
        for tick in self.input_record_map.values_mut() {
            tick.tick_count += 1;
        }
    }
}