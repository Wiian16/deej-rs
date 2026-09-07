use std::sync::Arc;

use clap::Parser;
use deej_rs::audio::pulseaudio::PulseAudioAdapter;
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

    let mut service_config = config::load(&config_path)?;
    log::debug!("loaded config: {service_config:#?}");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let shutdown = CancellationToken::new();
        let cloned_shutdown = shutdown.clone();

        tokio::spawn(async move {
            wait_for_shutdown_signal().await;
            log::info!("shutting down...");
            cloned_shutdown.cancel();
        });

        let config_watcher = match ConfigWatcher::new(&config_path) {
            Ok(watcher) => Some(watcher),
            Err(err) => {
                log::warn!("can't watch config file for changes, live reload not enabled");
                log::debug!("notify error: {err}");
                None
            }
        };

        'outer: loop {
            let adapter = Arc::new(PulseAudioAdapter::new().await?);

            let service_token = shutdown.child_token();
            let mut service = tokio::spawn(deej_rs::service::run(
                service_config.clone(),
                adapter.clone(),
                service_token.clone(),
            ));

            loop {
                tokio::select! {
                    _ = wait_for_config_change(&config_watcher) => {
                        match config::load(&config_path) {
                            Ok(new_config) => {
                                log::info!("config change detected, reloading deej");
                                service_config = new_config;
                                service_token.cancel();
                                let _ = service.await;
                                continue 'outer;
                            },
                            Err(err) => {
                                log::warn!("failed to reload config, keeping current service running");
                                log::debug!("config error : {err:#}");
                            }
                        }
                    }
                    _ = shutdown.cancelled() => {
                        let _ = service.await;
                        break 'outer;
                    }
                    result = &mut service => {
                        // Service exited on it's own, propagate error
                        return result?;
                    }
                }
            }
        }

        Ok(())
    })
}

async fn wait_for_config_change(watcher: &Option<ConfigWatcher>) {
    match watcher {
        Some(w) => w.notified().await,
        None => std::future::pending().await,
    }
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
