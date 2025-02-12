mod cpm;
mod dsk;

use clap::{Args, Parser, Subcommand};
use prettytable::{format, row, Table};
use std::fs::File;
use std::process::exit;

use crate::cpm::{CpmFs, LsMode, Params};

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
    #[command(
        about = "List files stored in the disk image.",
        long_about = "The 'ls' command lists the files present in the disk image. \
           \n\n\
           By default files all files are listed, except deleted ones. Use the --user option to\n\
           filter by the user number. Use the --deleted option to include deleted files.\n\n\
           Note: CP/M uses 0xE5 as a user number to mark unused directory entries.\n\
           Hence --deleted and --user options are mutually exclusive."
    )]
    Ls(LsArgs),

    /// Retrieve file contents
    #[command(about = "Copy a file out of the disk image")]
    Get,

    /// Write data into the file
    #[command(about = "Copy a file into the disk image")]
    Put,
}

#[derive(Args)]
struct LsArgs {
    /// Include deleted files
    #[arg(short, long)]
    deleted: bool,
    /// Filter by the user number
    #[arg(short, long)]
    user: Option<u8>,
}

fn ls(fs: &CpmFs, args: LsArgs) {
    if args.deleted && args.user.is_some() {
        println!("--deleted and --user options are mutually exclusive");
        exit(1);
    }

    let mode = if args.deleted {
        LsMode::Deleted
    } else if let Some(user) = args.user {
        LsMode::OwnedBy(user)
    } else {
        LsMode::All
    };

    let mut files = fs.list_files(mode).unwrap();
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(row!["User", "Size", "Name"]);
    for f in files {
        let user = if let Some(u) = f.user {
            u.to_string()
        } else {
            "-".to_string()
        };
        table.add_row(row![user, f.size, f.name]);
    }
    table.printstd();
}

fn main() {
    let cli = Cli::parse();

    let mut file = File::open(&cli.image_file).unwrap();

    let params = Params {
        sectors_per_track: 9,
        reserved_tracks: 2,
        sector_size: 512,
        sectors_per_block: 4,
        dir_blocks: 4,
    };
    let mut fs = CpmFs::load(&mut file, params).unwrap();

    match cli.command {
        Commands::Ls(args) => ls(&fs, args),
        Commands::Get => {
            println!("Getting contents of file: {}", &cli.image_file);
        }
        Commands::Put => {
            println!("Putting data into file: {}", &cli.image_file);
        }
    }
}
