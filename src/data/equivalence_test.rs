//! Every example must give identical MPS whether its data comes from `.dat` or from tables.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray};
use lasso::Spur;

use super::{DatSource, DataSource, Layered, TableSource, set_dimen};
use crate::ir::interner::intern_resolve;
use crate::ir::{self, Index, ParamVal, SetVal, SetValTerminal};
use crate::matrix::GenOptions;
use crate::{Model, matrix_to_mps_string};

const OPTS: GenOptions = GenOptions {
    prune: true,
    check: true,
};

#[test]
fn table_source_matches_dat() {
    let mut cases = 0;
    for dir in model_dirs(Path::new("examples")) {
        let large = dir.ends_with("osemosys_large") || dir.ends_with("osemosys_atlantis");
        if large && std::env::var("MOSOX_TEST_LARGE").is_err() {
            continue;
        }
        let model_path = files_with_ext(&dir, "mod").remove(0);
        let mut dats: Vec<Option<PathBuf>> =
            files_with_ext(&dir, "dat").into_iter().map(Some).collect();
        if dats.is_empty() {
            dats.push(None);
        }
        for dat in dats {
            for all_strings in [false, true] {
                check_example(&model_path, dat.as_deref(), all_strings);
                cases += 1;
            }
        }
    }
    assert!(cases > 15, "only {cases} cases found");
}

fn check_example(model_path: &Path, dat: Option<&Path>, all_strings: bool) {
    let label = format!(
        "{} {dat:?} (all_strings={all_strings})",
        model_path.display()
    );
    let load_dat = || {
        dat.map_or_else(DatSource::default, |p| {
            DatSource::from_file(p.to_str().unwrap()).unwrap()
        })
    };

    let model = Model::from_file(model_path.to_str().unwrap()).unwrap();
    let expected = model.compile(load_dat(), &OPTS);

    // Model without its own data, so only the tables are used
    let mut bare = Model::from_file(model_path.to_str().unwrap()).unwrap();
    let own = std::mem::take(&mut bare.data);
    let Some(tables) = to_tables(&bare, Layered(load_dat(), own), all_strings) else {
        // Ragged data (eg a malformed key) can't be expressed as a table
        assert!(expected.is_err(), "{label}: ragged data compiled from .dat");
        return;
    };
    let actual = bare.compile(tables, &OPTS);

    match (expected, actual) {
        (Ok(expected), Ok(actual)) => {
            let expected = matrix_to_mps_string(&expected, "m");
            let actual = matrix_to_mps_string(&actual, "m");
            if let Some((i, (e, a))) = expected
                .lines()
                .zip(actual.lines())
                .enumerate()
                .find(|(_, (e, a))| e != a)
            {
                panic!("{label}: line {i} differs\n  dat:   {e}\n  table: {a}");
            }
            assert_eq!(expected.len(), actual.len(), "{label}: MPS lengths differ");
        }
        (Err(_), Err(_)) => {}
        (expected, actual) => panic!(
            "{label}: dat {} but table {}",
            outcome(&expected),
            outcome(&actual)
        ),
    }
}

fn outcome<T>(r: &anyhow::Result<T>) -> String {
    match r {
        Ok(_) => "succeeded".into(),
        Err(e) => format!("failed: {e:#}"),
    }
}

/// Flatten each set and param into a table, as a user would build them
fn to_tables(model: &Model, mut source: impl DataSource, all_strings: bool) -> Option<TableSource> {
    let mut tables = TableSource::new();
    let decls: HashMap<Spur, &ir::Set> = model.sets.iter().map(|s| (s.name, s)).collect();
    for decl in &model.sets {
        let dimen = set_dimen(decl, &decls);
        let Some(values) = source.take_set(decl, dimen).unwrap() else {
            continue;
        };
        let Some(arity) = values.keys().next().map(|k| k.len()) else {
            continue;
        };
        if values.keys().any(|k| k.len() != arity) {
            return None;
        }
        let mut rows = vec![];
        for (index, vals) in &values {
            for val in vals.iter() {
                let mut row = cells(index);
                match val {
                    SetVal::Tuple(t) => row.extend(t.iter().map(terminal)),
                    val => row.push(cell(val)),
                }
                rows.push(row);
            }
        }
        tables.add(
            intern_resolve(decl.name),
            batch(rows, arity + dimen, all_strings),
        );
    }
    for decl in &model.params {
        let Some(param) = source.take_param(decl).unwrap() else {
            continue;
        };
        let name = intern_resolve(decl.name);
        if let Some(default) = param.default {
            let ParamVal::Num(n) = default else {
                panic!("symbolic data default for {name}")
            };
            tables.set_default(name, n);
        }
        let Some(arity) = param.values.keys().next().map(|k| k.len()) else {
            continue;
        };
        if param.values.keys().any(|k| k.len() != arity) {
            return None;
        }
        let rows = (param.values.iter())
            .map(|(index, val)| {
                let mut row = cells(index);
                row.push(*val);
                row
            })
            .collect();
        tables.add(name, batch(rows, arity + 1, all_strings));
    }
    Some(tables)
}

fn cells(index: &Index) -> Vec<ParamVal> {
    index.iter().map(cell).collect()
}

fn cell(val: &SetVal) -> ParamVal {
    match val {
        SetVal::Int(n) => ParamVal::Num(*n as f64),
        SetVal::Str(s) => ParamVal::Str(*s),
        SetVal::Tuple(_) => panic!("tuple in index"),
    }
}

fn terminal(val: &SetValTerminal) -> ParamVal {
    cell(&SetVal::from(val))
}

/// Integer, float or string columns depending on contents (or all strings)
fn batch(rows: Vec<Vec<ParamVal>>, ncols: usize, all_strings: bool) -> RecordBatch {
    let columns = (0..ncols).map(|c| {
        let col: Vec<ParamVal> = rows.iter().map(|r| r[c]).collect();
        let nums: Option<Vec<f64>> = (col.iter())
            .map(|v| match v {
                ParamVal::Num(n) => Some(*n),
                ParamVal::Str(_) => None,
            })
            .collect();
        let array: ArrayRef = match nums {
            Some(nums) if !all_strings && nums.iter().all(|n| n.fract() == 0.0) => {
                Arc::new(Int64Array::from_iter_values(nums.iter().map(|n| *n as i64)))
            }
            Some(nums) if !all_strings => Arc::new(Float64Array::from(nums)),
            _ => Arc::new(StringArray::from_iter_values(col.iter().map(|v| match v {
                ParamVal::Num(n) => n.to_string(),
                ParamVal::Str(s) => intern_resolve(*s).to_string(),
            }))),
        };
        (format!("c{c}"), array)
    });
    RecordBatch::try_from_iter(columns).unwrap()
}

fn model_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![];
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    if !files_with_ext(dir, "mod").is_empty() {
        dirs.push(dir.to_path_buf());
    }
    for entry in entries.into_iter().filter(|p| p.is_dir()) {
        dirs.extend(model_dirs(&entry));
    }
    dirs
}

fn files_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = (std::fs::read_dir(dir).unwrap())
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    files.sort();
    files
}
