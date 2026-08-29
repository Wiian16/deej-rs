use clap::Parser;

use crate::{args::Args, config::Config};

mod args;
mod config;

fn main() {
    let args = Args::parse();

    println!("CLI args: {:#?}", args);
    println!(
        "Config path: {:#?}",
        std::path::absolute(args.get_config_path())
    )
}
