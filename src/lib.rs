//! # mosox
//!
//! `mosox` is a GMPL parser and matrix generator.

mod gmpl;
mod ir;
mod matrix;
mod mps;

use std::path::Path;

use crate::gmpl::loader;
use crate::ir::Entry;
use crate::ir::model::ModelWithData;
use crate::matrix::{Compiled, gen_matrix};
use crate::mps::output::print_mps;

/// Loads the GMPL model file at `path` into an internal representation
pub fn load_model(path: &str) -> Vec<Entry> {
    let text = std::fs::read_to_string(path).expect("cannot read file");
    let pairs = loader::parse(&text);
    loader::consume(pairs)
}

/// Loads the GMPL data file at `path` into an internal representation
pub fn load_data(path: &str) -> Vec<Entry> {
    let text = std::fs::read_to_string(path).expect("cannot read file");

    // The grammar expects (at least one) `data;` statement to separate model from data
    // But GMPL allows it to be omitted from a .dat file, so insert it to be safe
    let prefixed = format!("data;\n{text}");
    let pairs = loader::parse(&prefixed);
    loader::consume(pairs)
}

/// Load model and data, calling `load_model` and `load_data`.
pub fn load_model_and_data(path: &str, data_path: Option<&str>) -> Vec<Entry> {
    let model_entries = load_model(path);
    let data_entries = match data_path {
        Some(data_path) => load_data(data_path),
        None => vec![],
    };
    model_entries.into_iter().chain(data_entries).collect()
}

/// Merge raw model and data into a `ModelWithData`.
pub fn merge_model(entries: Vec<Entry>) -> ModelWithData {
    ModelWithData::from_entries(entries)
}

/// Convert merged model to matrix.
pub fn generate_matrix(model: ModelWithData) -> Compiled {
    gen_matrix(model)
}

/// Print matrix in MPS format to stdout.
pub fn matrix_to_mps(compiled: Compiled, model_name: &str) {
    print_mps(compiled, model_name);
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
