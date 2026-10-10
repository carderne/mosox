//! # mosox
//!
//! `mosox` is a GMPL parser and matrix generator.

mod data;
mod gmpl;
mod highs;
mod ir;
mod matrix;
mod mps;
pub mod normalize;

use std::path::Path;
use std::time::Instant;

use anyhow::Result;

pub use crate::data::{DatSource, DataSource, TableSource};
pub use crate::highs::format::Format;
use crate::highs::highs_solve;
use crate::highs::output::write_solution;
pub use crate::ir::model::Model;
pub use crate::matrix::{Compiled, GenOptions};
use crate::mps::output::{print_mps, write_mps, write_mps_to_file};

/// Load a model file and optional `.dat` file.
pub fn load_model_and_data(path: &str, data_path: Option<&str>) -> Result<(Model, DatSource)> {
    eprintln!("Loading model from {path}");
    let model = Model::from_file(path)?;
    let data = match data_path {
        Some(data_path) => {
            eprintln!("Loading data from {data_path}");
            DatSource::from_file(data_path)?
        }
        None => DatSource::default(),
    };
    Ok((model, data))
}

/// Compile the model against `data`, logging progress.
pub fn generate_matrix(
    model: &Model,
    data: impl DataSource,
    opts: &GenOptions,
) -> Result<Compiled> {
    eprintln!("Generating matrix");
    let t0 = Instant::now();
    let compiled = model.compile(data, opts)?;
    eprintln!("Matrix compiled in {:?}", t0.elapsed());

    let num_rows = compiled.cons.len();
    let num_cols = compiled.vars.len();
    let num_nonzero: usize = compiled.vars.values().map(|v| v.coeffs.len()).sum();
    eprintln!("Matrix: {num_rows} rows, {num_cols} cols, {num_nonzero} nonzero");

    Ok(compiled)
}

/// Matrix in MPS format.
pub fn matrix_to_mps_string(compiled: &Compiled, model_name: &str) -> String {
    let mut buf = Vec::new();
    write_mps(compiled, model_name, &mut buf);
    String::from_utf8(buf).expect("MPS output is UTF-8")
}

/// Print matrix in MPS format to stdout.
pub fn matrix_to_mps(compiled: &Compiled, model_name: &str) {
    eprintln!("Outputting MPS to stdout");
    print_mps(compiled, model_name);
}

/// Write matrix in MPS format to a file.
pub fn matrix_to_mps_file(
    compiled: &Compiled,
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
