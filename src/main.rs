use clap::Parser;
use log::error;
use simplelog::{ColorChoice, ConfigBuilder, LevelFilter, TermLogger, TerminalMode};

use crate::{args::Args, config::Config};

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

    println!("CLI args: {:#?}", args);
    println!(
        "Config path: {:#?}",
        std::path::absolute(args.get_config_path()?)
    );

    let config = Config::from_file(args.get_config_path()?)?;
    println!("Config: {:#?}", config);

    Ok(())
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
