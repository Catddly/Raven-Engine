use structopt::StructOpt;

/// Collect console configuration into a struct.
pub fn from_args() -> ConsoleVars {
    let console_var = ConsoleVarsImpl::from_args();

    let level = match console_var.level.to_lowercase().trim() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => panic!("Unknown log level!"),
    };

    ConsoleVars {
        level: level
    }
}

/// Console variables collect from console commands.
pub struct ConsoleVars {
    pub level: log::LevelFilter,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "raven engine", about = "A small game engine.")]
struct ConsoleVarsImpl {
    /// log level (please choose from trace, debug, info, warn, error)
    #[structopt(short, long, default_value = "debug")]
    level: String,
} 