use std::{sync::Arc, time::Duration};

use clap::Parser;
use deej_rs::audio::{AudioAdapter, DummyAudioAdapter};
use log::error;
use simplelog::{ColorChoice, ConfigBuilder, LevelFilter, TermLogger, TerminalMode};
use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;

use crate::{args::Args, config::ConfigWatcher};

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
    let config_path = args.get_config_path()?;
    setup_logger(args.verbose)?;

    let service_config = config::load(&config_path)?;
    log::debug!("loaded config: {service_config:#?}");

    let adapter: Arc<dyn AudioAdapter> = Arc::new(DummyAudioAdapter);
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let shutdown = CancellationToken::new();
        let service = tokio::spawn(deej_rs::service::run(
            service_config,
            adapter,
            shutdown.clone(),
        ));

        let cloned_shutdown = shutdown.clone();
        tokio::spawn(async move {
            wait_for_shutdown_signal().await;
            log::info!("shutting down...");
            cloned_shutdown.cancel();
        });

        match ConfigWatcher::new(config_path) {
            Ok(config_watcher) => loop {
                // config_watcher.notified().await;
                // log::debug!("Config file changed");

                tokio::select! {
                    _ = config_watcher.notified() => {
                        log::debug!("config file changed");
                    },
                    _ = shutdown.cancelled() => {break;}
                };
            },
            Err(err) => {
                log::warn!("can't watch config file for changes, live reload not enabled");
                log::debug!("notify error: {err}");
            }
        }

        service.await?
    })
}

async fn wait_for_shutdown_signal() {
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {}
    }
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
        .add_filter_ignore_str("notify")
        .add_filter_ignore_str("inotify")
        .build();

    TermLogger::init(log_level, config, TerminalMode::Mixed, ColorChoice::Auto)
}
