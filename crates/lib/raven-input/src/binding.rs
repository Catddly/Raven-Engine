use std::collections::HashMap;

use crate::manager::KeyCode;

use super::{mouse::{MouseInputState}, keyboard::KeyboardInputState};

pub type InputBindingKey = &'static str;
pub type InputMap = HashMap<InputBindingKey, f32>;

pub struct InputBinding {
    key: InputBindingKey,
    /// Range from -1.0 to 1.0
    multiplier: f32,
    activation_time: f32,

    curr_activation_time: f32,
}

impl InputBinding {
    pub fn new(key: impl Into<InputBindingKey>, multiplier: f32) -> Self {
        let key = key.into();
        Self {
            key,
            multiplier,
            activation_time: 0.0,
            curr_activation_time: 0.0,
        }
    }

    pub fn activation_time(mut self, activation_time: f32) -> Self {
        self.activation_time = activation_time;
        self
    }
}

pub struct InputBindingMap {
    bindings: Vec<(KeyCode, InputBinding)>,
}

impl InputBindingMap {
    pub fn new() -> Self {
        Self {
            bindings: Default::default(),
        }
    }

    pub fn bind(&mut self, keycode: KeyCode, binding: InputBinding) {
        self.bindings.push((keycode, binding));
    }

    #[allow(dead_code)]
    pub fn unbind(&mut self, keycode: KeyCode) {
        // TODO: when we have multiple keycode in one bindings, we just remove one arbitrary element.
        let mut idx = 0;
        for (binding_idx, (kcode, _)) in self.bindings.iter().enumerate() {
            if keycode == *kcode {
                idx = binding_idx;
                break;
            }
        }

        self.bindings.swap_remove(idx);
    }

    #[allow(dead_code)]
    pub fn unbind_all(&mut self, key: impl Into<InputBindingKey>) {
        let key = key.into();
        self.bindings.retain(|(_, binding)| binding.key != key);
    }

    pub fn map_with_input(&mut self, vkinput: &KeyboardInputState, mouse_input: &MouseInputState, dt: f32) -> InputMap {
        let mut result: InputMap = HashMap::new();

        for (ref keycode, binding) in self.bindings.iter_mut() {
            let curr_activation_time = if binding.activation_time > 1e-10 {
                let dt = match keycode {
                    KeyCode::VirtualKeyCode(vk) => {
                        if vkinput.is_keyboard_pressed(*vk) { dt } else { -dt }
                    }
                    KeyCode::Mouse(mouse) => {
                        if mouse_input.is_button_hold(*mouse) { dt } else { -dt }
                    }
                };

                binding.curr_activation_time = (binding.curr_activation_time + dt).clamp(0.0, binding.activation_time);
                binding.curr_activation_time / binding.activation_time
            } else { // no activation time
                let activated = match keycode {
                    KeyCode::VirtualKeyCode(vk) => {
                        if vkinput.is_keyboard_pressed(*vk) { true } else { false }
                    }
                    KeyCode::Mouse(mouse) => {
                        if mouse_input.is_button_hold(*mouse) { true } else { false }
                    }
                };

                if activated {
                    binding.curr_activation_time = 1.0;
                    1.0
                } else {
                    binding.curr_activation_time = 0.0;
                    0.0
                }
            };

            let value = result.entry(binding.key).or_default();
            *value += curr_activation_time.powi(2) * binding.multiplier;
            *value = value.clamp(-1.0, 1.0);
        }

        result
    }
}