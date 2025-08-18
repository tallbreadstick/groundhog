use clap::{Parser, Subcommand};

/// groundhog: point-in-time snapshot manager for files and databases
#[derive(Parser, Debug)]
#[command(name = "groundhog", version, about = "Manage point-in-time snapshots of directories and databases.", long_about = None, arg_required_else_help = true)]
pub struct Cli {
    /// Target scope name (defaults to current directory scope if omitted)
    #[arg(short = 's', long = "scope")]
    pub scope: Option<String>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a .groundhog configuration at the target (or current directory)
    Init {
        /// Filesystem directory or database address (e.g., mysql://user:pass@host:port/dbname)
        target: Option<String>,
        /// Optional human-readable name for the scope
        #[arg(short = 'n', long = "name")]
        name: Option<String>,
    },

    /// Create a snapshot with a name; optionally lock with a password
    Snapshot {
        /// Name for the snapshot
        name: String,
        /// Optional password to lock/encrypt snapshot
        #[arg(long, value_name = "password")]
        password: Option<String>,
    },

    /// Roll back to a named snapshot
    Rollback {
        /// Name of snapshot
        #[arg(required_unless_present = "latest")]
        name: Option<String>,

        /// Roll back to the most recent snapshot
        #[arg(long)]
        latest: bool,
    },

    /// Delete a named snapshot
    Delete {
        /// Name of snapshot to delete
        name: String,
    },

    /// Rename the current or specified scope
    Rename {
        /// New name for the scope
        new_name: String,
    },

    /// List snapshots in the current workspace
    List,

    /// List defined scopes in the workspace
    Scopes,

    /// Print CLI version
    Version,
}


