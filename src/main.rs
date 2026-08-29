use clap::Parser;

use crate::{args::Args, config::Config};

mod args;
mod config;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("CLI args: {:#?}", args);
    println!(
        "Config path: {:#?}",
        std::path::absolute(args.get_config_path())
    );

    let config = Config::from_file(args.get_config_path())?;
    println!("Config: {:#?}", config);

    Ok(())
}
