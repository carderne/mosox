use std::collections::HashMap;

use anyhow::{Context, Result, bail, ensure};
use arrow_array::cast::AsArray;
use arrow_array::types::*;
use arrow_array::{Array, RecordBatch};
use lasso::Spur;

use crate::data::{DataSource, ParamValues, SetValues};
use crate::ir::interner::{intern, intern_resolve};
use crate::ir::{self, Index, ParamVal, SetVal, SetValTerminal};

/// Data from in-memory Arrow tables, one per set or param.
///
/// Columns are read by position, not name:
/// - param: the index columns, then the value
/// - set: the index columns (if indexed), then one column per member dimension
///
/// Values are read as in `.dat` files: numeric columns and numeric strings are numbers
/// (and integer set elements), other strings are symbolic.
/// A null param value is treated as absent, so the default applies.
#[derive(Clone, Default)]
pub struct TableSource {
    tables: HashMap<Spur, Table>,
}

#[derive(Clone, Default)]
struct Table {
    batches: Vec<RecordBatch>,
    default: Option<f64>,
}

impl TableSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add rows for a set or param. Repeated calls append.
    pub fn add(&mut self, name: &str, batch: RecordBatch) -> &mut Self {
        self.tables
            .entry(intern(name))
            .or_default()
            .batches
            .push(batch);
        self
    }

    /// Set a param's default, as `.dat`'s `param p default 0 := ...`
    pub fn set_default(&mut self, name: &str, value: f64) -> &mut Self {
        self.tables.entry(intern(name)).or_default().default = Some(value);
        self
    }
}

impl DataSource for TableSource {
    fn names(&self) -> Vec<Spur> {
        self.tables.keys().copied().collect()
    }

    fn take_set(&mut self, decl: &ir::Set, dimen: usize) -> Result<Option<SetValues>> {
        let Some(table) = self.tables.remove(&decl.name) else {
            return Ok(None);
        };
        table.num_columns()?;
        let mut values = SetValues::new();
        for batch in &table.batches {
            let cols = columns(batch)?;
            ensure!(
                cols.len() >= dimen,
                "set has dimension {dimen} but its table has {} columns",
                cols.len()
            );
            let (index_cols, member_cols) = cols.split_at(cols.len() - dimen);
            if index_cols.is_empty() {
                values.entry(Index::new()).or_default();
            }
            for row in 0..batch.num_rows() {
                let index = index_at(index_cols, row)?;
                let member = match member_cols {
                    [col] => SetVal::from(&element(col[row])?),
                    _ => SetVal::Tuple(
                        member_cols
                            .iter()
                            .map(|col| element(col[row]))
                            .collect::<Result<_>>()?,
                    ),
                };
                values.entry(index).or_default().0.push(member);
            }
        }
        Ok(Some(values))
    }

    fn take_param(&mut self, decl: &ir::Param) -> Result<Option<ParamValues>> {
        let Some(table) = self.tables.remove(&decl.name) else {
            return Ok(None);
        };
        table.num_columns()?;
        let mut values = HashMap::new();
        for batch in &table.batches {
            let cols = columns(batch)?;
            let Some((value_col, index_cols)) = cols.split_last() else {
                bail!(
                    "table for param '{}' has no columns",
                    intern_resolve(decl.name)
                );
            };
            for (row, value) in value_col.iter().enumerate() {
                if let Some(value) = value {
                    values.insert(index_at(index_cols, row)?, *value);
                }
            }
        }
        Ok(Some(ParamValues {
            values,
            default: table.default.map(ParamVal::Num),
        }))
    }
}

impl Table {
    fn num_columns(&self) -> Result<usize> {
        let counts: Vec<usize> = self.batches.iter().map(|b| b.num_columns()).collect();
        ensure!(
            counts.windows(2).all(|w| w[0] == w[1]),
            "batches have different numbers of columns: {counts:?}"
        );
        Ok(counts.first().copied().unwrap_or(0))
    }
}

fn columns(batch: &RecordBatch) -> Result<Vec<Vec<Option<ParamVal>>>> {
    let schema = batch.schema();
    batch
        .columns()
        .iter()
        .zip(schema.fields())
        .map(|(col, field)| {
            cells(col.as_ref()).with_context(|| format!("column '{}'", field.name()))
        })
        .collect()
}

fn index_at(cols: &[Vec<Option<ParamVal>>], row: usize) -> Result<Index> {
    cols.iter()
        .map(|col| Ok(SetVal::from(&element(col[row])?)))
        .collect::<Result<_>>()
        .with_context(|| format!("row {row}"))
}

fn element(cell: Option<ParamVal>) -> Result<SetValTerminal> {
    match cell {
        None => bail!("null set element"),
        Some(ParamVal::Str(s)) => Ok(SetValTerminal::Str(s)),
        Some(ParamVal::Num(n)) if n.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&n) => {
            Ok(SetValTerminal::Int(n as u32))
        }
        Some(ParamVal::Num(n)) => bail!("{n} is not a valid set element"),
    }
}

/// Read a column as `.dat`-style values, with None for nulls
fn cells(col: &dyn Array) -> Result<Vec<Option<ParamVal>>> {
    macro_rules! numeric {
        ($($t:ty),*) => {$(
            if let Some(a) = col.as_primitive_opt::<$t>() {
                return Ok(a.iter().map(|v| v.map(|v| ParamVal::Num(v as f64))).collect());
            }
        )*};
    }
    numeric!(
        Int8Type,
        Int16Type,
        Int32Type,
        Int64Type,
        UInt8Type,
        UInt16Type,
        UInt32Type,
        UInt64Type,
        Float32Type,
        Float64Type
    );

    let strs: Option<Vec<Option<&str>>> = (col.as_string_opt::<i32>().map(|a| a.iter().collect()))
        .or_else(|| col.as_string_opt::<i64>().map(|a| a.iter().collect()))
        .or_else(|| col.as_string_view_opt().map(|a| a.iter().collect()));
    if let Some(strs) = strs {
        return Ok(strs.into_iter().map(|s| s.map(str_val)).collect());
    }

    // eg pandas categoricals
    if let Some(dict) = col.as_any_dictionary_opt() {
        let values = cells(dict.values().as_ref())?;
        if values.is_empty() {
            return Ok(vec![None; col.len()]);
        }
        return Ok(dict
            .normalized_keys()
            .into_iter()
            .enumerate()
            .map(|(i, key)| if dict.is_null(i) { None } else { values[key] })
            .collect());
    }

    bail!("unsupported column type {}", col.data_type())
}

fn str_val(s: &str) -> ParamVal {
    match is_number(s) {
        true => ParamVal::Num(s.parse().unwrap()),
        false => ParamVal::Str(intern(s)),
    }
}

/// Matches the GMPL `number` token, so strings are read as a `.dat` file would read them
fn is_number(s: &str) -> bool {
    let digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    let s = s.strip_prefix('-').unwrap_or(s);
    let (mantissa, exp) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let exp_ok = exp.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    (!int.is_empty() || !frac.is_empty()) && digits(int) && digits(frac) && exp_ok
}
