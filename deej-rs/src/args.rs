#![deny(missing_docs)]

use std::{env, path::PathBuf};

use anyhow::Context;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Provide a config file for Deej, defaults to the location of the executable with the name 'config.yaml'
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Show verbose logs (useful for debugging serial)
    #[arg(short, long)]
    pub verbose: bool,
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
}
