mod cpm;
mod dsk;

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use prettytable::{format, row, Table};
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::exit;

use crate::cpm::{CpmFs, FileItem, LsMode, Params};
use fast_glob::glob_match;

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
    Get(GetArgs),

    /// Write data into the file
    #[command(about = "Copy a file into the disk image")]
    Put,
}

#[derive(Clone, ValueEnum, Debug, PartialEq)]
enum LsFormat {
    /// Only file names, as ls -1 on Linux
    Simple,
    /// Default tabular format with user ID and file size
    Default,
    /// As default, but with block list
    Verbose,
}

#[derive(Args)]
struct LsArgs {
    /// Include deleted files
    #[arg(short, long)]
    deleted: bool,
    /// Filter by the user number
    #[arg(short, long)]
    user: Option<u8>,
    /// Output format
    #[arg(short, long, value_enum, default_value_t = LsFormat::Default)]
    format: LsFormat,
    /// Glob expression to filter the files
    glob: Option<String>,
}

#[derive(Args)]
struct GetArgs {
    /// user number (default 0)
    #[arg(short, long)]
    user: Option<u8>,
    /// text mode (trim at ^Z)
    #[arg(short, long)]
    text: bool,
    /// file or glob
    image_file: String,
    /// local file name or path
    local_path: String,
}

fn ls(fs: &CpmFs, args: LsArgs) -> Result<()> {
    if args.deleted && args.user.is_some() {
        bail!("--deleted and --user options are mutually exclusive");
    }

    let mode = if args.deleted {
        LsMode::Deleted
    } else if let Some(user) = args.user {
        LsMode::OwnedBy(user)
    } else {
        LsMode::All
    };

    let mut files = fs.list_files(mode)?;
    if let Some(glob) = args.glob {
        files = files.into_iter().filter(|file| glob_match(&glob, &file.name)).collect();
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));

    match args.format {
        LsFormat::Simple => {
            for f in files {
                println!("{}", f.name);
            }
        }
        LsFormat::Default | LsFormat::Verbose => {
            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

            if args.format == LsFormat::Verbose {
                table.set_titles(row!["User", "Name", "Size", "Blocks"]);
            } else {
                table.set_titles(row!["User", "Name", "Size",]);
            }

            for f in files {
                let user = if let Some(u) = f.user {
                    u.to_string()
                } else {
                    "-".to_string()
                };
                if args.format == LsFormat::Verbose {
                    let blocks = f.block_list.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(",");
                    table.add_row(row![user, f.name, f.size, blocks]);
                } else {
                    table.add_row(row![user, f.name, f.size]);
                }
            }
            table.printstd();
        }
    };

    Ok(())
}

fn get_files(fs: &CpmFs, args: GetArgs) -> Result<()> {
    let files: Vec<FileItem> = fs
        .list_files(LsMode::OwnedBy(args.user.unwrap_or(0)))?
        .into_iter()
        .filter(|file| glob_match(&args.image_file, &file.name))
        .collect();
    let target_path = Path::new(&args.local_path);

    match files.len() {
        0 => {
            bail!("No files on the image matches {}.", args.image_file);
        }
        1 => {
            let f = &files[0];
            let local_file = if target_path.is_dir() {
                target_path.join(&f.name)
            } else {
                target_path.to_owned()
            };
            get_single_file(fs, f, &local_file, args.text)
        }
        _ => {
            if !target_path.is_dir() {
                bail!("Multiple files match, target must be a directory.");
            }
            for f in &files {
                let local_file = target_path.join(&f.name);
                get_single_file(fs, f, &local_file, args.text)?;
            }
            Ok(())
        }
    }
}

fn get_single_file(fs: &CpmFs, item: &FileItem, target: &Path, text_mode: bool) -> Result<()> {
    let mut f = File::create(&target)?;
    let block_size = fs.block_size();
    let mut buf = vec![0; block_size];

    let mut size_left = item.size;
    for block in &item.block_list {
        fs.read_block(*block, &mut buf)?;

        // All chunks are of block_size bytes, except the last one,
        // which can be shorter.
        let chunk_size = min(size_left, block_size);
        let chunk = &buf[0..chunk_size];

        // In text mode we trim the file at first ^Z (0x1A) character.
        if text_mode {
            // It should happen in the last chunk, but it makes little sense checking that.
            // Just write the bytes up to (not including) ^Z and return.
            if let Some(trim_at) = chunk.iter().position(|&a| a == 0x1A) {
                f.write_all(&chunk[0..trim_at])?;
                return Ok(());
            }
        }

        f.write_all(&buf[0..chunk_size])?;
        size_left -= chunk_size;
    }
    assert_eq!(size_left, 0);
    Ok(())
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
    let fs = CpmFs::load(&mut file, params).unwrap();

    let result = match cli.command {
        Commands::Ls(args) => ls(&fs, args),
        Commands::Get(args) => get_files(&fs, args),
        Commands::Put => {
            println!("Putting data into file: {}", &cli.image_file);
            Ok(())
        }
    };

    if let Err(e) = result {
        println!("Error: {}", e);
        exit(1);
    }
}
