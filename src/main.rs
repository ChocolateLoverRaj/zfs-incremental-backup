mod auto_back;
mod auto_back_cli;
mod backup;
mod command_error;
mod init_auto_back_cli;
mod parse_storage_class;
mod snap_and_back;
mod zfs_create;
mod zfs_dataset;
mod zfs_ensure_snapshot;
mod zfs_send;
mod zfs_snapshot;
mod zfs_snapshot_exists;
mod zfs_take_snapshot;
mod zpool_create;
mod zpool_destroy;
mod zpool_ensure_destroy;
mod zpool_list;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    SnapAndBack(snap_and_back::Cli),
    InitAutoBack(init_auto_back_cli::Cli),
    AutoBack(auto_back_cli::Cli),
}

#[tokio::main]
async fn main() {
    let Cli { command } = Cli::parse();
    match command {
        Commands::SnapAndBack(command) => snap_and_back::snap_and_back(command).await,
        Commands::InitAutoBack(command) => init_auto_back_cli::init_auto_back(command).await,
        Commands::AutoBack(command) => auto_back_cli::auto_back_cli(command).await,
    }
}
