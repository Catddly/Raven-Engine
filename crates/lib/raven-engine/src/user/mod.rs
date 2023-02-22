pub trait App {
    fn init(&mut self) -> anyhow::Result<()>;
    fn tick_logic(&mut self, dt: f32);
    fn shutdown(&mut self);
}

#[macro_export]
macro_rules! raven_main {
    ($app:expr) => {
        fn main() {
            raven_engine::init(Box::new($app)).unwrap_or_else(|err| {
                eprintln!("Raven Engine failed to initialize with: {}", err); // use eprintln here, because log module may not be initialized successfully.
                std::process::exit(1);
            });
            raven_engine::main_loop();
            raven_engine::shutdown();
        }
    };
}