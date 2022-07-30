// use log macros.
#[macro_use]
extern crate log as _log;

fn main() {
    raven_engine::init();

    trace!("trace!");
    debug!("debug!");
    info!("info!");
    warn!("warn!");
    error!("error!");

    raven_engine::shutdown();
}
