use crate::speccy_files::{SpeccyFile, SpeccyFileType};
use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use std::io::Write;

#[derive(Args)]
pub struct TapArgs {
    /// The disk image file
    pub tap_file: String,

    #[command(subcommand)]
    pub command: TapCommands,
}

#[derive(Subcommand)]
pub enum TapCommands {
    /// Show .tap file info (list of files)
    Info,
    /// Extract individual file from the .tap file
    Extract(ExtractArgs),
    /// Extract all files from the .tap file
    Explode(ExplodeArgs),
}

#[derive(Args)]
pub struct ExtractArgs {
    /// Index of the file to extract
    #[arg(short, long)]
    pub index: usize,
    /// Output file name
    pub output_file: String,
    /// Extract only the raw header bytes
    #[arg(long = "header", conflicts_with = "only_data")]
    pub only_header: bool,
    /// Extract only the raw data bytes
    #[arg(short = 'd', long = "data")]
    pub only_data: bool,
    /// Disable autorun (Basic only)
    #[arg(short = 'n', long)]
    pub no_autorun: bool,
}

#[derive(Args)]
pub struct ExplodeArgs {
    /// Prefix for output file names
    pub prefix: String,
}

pub fn tap(args: TapArgs) -> Result<()> {
    match args.command {
        TapCommands::Info => info(&args.tap_file),
        TapCommands::Extract(ext_args) => extract(&args.tap_file, ext_args),
        TapCommands::Explode(exp_args) => explode(&args.tap_file, exp_args),
    }
}

fn info(fname: &str) -> Result<()> {
    let mut tap_file = std::fs::File::open(fname)?;
    let entries = SpeccyFile::load_tap_file(&mut tap_file)?;

    for (idx, entry) in entries.iter().enumerate() {
        println!("{idx}: \"{}\"", entry.name());
        // TODO: file offset?
        println!("    type: {}", entry.file_type());
        println!("    size: {}", entry.size());

        match entry {
            SpeccyFile::Program(p) => {
                if let Some(l) = p.get_autostart_line() {
                    println!("    autostart: {}", l)
                }
                println!("    vars offet: {}", p.vars_offset())
            }
            SpeccyFile::Code(c) => {
                println!("    load address: 0x{:04X}", c.load_address())
            }
            SpeccyFile::NumArray(n) => {
                println!("    num array - TODO")
            }
            SpeccyFile::StrArray(s) => {
                println!("    string array - TODO")
            }
            _ => {}
        }
        println!();
    }
    Ok(())
}

fn extract(fname: &str, args: ExtractArgs) -> Result<()> {
    if args.only_header && args.only_data {
        bail!("--header and --data are mutually exclusive");
    }
    let mut tap_file = std::fs::File::open(fname)?;
    let mut entries = SpeccyFile::load_tap_file(&mut tap_file)?;
    if args.index >= entries.len() {
        bail!("Invalid file index");
    }

    let entry = &mut entries[args.index];
    let mut out_file = std::fs::File::create(args.output_file)?;

    if let SpeccyFile::Program(ref mut p) = entry {
        if args.no_autorun {
            p.disable_autorun();
        }
    }

    if args.only_header {
        entry.write_header(&mut out_file)?;
    } else if args.only_data {
        entry.write_raw_data(&mut out_file)?;
    } else {
        entry.write_header(&mut out_file)?;
        entry.write_raw_data(&mut out_file)?;
    }

    Ok(())
}

fn explode(fname: &str, args: ExplodeArgs) -> Result<()> {
    let mut tap_file = std::fs::File::open(fname)?;
    let entries = SpeccyFile::load_tap_file(&mut tap_file)?;

    for (idx, entry) in entries.iter().enumerate() {
        let ext = entry.file_type().extension();
        let out_name = format!("{}{:02}.{}", args.prefix, idx, ext);
        let mut out_file = std::fs::File::create(&out_name)?;
        entry.write_header(&mut out_file)?;
        entry.write_raw_data(&mut out_file)?;
        println!("{}: {} -> {}", idx, entry.name(), out_name);
    }

    Ok(())
}
