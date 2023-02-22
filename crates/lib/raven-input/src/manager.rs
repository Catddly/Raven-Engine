use winit::event::Event;

use raven_math::Vec2;

use crate::InputMap;
use crate::{binding::InputBindingMap, InputBinding};
use crate::keyboard::KeyboardInputState;
use crate::mouse::MouseInputState;

use super::{VirtualKeyCode, MouseButton};

#[derive(Hash, Copy, Clone)]
pub enum KeyCode {
    VirtualKeyCode(VirtualKeyCode),
    Mouse(MouseButton),
}

impl PartialEq for KeyCode {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::VirtualKeyCode(vk) => {
                if let KeyCode::VirtualKeyCode(vk_other) = other {
                    return vk == vk_other;
                } else {
                    false
                }
            }
            Self::Mouse(mouse) => {
                if let KeyCode::Mouse(mouse_other) = other {
                    return mouse == mouse_other;
                } else {
                    false
                }
            }
        }
    }
}
impl Eq for KeyCode {}

impl KeyCode {
    #[inline]
    pub fn vkcode(vk: VirtualKeyCode) -> Self {
        Self::VirtualKeyCode(vk)
    }
    
    #[inline]
    pub fn mouse(mouse: MouseButton) -> Self {
        Self::Mouse(mouse)
    }
}


// TODO: distinguish different device id (support different devices and multi-devices)
#[non_exhaustive]
pub struct InputManager {
    keyboard_input: KeyboardInputState,
    mouse_input: MouseInputState,

    bindings: InputBindingMap,
}

impl std::fmt::Debug for InputManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug InputManager")
    }
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
    pub fn is_keyboard_pressed(&self, vk: VirtualKeyCode) -> bool {
        self.keyboard_input.is_keyboard_pressed(vk)
    }

    #[inline]
    pub fn is_keyboard_just_pressed(&self, vk: VirtualKeyCode) -> bool {
        self.keyboard_input.is_keyboard_just_pressed(vk)
    }

    #[inline]
    pub fn is_mouse_just_pressed(&self, mb: MouseButton) -> bool {
        self.mouse_input.is_button_just_pressed(mb)
    }

    #[inline]
    pub fn is_mouse_button_hold(&self, mb: MouseButton) -> bool {
        self.mouse_input.is_button_hold(mb)
    }

    #[inline]
    pub fn is_mouse_button_just_released(&self, mb: MouseButton) -> bool {
        self.mouse_input.is_button_just_released(mb)
    }

    #[inline]
    pub fn mouse_pos_delta(&self) -> Vec2 {
        self.mouse_input.position_delta()
    }

    #[inline]
    pub fn mouse_wheel_delta(&self) -> Vec2 {
        self.mouse_input.wheel_delta()
    }
}
