mod keyboard;
mod mouse;
mod binding;
mod manager;

pub use manager::InputManager;

pub use mouse::MouseButton;
pub use keyboard::VirtualKeyCode;
pub use manager::KeyCode;

pub use binding::{InputMap, InputBindingKey, InputBinding};