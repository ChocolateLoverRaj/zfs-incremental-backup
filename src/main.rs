use backup_command::{backup_commands, BackupCommand};
use change_password_command::{change_password_command, ChangePasswordCommand};
use check_password_command::{check_password_command, CheckPasswordCommand};
use clap::{Parser, Subcommand};
use init_command::{init_command, InitCommand};
use recover_config_command::{recover_config_command, RecoverConfigCommand};
use status_command::{status_command, StatusCommand};

mod aws_s3_prices;
mod aws_test;
mod backup_command;
mod backup_config;
mod backup_data;
mod backup_steps;
mod change_password_command;
mod check_password_command;
mod chunks_stream;
mod config;
mod create_bucket;
mod create_immutable_key;
mod create_sqs;
mod decrypt_immutable_key;
mod derive_key;
mod diff_entry;
mod diff_or_first;
mod encrypt_stream;
mod encryption_password;
mod encryption_test;
mod file_meta_data;
mod get_account_id;
mod get_config;
mod get_data;
mod get_encrypted_snapshot_name;
mod get_hasher;
mod get_snapshot_len;
mod init_command;
mod init_encryption_data;
mod optimize_diff_entries;
mod read_dir_recursive;
mod recover_config_command;
mod remote_hot_data;
mod restore_command;
mod retry_steps_2;
mod serde_file;
mod set_s3_notifications;
mod sleep_with_spinner;
mod snapshot_upload_stream_2;
mod status_command;
mod zfs_list_snapshots;
mod zfs_mount_get;
mod zfs_take_snapshot;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init(InitCommand),
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
    CheckPassword(CheckPasswordCommand),
    ChangePassword(ChangePasswordCommand),
    Status(StatusCommand),
    RecoverConfig(RecoverConfigCommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init(command) => init_command(command).await?,
        Commands::Backup { command } => backup_commands(command).await?,
        Commands::CheckPassword(command) => check_password_command(command).await?,
        Commands::ChangePassword(command) => change_password_command(command).await?,
        Commands::Status(command) => status_command(command).await?,
        Commands::RecoverConfig(commnad) => recover_config_command(commnad).await?,
    }
    Ok(())
}
