mod cpm;
mod dsk;
mod file_arg;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use prettytable::{format, row, Table};
use std::fs::File;
use std::path::Path;
use std::process::exit;

use cpm::{CpmFs, FileItem, LsMode, Params};
use fast_glob::glob_match;
use file_arg::FileArg;

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

    /// Copy files
    #[command(about = "Copy file or files to/from the disk image")]
    Cp(CpArgs),
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

#[derive(Args)]
struct CpArgs {
    /// text mode (trim at ^Z)
    #[arg(short, long)]
    text: bool,
    /// source files
    #[arg(required = true)]
    src_files: Vec<FileArg>,
    /// destination file or directory (must be directory for multiple sources)
    #[arg(required = true)]
    dst_file: FileArg,
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
            let mut lf = File::create(local_file)?;
            fs.read_file(f, &mut lf, args.text)
        }
        _ => {
            if !target_path.is_dir() {
                bail!("Multiple files match, target must be a directory.");
            }
            for f in &files {
                let mut lf = File::create(&target_path.join(&f.name))?;
                fs.read_file(f, &mut lf, args.text)?;
            }
            Ok(())
        }
    }
}

fn cp_files(fs: &CpmFs, args: CpArgs) -> Result<()> {
    match &args.dst_file {
        FileArg::Local { path } => cp_files_from_image(fs, &path, &args),
        FileArg::Image { .. } => cp_files_to_image(fs, &args),
    }
}

fn cp_files_from_image(fs: &CpmFs, dst: &Path, args: &CpArgs) -> Result<()> {
    let sources = args
        .src_files
        .iter()
        .map(|f| {
            let FileArg::Image { owner, name } = f else {
                bail!("All sources must be on the image if copying from the image to the local filesystem.");
            };
            let Some(name) = name else {
                dbg!(f);
                bail!("Source argument is missing the file name.");
            };

            let files: Vec<FileItem> = fs
                .list_files(LsMode::OwnedBy(*owner))?
                .into_iter()
                .filter(|file| glob_match(name, &file.name))
                .collect();

            Ok(files)
        })
        .try_fold(vec![], |mut files, i| {
            i.map(|chunk| {
                files.extend(chunk);
                files
            })
        })?;

    if sources.len() > 1 && !dst.is_dir() {
        bail!("Multiple source files match, target must be a directory.");
    }

    for s in &sources {
        let local_file = if dst.is_dir() {
            dst.join(&s.name)
        } else {
            dst.to_owned()
        };
        let mut lf = File::create(local_file)?;
        fs.read_file(s, &mut lf, args.text)?
    }

    Ok(())
}

fn cp_files_to_image(fs: &CpmFs, args: &CpArgs) -> Result<()> {
    if (&args.src_files).iter().any(|f| !f.is_local()) {
        bail!("All sources must be on the local filesystem if copying to the image.")
    }

    Ok(())
}

fn cli() -> Result<()> {
    let cli = Cli::parse();
    let mut file = File::open(&cli.image_file).context("Can't open image file")?;

    let params = Params {
        sectors_per_track: 9,
        reserved_tracks: 2,
        sector_size: 512,
        sectors_per_block: 4,
        dir_blocks: 4,
    };
    let fs = CpmFs::load(&mut file, params).context("Error loading image file")?;

    match cli.command {
        Commands::Ls(args) => ls(&fs, args),
        Commands::Get(args) => get_files(&fs, args),
        Commands::Cp(args) => cp_files(&fs, args),
    }
}

fn main() {
    let result = cli();
    if let Err(e) = result {
        println!("Error: {:?}", e);
        exit(1);
    }
}
