#![deny(missing_docs)]

use std::{env, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use log::LevelFilter;

#[derive(Parser, Debug)]
pub struct Args {
    /// Provide a config file for Deej, defaults to the location of the executable with the name 'config.yaml'
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Suppress all application logs except for warning
    #[arg(short, long, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Increase logging verbosity (useful for debugging serial).
    ///
    /// Ex: *none* (error, warn), -v (error, warn, info), -vv (error, warn, info, debug), -vvv (error warn, info, debug, trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl Args {
    /// Gets the proper path to the config file. If not provided, returns `config.yaml` in the executable's directory.
    ///
    /// # Errors
    ///
    /// May return an error if the curren't executable path or parent direcctory cannot be determined.
    pub fn get_config_path(&self) -> anyhow::Result<PathBuf> {
        if let Some(path) = &self.config {
            Ok(path.clone())
        } else {
            let exe =
                env::current_exe().context("failed to determine the current executable path")?;

            let exe_dir = exe
                .parent()
                .context("failed to determine the executable's directory")?;

            Ok(exe_dir.join("config.yaml"))
        }
    }

    pub const fn log_level(&self) -> LevelFilter {
        if self.quiet {
            return LevelFilter::Error;
        }

        match self.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}
