pub mod config;
mod cli;
mod ops;
mod storage;
mod drivers;
mod utils;
mod registry;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { target, name } => {
            ops::do_init(target, name)?;
        }
        Commands::Snapshot { name, password } => {
            let pw = password;
            ops::do_snapshot(&cli.scope, &name, pw)?;
        }
        Commands::Rollback { name, latest } => {
            ops::do_rollback(&cli.scope, name, latest)?;
        }
        Commands::Delete { name } => {
            ops::do_delete(&cli.scope, &name)?;
        }
        Commands::List => {
            ops::do_list()?;
        }
        Commands::Scopes => {
            ops::do_scopes()?;
        }
        Commands::Rename { new_name } => {
            ops::do_rename(&cli.scope, &new_name)?;
        }
        Commands::Version => {
            ops::do_version();
        }
    }

    Ok(())
}
