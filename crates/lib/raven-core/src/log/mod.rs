use std::collections::HashSet;

use anyhow::Ok;
use fern::colors::{Color, ColoredLevelConfig};

pub use log::LevelFilter as LevelFilter;

use crate::filesystem;

lazy_static::lazy_static! {
    static ref GLOBAL_MUTE_MODULE_NAMES: HashSet<&'static str> = HashSet::from([
        "hotwatch::util",
        "gpu_allocator::vulkan"
    ]);
}

/// Log configuration.
#[derive(Copy)]
pub struct LogConfig {
    pub level: LevelFilter,
}

impl Clone for LogConfig {
    #[inline]
    fn clone(&self) -> LogConfig {
        *self
    }
}

/// Initialize log module.
pub fn init_log(config: LogConfig) -> anyhow::Result<()> {
    filesystem::exist_or_create(filesystem::ProjectFolder::Log)?;
    setup_logger(config).expect("Failed to initialize log module!");
    
    glog::trace!("log initialized!");
    Ok(())
}

fn setup_logger(config: LogConfig) -> anyhow::Result<()> {
    // setup colors
    let colors = ColoredLevelConfig::new()
        .trace(Color::White)
        .debug(Color::Magenta)
        .info(Color::Cyan)
        .warn(Color::Yellow)
        .error(Color::Red);

    // standard output dispatch, for trace, debug and info messages.
    let stdout = fern::Dispatch::new()
        .filter(|meta| {
            meta.level() >= log::Level::Info &&
            GLOBAL_MUTE_MODULE_NAMES.get(meta.target()).is_none()
        })
        .chain(std::io::stdout());
            
    // standard error dispatch, for warn and error messages.
    let stderr = fern::Dispatch::new()
        .level(LevelFilter::Warn)
        .filter(|meta| {
            GLOBAL_MUTE_MODULE_NAMES.get(meta.target()).is_none()
        })
        .chain(std::io::stderr());
    
    // console output with the colors
    let console_output = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors.color(record.level()),
                message
            ))
        })
        .chain(stdout)
        .chain(stderr);

    let file_output = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .chain(std::fs::OpenOptions::new() // global file output
            .create(true)
            .write(true)
            .truncate(true)
            .open("log/log.txt")?);

    // final apply to all the dispatches
    fern::Dispatch::new()
        .level(config.level) // setup base log level from user
        .chain(console_output)
        .chain(file_output)
        .apply()?;

        Ok(())
}