mod cmd_basic;
mod cmd_dsk;
mod cpm;
mod dsk;
mod file_arg;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process::exit;

#[derive(Parser)]
#[command(name = "judim")]
#[command(about = "Junior Disk Image Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Disk image operations
    #[command(about = "Disk image operations (ls, get, cp)")]
    Dsk(cmd_dsk::DskArgs),

    /// BASIC file operations
    #[command(about = "BASIC file operations")]
    Basic(cmd_basic::BasicArgs),
}

fn cli() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dsk(args) => cmd_dsk::dsk(args),
        Commands::Basic(args) => cmd_basic::basic(args),
    }
}

fn main() {
    let result = cli();
    if let Err(e) = result {
        println!("Error: {:?}", e);
        exit(1);
    }
}
