use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};

// extern crate mptk;
use mptk::model::ModelWithData;
use mptk::mps::compile_mps;
use mptk::mps::output::print_mps;
use mptk::util::stem;
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
    /// Load and output to MPS
    Comp {
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
            let _ = check(path, data_path, verbose);
            set_exit()
        }
        Commands::Comp { path, data_path } => {
            let t_total = Instant::now();

            let t0 = Instant::now();
            let model = check(path, data_path, &false).unwrap();
            eprintln!("load: {:?}", t0.elapsed());

            let t1 = Instant::now();
            let compiled = compile_mps(model);
            eprintln!("compile: {:?}", t1.elapsed());

            let t2 = Instant::now();
            print_mps(compiled, stem(path));
            eprintln!("print: {:?}", t2.elapsed());

            eprintln!("total: {:?}", t_total.elapsed());
            set_exit()
        }
    }
}

fn check(
    path: &str,
    data_path: &Option<String>,
    verbose: &bool,
) -> Result<ModelWithData, ExitCode> {
    let model_entries = load_model(path);
    let data_entries = if let Some(data_path) = data_path {
        load_data(data_path)
    } else {
        vec![]
    };
    let entries = [&model_entries[..], &data_entries[..]].concat();

    // Build the model with matched data
    let model = ModelWithData::from_entries(entries);

    // Print the model
    if *verbose {
        println!("{:#?}", model);
    }
    Ok(model)
}
