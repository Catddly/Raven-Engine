// Raven Engine APIs
use raven_core::log;
use raven_core::console;

/// Initialize raven engine.
pub fn init() {
    let console_var = console::from_args();

    log::init_log(log::LogConfig {
        level: console_var.level,
    });
}

/// Shutdown raven engine.
pub fn shutdown() {

}