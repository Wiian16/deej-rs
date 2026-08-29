#![deny(missing_docs)]

use std::{env, path::PathBuf};

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Provide a config file for Deej, defaults to the location of the executable with the name 'config.yaml'
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Show verbose logs (useful for debugging serial)
    #[arg(short, long)]
    verbose: bool,
}

impl Args {
    /// Gets the proper path to the config file. If not provided, returns `config.yaml` in the executable's directory.
    ///
    /// # Panics
    ///
    /// Panics if [`env::current_exe`] fails to return a path or if [`Path::parent`](std::path::Path::parent) fails to return a path.
    pub fn get_config_path(&self) -> PathBuf {
        self.config.clone().unwrap_or(
            env::current_exe()
                .expect("failed to get the current exe path")
                .parent()
                .expect("failed to get the current exe path parent")
                .to_path_buf()
                .join("config.yaml"),
        )
    }
}
