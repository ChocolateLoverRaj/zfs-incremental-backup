mod backup;
mod init_cli;
mod parse_storage_class;
mod run;
mod run_cli;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init(init_cli::Cli),
    Run(run_cli::Cli),
}

#[tokio::main]
async fn main() {
    let Cli { command } = Cli::parse();
    match command {
        Commands::Init(command) => init_cli::init_cli(command).await,
        Commands::Run(command) => run_cli::run_cli(command).await,
    }
}
