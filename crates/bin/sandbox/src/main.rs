use std::process::exit;

extern crate log as glog;

fn main() {
    let mut engine_context = raven_engine::init().unwrap_or_else(|err| {
        eprintln!("Raven Engine failed to init with: {}", err); // use eprintln here, because log module may not be intialized successfully.
        exit(1);
    });
    raven_engine::main_loop(&mut engine_context);
    raven_engine::shutdown(engine_context);
}
