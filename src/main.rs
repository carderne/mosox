use std::process::ExitCode;

use clap::{Parser, Subcommand};

// extern crate mosaic;
use mosaic::utils::print_entries;
use mosaic::{load_data, load_model};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check for errors and quit
    Check {
        path: String,
        data_path: Option<String>,
    },
}

fn set_exit() -> ExitCode {
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    env_logger::init();
    let cli = Cli::parse();
    match &cli.command {
        Commands::Check { path, data_path } => {
            let model_entries = load_model(path);
            let data_entries = if let Some(data_path) = data_path {
                load_data(data_path)
            } else {
                vec![]
            };
            let entries = [&model_entries[..], &data_entries[..]].concat();
            print_entries(&entries);
            set_exit()
        }
    }
}
