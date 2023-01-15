use raven_math::Vec2;
use winit::{dpi::PhysicalPosition, event::{Event, DeviceEvent, WindowEvent, MouseButton as WinitMouseButton, ElementState, MouseScrollDelta}};

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MouseButton(usize);

impl MouseButton {
    pub const UNKNOWN : Self = Self(0);
    pub const LEFT    : Self = Self(1);
    pub const MIDDLE  : Self = Self(2);
    pub const RIGHT   : Self = Self(3);

    fn as_usize(&self) -> usize {
        self.0
    }

    #[allow(dead_code)]
    fn from_usize(v: usize) -> Self {
        Self(v)
    }
}

pub struct MouseInputState {
    physical_position: PhysicalPosition<f64>,
    position_delta: Vec2,
    wheel_delta: Vec2,
    button_hold: u8,
    button_press: u8,
    button_release: u8,
}

impl MouseInputState {
    pub fn new() -> Self {
        Self {
            physical_position: PhysicalPosition { x: 0.0, y: 0.0 },
            position_delta: Vec2::ZERO,
            wheel_delta: Vec2::ZERO,
            button_hold: 0,
            button_press: 0,
            button_release: 0,
        }
    }

    #[allow(dead_code)]
    pub fn physical_position(&self) -> Vec2 {
        Vec2::new(self.physical_position.x as f32, self.physical_position.y as f32)
    }

    pub fn position_delta(&self) -> Vec2 {
        self.position_delta
    }

    #[allow(dead_code)]
    pub fn wheel_delta(&self) -> Vec2 {
        self.wheel_delta
    }

    pub fn is_button_just_pressed(&self, button: MouseButton) -> bool {
        let button = button.as_usize();
        if (self.button_press & (1 << button)) != 0 {
            true
        } else {
            false
        }
    }

    pub fn is_button_hold(&self, button: MouseButton) -> bool {
        let button = button.as_usize();
        if (self.button_hold & (1 << button)) != 0 {
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn is_button_just_released(&self, button: MouseButton) -> bool {
        let button = button.as_usize();
        if (self.button_release & (1 << button)) != 0 {
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, events: &[Event<'_, ()>]) {
        self.button_press = 0;
        self.button_release = 0;
        self.position_delta = Vec2::ZERO;
        self.wheel_delta = Vec2::ZERO;

        for event in events {
            match event {
                Event::WindowEvent { 
                    window_id: _, 
                    event,
                } => {
                    match event {
                        WindowEvent::MouseInput {
                            state,
                            button,
                            ..
                        } => {
                            let button = match button {
                                WinitMouseButton::Left => MouseButton::LEFT,
                                WinitMouseButton::Middle => MouseButton::MIDDLE,
                                WinitMouseButton::Right => MouseButton::RIGHT,
                                _ => MouseButton::UNKNOWN,
                            }.as_usize();
        
                            if *state == ElementState::Pressed {
                                self.button_press |= 1 << button;
                                self.button_hold |= 1 << button;
                            } else {
                                self.button_hold &= !(1 << button);
                                self.button_release |= 1 << button;
                            }
                        }
                        // Only can about mouse now
                        WindowEvent::MouseWheel { delta, .. } => {
                            if let MouseScrollDelta::LineDelta(left, up) = delta {
                                self.wheel_delta += Vec2::new(*left, *up);
                            };
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            self.physical_position = *position;
                        }
                        _ => {},
                    }
                }
                Event::DeviceEvent { device_id: _, event } => {
                    match event {
                        DeviceEvent::MouseMotion { delta } => {
                            let delta = *delta;
                            self.position_delta += Vec2::new(delta.0 as f32, delta.1 as f32);
                        }
                        _ => {},
                    }
                }
                _ => {}
            }
        }
    }
}