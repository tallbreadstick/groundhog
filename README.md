<p align="center">
  <img src="Groundhog.png" alt="Groundhog logo" width="160" />
  <br/>
  <h1 align="center" style="margin: 0; padding: 0; font-size: 2.4rem;">Groundhog</h1>
  <p align="center" style="margin-top: 0.25rem; font-size: 1.05rem;">
    Manage point-in-time snapshots of directories and databases for rapid rollback.
    Snapshots are immutable; rollbacks restore a previous stable state without tracking intermediate changes.
  </p>
</p>

<p align="center">
  <strong>⚠️ Disclaimer:</strong> This project is not yet at a stable release (pilot <code>0.1-alpha</code>).<br/>
  Please do not use for critical or production purposes. Use at your own risk.
  
</p>

Features
- Named scopes for directories or databases
- Global scope registry (user-writable)
- Fast snapshot/rollback workflow with progress bars and colored output
- Per-scope local storage under `.groundhog/`
- Modular driver architecture (filesystem implemented, DB drivers stubbed)

Install
- Build and install locally:
```
cargo install --path .
```

Quick Start
```
# Initialize current directory as a scope (auto-named via path hash)
groundhog init

# Or initialize and assign a name
groundhog init "C:\\projects\\app" -n app

# List globally registered scopes
groundhog scopes

# Create a snapshot for a specific scope from anywhere
groundhog -s app snapshot "baseline"

# Roll back to the latest snapshot for a scope
groundhog -s app rollback --latest

# Delete a snapshot (will prompt to confirm)
groundhog -s app delete "baseline"
```

Commands

groundhog init <path|db_addr> [-n <name>]
- Initialize a `.groundhog` workspace at the given path or database target
- Register the scope globally in the central registry
- If `-n` is not provided, the scope name defaults to a hash of the target path/URI
- Examples:
```
# Current directory
groundhog init

# Specific directory with name
groundhog init /opt/lab -n lab

# Database target (driver behavior is stubbed today)
groundhog init "postgres://user:pass@host:5432/db" -n prod-db
```

groundhog -s <scope_name> snapshot "<name>" [--password <password>]
- Create a snapshot for the selected scope
- Stores snapshot data under `<scope_root>/.groundhog/store/`
- `--password` will mark the snapshot locked (encryption TODO)
- Examples:
```
groundhog -s app snapshot "baseline"
groundhog -s app snapshot "locked" --password "s3cret"
```

groundhog -s <scope_name> rollback "<name>" | --latest
- Restore the scope to the given named snapshot or the most recent one
- Applies minimal I/O (future: Merkle/diff-based optimization)
- Examples:
```
groundhog -s app rollback "baseline"
groundhog -s app rollback --latest
```

groundhog -s <scope_name> delete "<name>"
- Delete a named snapshot in the scope (prompts for confirmation)
- Example:
```
groundhog -s app delete "baseline"
```

groundhog list
- List snapshots for the local workspace (must be run inside a directory containing `.groundhog` or a descendant)
- Shows: name, type, timestamp, lock status
- Example:
```
cd /opt/lab
groundhog list
```

groundhog scopes
- List all globally registered scopes (works from any directory)
- Auto-cleans entries whose target no longer contains `.groundhog`
- Example:
```
groundhog scopes
```

groundhog -s <scope_name> rename "<new_name>"
- Rename a scope globally
- Updates the scope name in the central registry and local snapshot metadata
- Example:
```
groundhog -s app rename "app-prod"
```

groundhog version
- Print the CLI version (`0.1-alpha`)
- Example:
```
groundhog version
```

Drivers
- Filesystem driver: copies directory contents (excluding `.groundhog`)
- Database drivers (MySQL/PostgreSQL/SQLite): placeholders; implement physical or logical backup/restore as needed

Roadmap / Implementation Notes
- Diffing and Merkle-tree optimization: implement hashing and tree-building in `src/utils/hash.rs` and replace the naive copy/overwrite strategy in `src/ops.rs`
- Encryption for `--password` snapshots: add cryptographic envelope for snapshot contents and secure password prompts
- Smarter progress feedback: per-file progress, throughput, and ETA via `indicatif`

Safety Notes
- Snapshots are immutable and rollbacks overwrite state for the selected scope
- Always validate critical paths and back up important data before running destructive operations


