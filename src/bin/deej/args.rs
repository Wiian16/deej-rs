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
    /// # Panics
    ///
    /// Panics if [`env::current_exe`] fails to return a path or if [`Path::parent`](std::path::Path::parent) fails to return a path.
    pub fn get_config_path(&self) -> anyhow::Result<PathBuf> {
        match &self.config {
            Some(path) => Ok(path.clone()),
            None => {
                let exe = env::current_exe()
                    .context("failed to determine the current executable path")?;

                let exe_dir = exe
                    .parent()
                    .context("failed to determine the executable's directory")?;

                Ok(exe_dir.join("config.yaml"))
            }
        }
    }
}
