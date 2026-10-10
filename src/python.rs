use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray};
use indexmap::IndexMap;
use lasso::Spur;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3_arrow::PyTable;

use crate::highs::{SolutionRow, highs_solve};
use crate::ir::interner::intern_resolve;
use crate::ir::{Domain, DomainPartVar, SetAtom, SetExpr, SetVal};
use crate::{Compiled, GenOptions, Model, TableSource, matrix_to_mps_string, stem};

create_exception!(_mosox, MosoxError, PyException);

fn to_py_err(e: anyhow::Error) -> PyErr {
    MosoxError::new_err(format!("{e:#}"))
}

type Tables<'py> = Vec<(String, Bound<'py, PyAny>)>;

#[pyclass(name = "Model", frozen)]
struct PyModel {
    model: Model,
    name: String,
}

#[pymethods]
impl PyModel {
    #[new]
    fn new(text: &str) -> PyResult<Self> {
        let model = Model::parse(text).map_err(to_py_err)?;
        Ok(Self {
            model,
            name: "model".into(),
        })
    }

    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        let model = Model::from_file(path).map_err(to_py_err)?;
        Ok(Self {
            model,
            name: stem(path).into(),
        })
    }

    fn to_mps(
        &self,
        py: Python,
        data: HashMap<String, PyTable>,
        prune: bool,
        check: bool,
    ) -> PyResult<String> {
        let compiled = self.compile(py, data, prune, check)?;
        Ok(matrix_to_mps_string(&compiled, &self.name))
    }

    /// Returns the objective value, then a table per variable and per constraint
    fn solve<'py>(
        &self,
        py: Python<'py>,
        data: HashMap<String, PyTable>,
        prune: bool,
        check: bool,
        options: Vec<(String, String)>,
        verbose: bool,
    ) -> PyResult<(f64, Tables<'py>, Tables<'py>)> {
        let compiled = self.compile(py, data, prune, check)?;
        let solution = py
            .detach(|| highs_solve(compiled, &options, verbose))
            .map_err(to_py_err)?;
        let model = &self.model;
        let var_columns = model
            .vars
            .iter()
            .map(|v| (v.name, columns(v.domain.as_ref())));
        let con_columns = (model.constraints.iter()).map(|c| (c.name, columns(c.domain.as_ref())));
        Ok((
            solution.objective_value,
            tables(py, &solution.variables, &var_columns.collect())?,
            tables(py, &solution.constraints, &con_columns.collect())?,
        ))
    }
}

impl PyModel {
    fn compile(
        &self,
        py: Python,
        data: HashMap<String, PyTable>,
        prune: bool,
        check: bool,
    ) -> PyResult<Compiled> {
        let mut source = TableSource::new();
        for (name, table) in data {
            for batch in table.into_inner().0 {
                source.add(&name, batch);
            }
        }
        let opts = GenOptions { prune, check };
        py.detach(|| self.model.compile(source, &opts))
            .map_err(to_py_err)
    }
}

/// Index column names from a declaration's domain dummies, eg `{p in P, (a,b) in AB}`
fn columns(domain: Option<&Domain>) -> Vec<String> {
    let mut cols = vec![];
    for part in domain.map_or(&[][..], |d| &d.parts) {
        match &part.var {
            DomainPartVar::Single(var) => cols.push(intern_resolve(*var).to_string()),
            DomainPartVar::Tuple(vars) => {
                cols.extend(vars.iter().map(|v| intern_resolve(*v).to_string()))
            }
            DomainPartVar::None => cols.push(match &part.expr {
                SetExpr::Atom(SetAtom::Ref(r)) => intern_resolve(r.spur).to_string(),
                _ => String::new(),
            }),
        }
    }
    cols
}

/// One pyarrow table per name, with index columns then `value` and `marginal`
fn tables<'py>(
    py: Python<'py>,
    rows: &[SolutionRow],
    columns: &HashMap<Spur, Vec<String>>,
) -> PyResult<Tables<'py>> {
    let mut grouped: IndexMap<Spur, Vec<&SolutionRow>> = IndexMap::new();
    for row in rows {
        grouped.entry(row.id.0).or_default().push(row);
    }
    grouped
        .into_iter()
        .map(|(name, rows)| {
            let arity = rows[0].id.1.len();
            let names = valid_names(columns.get(&name), arity);
            let mut cols: Vec<(String, ArrayRef)> = (names.into_iter().enumerate())
                .map(|(c, col_name)| (col_name, index_column(&rows, c)))
                .collect();
            let value = Float64Array::from_iter_values(rows.iter().map(|r| r.value));
            let marginal = Float64Array::from_iter_values(rows.iter().map(|r| r.marginal));
            cols.push(("value".into(), Arc::new(value)));
            cols.push(("marginal".into(), Arc::new(marginal)));
            let batch =
                RecordBatch::try_from_iter(cols).map_err(|e| MosoxError::new_err(e.to_string()))?;
            let schema = batch.schema();
            let table = PyTable::try_new(vec![batch], schema)?.into_pyarrow(py)?;
            Ok((intern_resolve(name).to_string(), table))
        })
        .collect()
}

/// Domain names if usable as distinct column names, else `i0, i1, ...`
fn valid_names(names: Option<&Vec<String>>, arity: usize) -> Vec<String> {
    if let Some(names) = names {
        let mut seen = HashSet::from(["value", "marginal", ""]);
        if names.len() == arity && names.iter().all(|n| seen.insert(n)) {
            return names.clone();
        }
    }
    (0..arity).map(|i| format!("i{i}")).collect()
}

fn index_column(rows: &[&SolutionRow], c: usize) -> ArrayRef {
    let ints: Option<Vec<i64>> = (rows.iter())
        .map(|r| match r.id.1[c] {
            SetVal::Int(n) => Some(n as i64),
            _ => None,
        })
        .collect();
    match ints {
        Some(ints) => Arc::new(Int64Array::from(ints)),
        None => Arc::new(StringArray::from_iter_values(
            rows.iter().map(|r| r.id.1[c].to_string()),
        )),
    }
}

#[pymodule]
fn _mosox(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("MosoxError", m.py().get_type::<MosoxError>())?;
    m.add_class::<PyModel>()?;
    Ok(())
}
