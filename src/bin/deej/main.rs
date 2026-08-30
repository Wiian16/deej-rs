use std::sync::Arc;

use clap::Parser;
use deej_rs::audio::{AudioAdapter, DummyAudioAdapter};
use log::error;
use simplelog::{ColorChoice, ConfigBuilder, LevelFilter, TermLogger, TerminalMode};

use crate::args::Args;

mod args;
mod config;

fn main() {
    if let Err(err) = run() {
        // use eprintln for errors occurring before logging is set up
        if log::max_level() == log::LevelFilter::Off {
            eprintln!("Error: {err}");
        }

        error!("{err}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    setup_logger(args.verbose)?;

    let service_config = config::load(args.get_config_path()?)?;
    log::debug!("loaded config: {service_config:#?}");

    let adapter: Arc<dyn AudioAdapter> = Arc::new(DummyAudioAdapter);
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(deej_rs::service::run(service_config, adapter))
}

fn setup_logger(verbose: bool) -> Result<(), log::SetLoggerError> {
    let log_level = if verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Warn
    };

    let config = ConfigBuilder::new()
        .set_target_level(LevelFilter::Off)
        .set_thread_level(LevelFilter::Off)
        .set_location_level(LevelFilter::Off)
        .build();

    TermLogger::init(log_level, config, TerminalMode::Mixed, ColorChoice::Auto)
}
