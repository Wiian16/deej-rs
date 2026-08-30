use std::time::Duration;

use clap::Parser;
use deej_rs::{audio::NormalizedVolume, serial};
use log::error;
use simplelog::{ColorChoice, ConfigBuilder, LevelFilter, TermLogger, TerminalMode};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

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

    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let (tx, mut rx) = watch::channel(vec![NormalizedVolume::MIN]);
        let cancellation = CancellationToken::new();

        tokio::spawn(async move {
            loop {
                let result: String = (*rx
                    .borrow_and_update()
                    .iter()
                    .map(|volume| format!("{:.1}%", volume.get() * 100.0))
                    .collect::<Vec<_>>()
                    .join("|")
                    .to_string())
                .to_string();

                println!("{}", result);
                if rx.changed().await.is_err() {
                    break;
                }
            }
        });

        tokio::spawn(serial::run(
            config.com_port,
            config.baud_rate,
            config.invert_sliders,
            tx,
            cancellation,
        ));

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

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
