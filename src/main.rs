mod cpm;
mod dsk;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "judim")]
#[command(about = "Junior Disk Image Manager", long_about = None)]
struct Cli {
    /// The file name (first argument)
    image_file: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "List files stored in the disk image")]
    Ls,

    /// Retrieve file contents
    #[command(about = "Copy a file out of the disk image")]
    Get,

    /// Write data into the file
    #[command(about = "Copy a file into the disk image")]
    Put,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ls => {
            println!("Listing details for file: {}", cli.image_file);
        }
        Commands::Get => {
            println!("Getting contents of file: {}", cli.image_file);
        }
        Commands::Put => {
            println!("Putting data into file: {}", cli.image_file);
        }
    }
}
