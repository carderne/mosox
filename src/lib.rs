//! # mosox
//!
//! `mosox` is a GMPL parser and matrix generator.

mod gmpl;
mod highs;
mod ir;
mod matrix;
mod mps;
pub mod normalize;

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

use crate::gmpl::loader;
pub use crate::highs::format::Format;
use crate::highs::highs_solve;
use crate::highs::output::write_solution;
use crate::ir::Entry;
use crate::ir::model::ModelWithData;
use crate::matrix::{Compiled, gen_matrix};
use crate::mps::output::{print_mps, write_mps_to_file};

/// Loads the GMPL model file at `path` into an internal representation
pub fn load_model(path: &str) -> Result<Vec<Entry>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("Cannot read file: {path}"))?;
    loader::parse(&text)
}

/// Loads the GMPL data file at `path` into an internal representation
pub fn load_data(path: &str) -> Result<Vec<Entry>> {
    let text = std::fs::read_to_string(path).expect("cannot read file");

    // The grammar expects (at least one) `data;` statement to separate model from data
    // But GMPL allows it to be omitted from a .dat file, so insert it to be safe
    let prefixed = format!("data;\n{text}");
    loader::parse(&prefixed)
}

/// Load model and data, calling `load_model` and `load_data`.
pub fn load_model_and_data(path: &str, data_path: Option<&str>) -> Result<Vec<Entry>> {
    eprintln!("Loading model from {path}");
    let model_entries = load_model(path)?;
    let data_entries = match data_path {
        Some(data_path) => {
            eprintln!("Loading data from {data_path}");
            load_data(data_path)?
        }
        None => vec![],
    };
    Ok(model_entries.into_iter().chain(data_entries).collect())
}

/// Merge raw model and data into a `ModelWithData`.
pub fn merge_model(entries: Vec<Entry>) -> Result<ModelWithData> {
    ModelWithData::from_entries(entries)
}

/// Convert merged model to matrix.
pub fn generate_matrix(model: ModelWithData) -> Result<Compiled> {
    eprintln!("Generating matrix");
    let t0 = Instant::now();
    let compiled = gen_matrix(model)?;
    eprintln!("Matrix compiled in {:?}", t0.elapsed());

    let num_rows = compiled.cons.len();
    let num_cols = compiled.vars.len();
    let num_nonzero: usize = compiled.vars.values().map(|v| v.coeffs.len()).sum();
    eprintln!("Matrix: {num_rows} rows, {num_cols} cols, {num_nonzero} nonzero");

    Ok(compiled)
}

/// Print matrix in MPS format to stdout.
pub fn matrix_to_mps(compiled: Compiled, model_name: &str) {
    eprintln!("Outputting MPS to stdout");
    print_mps(compiled, model_name);
}

/// Write matrix in MPS format to a file.
pub fn matrix_to_mps_file(
    compiled: Compiled,
    model_name: &str,
    path: &std::path::Path,
) -> Result<()> {
    eprintln!("Outputting MPS to {}", path.display());
    write_mps_to_file(compiled, model_name, path)?;
    Ok(())
}

/// Solve the compiled matrix with Highs
pub fn solve_matrix(
    compiled: Compiled,
    format: Format,
    config: &[(String, String)],
    verbose: bool,
    output: Option<&std::path::Path>,
) -> Result<()> {
    eprintln!("Solving matrix with HiGHS");
    let t0 = Instant::now();
    let solution = highs_solve(compiled, config, verbose)?;
    eprintln!("Solved in {:?}", t0.elapsed());
    eprintln!("Objective value: {}", solution.objective_value);
    if let Some(path) = output {
        write_solution(solution, format, path);
        eprintln!("Results output to {}", path.display());
    }
    Ok(())
}

/// Get the stem from a path.
///
/// ```
/// use mosox::stem;
/// let path = "/some/file.txt";
/// let path_stem = stem(path);
/// assert!(path_stem == "file");
/// ```
pub fn stem(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
}
