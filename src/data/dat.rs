use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use lasso::Spur;
use smallvec::SmallVec;

use crate::data::{DataSource, ParamValues, SetValues};
use crate::gmpl::loader;
use crate::ir::{
    self, DomainPartVar, Entry, Index, Param, ParamData, ParamDataBody, ParamDataPlain,
    ParamDataPlainValue, ParamDataTarget, ParamVal, SetData, SetVal, SetValTerminal, SetVals,
    interner::intern_resolve,
};

/// Data from GMPL `.dat` text (or the `data;` section of a model file)
#[derive(Clone, Default)]
pub struct DatSource {
    sets: HashMap<Spur, Vec<SetData>>,
    params: HashMap<Spur, ParamData>,
}

impl DatSource {
    pub fn from_file(path: &str) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("Cannot read file: {path}"))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        // GMPL allows a .dat file to omit the `data;` statement
        let mut source = Self::default();
        for entry in loader::parse(&format!("data;\n{text}"))? {
            match entry {
                Entry::DataSet(data) => source.add_set(data),
                Entry::DataParam(data) => source.add_param(data)?,
                _ => bail!("data files can only contain set and param data"),
            }
        }
        Ok(source)
    }

    pub(crate) fn add_set(&mut self, data: SetData) {
        self.sets.entry(data.name).or_default().push(data);
    }

    pub(crate) fn add_param(&mut self, data: ParamData) -> Result<()> {
        let name = data.name;
        if self.params.insert(name, data).is_some() {
            bail!(
                "Data for param '{}' given more than once",
                intern_resolve(name)
            );
        }
        Ok(())
    }
}

impl DataSource for DatSource {
    fn names(&self) -> Vec<Spur> {
        self.sets
            .keys()
            .chain(self.params.keys())
            .copied()
            .collect()
    }

    fn take_set(&mut self, decl: &ir::Set, dimen: usize) -> Result<Option<SetValues>> {
        let Some(data) = self.sets.remove(&decl.name) else {
            return Ok(None);
        };
        data.into_iter()
            .map(|d| Ok((d.index, regroup_set_values(d.values, dimen)?)))
            .collect::<Result<_>>()
            .map(Some)
    }

    fn take_param(&mut self, decl: &ir::Param) -> Result<Option<ParamValues>> {
        let Some(data) = self.params.remove(&decl.name) else {
            return Ok(None);
        };
        let data = resolve_tabbing(data, decl)?;
        Ok(Some(ParamValues {
            values: data.body.map(body_to_data).transpose()?.unwrap_or_default(),
            default: data.default,
        }))
    }
}

/// Regroup flat set values into tuples based on dimension
/// e.g., with dimen=2: [A, 1, B, 2, ...] -> [(A,1), (B,2), ...]
fn regroup_set_values(values: SetVals, dimen: usize) -> Result<SetVals> {
    // If already tuples or dimen is 1, return as-is
    if dimen <= 1 || matches!(values.first(), Some(SetVal::Tuple(_))) {
        return Ok(values);
    }

    // Convert flat values to terminals and group them
    let terminals: Vec<SetValTerminal> = values
        .iter()
        .map(|v| {
            Ok(match v {
                SetVal::Str(s) => SetValTerminal::Str(*s),
                SetVal::Int(i) => SetValTerminal::Int(*i),
                SetVal::Tuple(_) => bail!("unexpected tuple in flat values"),
            })
        })
        .collect::<Result<Vec<SetValTerminal>>>()?;

    let tuples: Vec<SetVal> = terminals
        .chunks(dimen)
        .map(|chunk| SetVal::Tuple(SmallVec::from_slice(chunk)))
        .collect();

    Ok(SetVals(tuples))
}

/// Convert a `ParamDataBody::Tabbing` body into `Plain` using the matched
/// param's domain arity. Non-tabbing bodies pass through untouched.
fn resolve_tabbing(mut data: ParamData, decl: &Param) -> Result<ParamData> {
    let Some(body) = data.body.take() else {
        return Ok(data);
    };
    let tb = match body {
        ParamDataBody::Tabbing(tb) => tb,
        other => {
            data.body = Some(other);
            return Ok(data);
        }
    };

    let parts = decl
        .domain
        .as_ref()
        .map(|d| d.parts.as_slice())
        .unwrap_or(&[]);

    // Tuple-var domain parts (e.g. `(t, y) in TECH_YEAR`) would make the true
    // tuple arity > parts.len(), breaking our row chunking. Bail instead of
    // silently producing wrong data.
    for part in parts {
        if matches!(part.var, DomainPartVar::Tuple(_)) {
            bail!(
                "tabbing data for param '{}' with tuple-var domain \
                 (e.g. `(x,y) in SET`) is not supported",
                intern_resolve(data.name)
            );
        }
    }

    let n = parts.len();
    let stride = n + tb.num_cols;
    if stride == 0 || tb.values.len() % stride != 0 {
        bail!(
            "tabbing row length mismatch for param '{}': n+k={} does not divide {} values",
            intern_resolve(data.name),
            stride,
            tb.values.len()
        );
    }

    let plain: Vec<ParamDataPlain> = tb
        .values
        .chunks(stride)
        .map(|chunk| {
            let target: Vec<ParamDataTarget> = chunk[..n]
                .iter()
                .map(|v| {
                    Ok(ParamDataTarget::IndexVar(param_val_to_set_val(
                        *v, data.name,
                    )?))
                })
                .collect::<Result<_>>()?;
            Ok(ParamDataPlain {
                target: Some(target),
                value: ParamDataPlainValue::Scalar(chunk[n + tb.column]),
            })
        })
        .collect::<Result<_>>()?;

    data.body = Some(ParamDataBody::Plain(plain));
    Ok(data)
}

fn param_val_to_set_val(v: ParamVal, param_name: lasso::Spur) -> Result<SetVal> {
    match v {
        ParamVal::Str(s) => Ok(SetVal::Str(s)),
        ParamVal::Num(n) => {
            if n.fract() == 0.0 && n >= 0.0 && n <= u32::MAX as f64 {
                Ok(SetVal::Int(n as u32))
            } else {
                bail!(
                    "tabbing tuple value for param '{}' is not a valid set index (got {})",
                    intern_resolve(param_name),
                    n
                )
            }
        }
    }
}

fn body_to_data(body: ParamDataBody) -> Result<HashMap<Index, ParamVal>> {
    let mut arr: HashMap<Index, ParamVal> = HashMap::new();
    match body {
        ParamDataBody::Plain(plain_entries) => {
            for entry in plain_entries {
                let target_idxs = param_target_to_index(entry.target);
                match entry.value {
                    ParamDataPlainValue::Scalar(val) => {
                        arr.insert(target_idxs.into(), val);
                    }
                    ParamDataPlainValue::Pairs(pairs) => {
                        for pair in pairs {
                            arr.insert(
                                [target_idxs.clone(), vec![pair.key]].concat().into(),
                                pair.value,
                            );
                        }
                    }
                }
            }
        }
        ParamDataBody::Tabbing(_) => {
            // Tabbing is resolved to Plain during model merge; reaching
            // here means resolve_tabbing was skipped.
            bail!("internal: unresolved tabbing body reached matrix resolution")
        }
        ParamDataBody::Tabular(tables) => {
            for table in tables {
                let target_idxs = param_target_to_index(table.target);
                for row in table.rows {
                    for (col, value) in table.cols.iter().zip(row.values.iter()) {
                        arr.insert(
                            [target_idxs.clone(), vec![row.label.clone(), col.clone()]]
                                .concat()
                                .into(),
                            *value,
                        );
                    }
                }
            }
        }
    }
    Ok(arr)
}

fn param_target_to_index(target: Option<Vec<ParamDataTarget>>) -> Vec<SetVal> {
    // Expressions like:
    // [Atlantis_00A,NGCC,NOx,*,*]:
    // Become prefixes for the indexes down below
    match target {
        Some(targets) => targets
            .into_iter()
            .filter_map(|t| match t {
                ParamDataTarget::IndexVar(idx) => Some(idx),
                ParamDataTarget::Any => None,
            })
            .collect(),
        None => vec![],
    }
}
