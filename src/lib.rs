#![allow(clippy::let_and_return)]

//! # mptk
//!
//! `mptk` is a GMPL parser and converter

use crate::data::Entry;

mod data;
mod grammar;
pub mod loader;
pub mod model;
pub mod utils;

/// Loads the GMPL model file at `path` into an internal representation
pub fn load_model(path: &str) -> Vec<Entry> {
    let text = std::fs::read_to_string(path).expect("cannot read file");
    let pairs = loader::parse(&text);
    let entries = loader::consume(pairs);
    entries
}

/// Loads the GMPL data file at `path` into an internal representation
pub fn load_data(path: &str) -> Vec<Entry> {
    let text = std::fs::read_to_string(path).expect("cannot read file");

    // The grammar expects (at least one) `data;` statement to separate model from data
    // But GMPL allows it to be omitted from a .dat file, so insert it to be safe
    let prefixed = format!("data;\n{text}");
    let pairs = loader::parse(&prefixed);
    let entries = loader::consume(pairs);
    entries
}
