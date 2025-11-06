use std::process::ExitCode;

use clap::{Parser, Subcommand};

// extern crate mptk;
use mptk::model::ModelWithData;
use mptk::{load_data, load_model};

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
        /// Display full Debug output instead of concise Display output
        #[arg(short, long)]
        verbose: bool,
    },
}

fn set_exit() -> ExitCode {
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    env_logger::init();
    let cli = Cli::parse();
    match &cli.command {
        Commands::Check {
            path,
            data_path,
            verbose,
        } => {
            let model_entries = load_model(path);
            let data_entries = if let Some(data_path) = data_path {
                load_data(data_path)
            } else {
                vec![]
            };
            let entries = [&model_entries[..], &data_entries[..]].concat();

            // Build the model with matched data
            let model = match ModelWithData::from_entries(&entries) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    return ExitCode::FAILURE;
                }
            };

            // Print the model
            if *verbose {
                println!("{:#?}", model);
            } else {
                print!("{}", model);
            }

            set_exit()
        }
    }
}
