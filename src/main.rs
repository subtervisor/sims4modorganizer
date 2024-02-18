mod commands;
mod entities;
mod migrator;
mod util;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

use clap::{Parser, Subcommand};
use tracing::info;

/// Program to manage Sims 4 mods
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initializes the database
    Initialize {
        /// Replace an existing database, if found
        #[arg(short, long)]
        force: bool,
    },
    /// Lists currently registered mods
    List {
        /// Only show mods matching the given tags
        #[arg(short, long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        /// Verify file data and show results
        #[arg(short, long)]
        verify: bool,

        /// Show detailed information
        #[arg(short, long)]
        details: bool,
    },
    /// Scans for out of date or new mods in the mod directory
    Scan {
        /// Verify file data for existing mods
        #[arg(short, long)]
        verify: bool,

        /// Interactively update the database for detected changes
        #[arg(short, long)]
        fix: bool,

        /// Update file hash data without changing mod metadata (dangerous)
        #[arg(short, long)]
        sync_hashes: bool,
    },
    /// View and delete tags
    Tags {
        /// Deletes a given tag. Does not delete any mods.
        #[arg(short, long)]
        delete: Option<String>,

        /// Only show given tags
        #[arg(short, long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
    },
    /// Edit mod information and tags
    Edit {
        /// Edit interactively. Cannot be used with other options.
        #[arg(short, long)]
        interactive: bool,

        /// Mod ID to edit
        #[arg(short, long)]
        mod_id: Option<i32>,

        /// Mod name to edit
        #[arg(short, long)]
        name: Option<String>,

        /// Source URL to set
        #[arg(short, long)]
        source_url: Option<String>,

        /// Tags to set
        #[arg(short, long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        /// Version to set
        #[arg(short = 'v', long)]
        mod_version: Option<String>,
    },
    // Open the Sims 4 mod directory in a file explorer
    OpenModDir,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!(
        "Starting sims4modorganizer version {}",
        env!("CARGO_PKG_VERSION")
    );
    let args = Args::parse();
    match args.command {
        Command::Initialize { force } => util::init_database(force).await,
        Command::List {
            tags,
            verify,
            details,
        } => commands::list(tags, verify, details).await,
        Command::Scan {
            verify,
            fix,
            sync_hashes,
        } => {
            if fix && sync_hashes {
                eprintln!("Interactive fix and hash sync are mutually exclusive.");
                std::process::exit(1);
            }
            commands::scan(None, verify, fix, sync_hashes).await
        }
        Command::Tags { delete, tags } => {
            if delete.is_some() && tags.is_some() {
                eprintln!("Delete and show tag options are mutually exclusive.");
                std::process::exit(1);
            }
            commands::tags(delete, tags).await
        }
        Command::Edit {
            interactive,
            mod_id,
            name,
            source_url,
            tags,
            mod_version,
        } => {
            if !interactive {
                if mod_id.is_none() {
                    eprintln!("Mod ID required to edit non-interactively");
                    std::process::exit(1);
                } else if name.is_none()
                    && source_url.is_none()
                    && tags.is_none()
                    && mod_version.is_none()
                {
                    eprintln!("At least one field to edit must be provided");
                    std::process::exit(1);
                }
            }
            commands::edit(interactive, mod_id, name, source_url, tags, mod_version).await
        }
        Command::OpenModDir => opener::open(util::get_sims_mod_dir()?).map_err(|e| e.into()),
    }
}
