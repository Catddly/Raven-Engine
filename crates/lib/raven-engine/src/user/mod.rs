pub trait App {
    fn init(&mut self) -> anyhow::Result<()>;
    fn tick(&mut self, dt: f32);
    fn shutdown(self) where Self: Sized;
}

#[macro_export]
macro_rules! raven_main {
    ($app:expr) => {
        fn main() {
            let mut engine_context = raven_engine::init($app).unwrap_or_else(|err| {
                eprintln!("Raven Engine failed to init with: {}", err); // use eprintln here, because log module may not be initialized successfully.
                std::process::exit(1);
            });
            raven_engine::main_loop(&mut engine_context);
            raven_engine::shutdown(engine_context);
        }
    };
}