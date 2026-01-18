use anyhow::Result;
use clap::{Args, Subcommand};
use crate::speccy_files::{SpeccyFile, SpeccyFileType};

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
}

pub fn tap(args: TapArgs) -> Result<()> {
    match args.command {
        TapCommands::Info => info(&args.tap_file),
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
            },
            SpeccyFile::Code(c) => {
                println!("    load address: 0x{:04X}", c.load_address())
            }
            _ => {}
        }
    
        
    }
    Ok(())
}
