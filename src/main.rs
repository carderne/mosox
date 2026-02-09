//! # mosox
//!
//! `mosox` is a GMPL parser and matrix generator.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use std::fs::File;
use std::io::{BufReader, BufWriter};

use mosox::{
    Format, generate_matrix, load_model_and_data, matrix_to_mps, merge_model, solve_matrix, stem,
};

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
    /// Load and output to MPS
    Generate {
        path: String,
        data_path: Option<String>,
    },
    /// Solve with HiGHS
    Solve {
        path: String,
        data_path: Option<String>,
        #[arg(short, long, default_value_t = Format::Txt)]
        format: Format,
    },
    /// Normalize an MPS file for diffing
    Normalize { input: String, output: String },
    /// Compare two normalized MPS files with epsilon tolerance
    Compare {
        expected: String,
        actual: String,
        #[arg(short, long, default_value_t = 0.02)]
        epsilon: f64,
    },
}

fn run() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    match &cli.command {
        Commands::Check { path, data_path } => {
            let entries = load_model_and_data(path, data_path.as_deref())?;
            merge_model(entries);
            Ok(())
        }
        Commands::Generate { path, data_path } => {
            let entries = load_model_and_data(path, data_path.as_deref())?;
            let model = merge_model(entries);
            let compiled = generate_matrix(model);
            matrix_to_mps(compiled, stem(path));
            Ok(())
        }
        Commands::Solve {
            path,
            data_path,
            format,
        } => {
            let entries = load_model_and_data(path, data_path.as_deref())?;
            let model = merge_model(entries);
            let compiled = generate_matrix(model);
            solve_matrix(compiled, format.clone());
            Ok(())
        }
        Commands::Normalize { input, output } => {
            let reader = BufReader::new(File::open(input).expect("cannot open input file"));
            let writer = BufWriter::new(File::create(output).expect("cannot create output file"));
            mosox::normalize::normalize_mps(reader, writer);
            Ok(())
        }
        Commands::Compare {
            expected,
            actual,
            epsilon,
        } => {
            let exp_reader =
                BufReader::new(File::open(expected).expect("cannot open expected file"));
            let act_reader = BufReader::new(File::open(actual).expect("cannot open actual file"));
            let diffs = mosox::normalize::compare_mps(exp_reader, act_reader, *epsilon);
            if diffs.is_empty() {
                println!("Files match within epsilon {epsilon}");
            } else {
                let shown = if diffs.len() > 20 {
                    &diffs[..20]
                } else {
                    &diffs
                };
                for d in shown {
                    println!("{d}");
                }
                if diffs.len() > 20 {
                    println!("... and {} more", diffs.len() - 20);
                }
                println!("\n{} total differences", diffs.len());
            }
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    if let Err(e) = run() {
        eprintln!("Error: {e:#}"); // {:#} prints full error chain
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
