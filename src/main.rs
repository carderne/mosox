//! # mosox
//!
//! `mosox` is a GMPL parser and matrix generator.

use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};

use mosox::{generate_matrix, load_model_and_data, matrix_to_mps, merge_model, stem};

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
    /// Load and output to MPS
    Generate {
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
        Commands::Check {
            path,
            data_path,
            verbose,
        } => {
            let entries = load_model_and_data(path, data_path.as_deref());
            let model = merge_model(entries);

            // Print the model
            if *verbose {
                println!("{:#?}", model);
            }
            set_exit()
        }
        Commands::Generate { path, data_path } => {
            let t_total = Instant::now();

            let t0 = Instant::now();
            let entries = load_model_and_data(path, data_path.as_deref());
            let model = merge_model(entries);

            eprintln!("load: {:?}", t0.elapsed());

            let t1 = Instant::now();
            let compiled = generate_matrix(model);
            eprintln!("compile: {:?}", t1.elapsed());

            let t2 = Instant::now();
            matrix_to_mps(compiled, stem(path));
            eprintln!("print: {:?}", t2.elapsed());

            eprintln!("total: {:?}", t_total.elapsed());
            set_exit()
        }
    }
}
