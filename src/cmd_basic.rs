use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct BasicArgs {
    #[command(subcommand)]
    pub command: BasicCommands,
}

#[derive(Subcommand)]
pub enum BasicCommands {
    /// Dump BASIC program
    Dump,
    /// Tokenize BASIC program
    Tokenize,
}

pub fn basic(args: BasicArgs) -> Result<()> {
    match args.command {
        BasicCommands::Dump => dump(),
        BasicCommands::Tokenize => tokenize(),
    }
}

fn dump() -> Result<()> {
    Ok(())
}

fn tokenize() -> Result<()> {
    Ok(())
}
