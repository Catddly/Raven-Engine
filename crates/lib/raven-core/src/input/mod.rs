use glam::Vec2;
use winit::event::{Event, VirtualKeyCode};

use self::{keyboard::KeyboardInputState, mouse::MouseInputState, binding::{InputBindingMap}};

mod keyboard;
mod mouse;
mod binding;

pub use mouse::MouseButton;
pub use binding::{KeyCode, InputMap, InputBindingKey, InputBinding};

// TODO: distinguish different device id (support different devices and multi-devices)
pub struct InputManager {
    keyboard_input: KeyboardInputState,
    mouse_input: MouseInputState,

    bindings: InputBindingMap,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            keyboard_input: KeyboardInputState::new(),
            mouse_input: MouseInputState::new(),

            bindings: InputBindingMap::new(),
        }
    }

    #[inline]
    pub fn add_binding(&mut self, keycode: KeyCode, binding: InputBinding) {
        self.bindings.bind(keycode, binding);
    }

    pub fn update(&mut self, events: &[Event<'_, ()>]) {
        self.keyboard_input.update(events);
        self.mouse_input.update(events);
    }

    pub fn map(&mut self, dt: f32) -> InputMap {
        self.bindings.map_with_input(&self.keyboard_input, &self.mouse_input, dt)
    }

    #[inline]
    pub fn is_keyboard_just_pressed(&self, vk: VirtualKeyCode) -> bool {
        self.keyboard_input.is_keyboard_just_pressed(vk)
    }

    #[inline]
    pub fn mouse_pos_delta(&self) -> Vec2 {
        self.mouse_input.position_delta()
    }
    
    #[inline]
    pub fn is_mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_input.is_button_just_pressed(button)
    }

    #[inline]
    pub fn is_mouse_hold(&self, button: MouseButton) -> bool {
        self.mouse_input.is_button_hold(button)
    }
}