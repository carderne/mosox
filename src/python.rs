use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray};
use indexmap::IndexMap;
use lasso::Spur;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3_arrow::PyTable;

use crate::data::{Layered, expr_dimen, set_dimen};
use crate::highs::{SolutionRow, highs_solve, status_name};
use crate::ir::interner::intern_resolve;
use crate::ir::op::RowType;
use crate::ir::{self, Domain, DomainPartVar, SetVal};
use crate::{
    Compiled, DatSource, DataSource, GenOptions, Model, TableSource, matrix_to_mps_string, stem,
};

create_exception!(_mosox, MosoxError, PyException);

fn to_py_err(e: anyhow::Error) -> PyErr {
    MosoxError::new_err(format!("{e:#}"))
}

type Tables<'py> = Vec<(String, Bound<'py, PyAny>)>;

/// Kind, name, index columns and (for sets) member dimension
type Declaration = (&'static str, String, Vec<String>, Option<usize>);

/// Parsed `.dat` data, reusable across compiles
#[pyclass(name = "Dat", frozen)]
struct PyDat(DatSource);

#[pymethods]
impl PyDat {
    #[new]
    fn new(text: &str) -> PyResult<Self> {
        DatSource::parse(text).map(Self).map_err(to_py_err)
    }

    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        DatSource::from_file(path).map(Self).map_err(to_py_err)
    }
}

/// One layer of data, each layered over those before it
#[derive(FromPyObject)]
enum Layer<'py> {
    Dat(Bound<'py, PyDat>),
    /// Tables, param defaults, and whether the tables patch lower layers' params
    Tables(HashMap<String, PyTable>, HashMap<String, f64>, bool),
}

type Source = Box<dyn DataSource + Send>;

fn layered(layers: Vec<Layer>) -> Source {
    let mut source: Source = Box::new(TableSource::new());
    for layer in layers {
        let top: Source = match layer {
            Layer::Dat(dat) => Box::new(dat.get().0.clone()),
            Layer::Tables(tables, defaults, patch) => {
                let mut top = TableSource::new();
                for (name, table) in tables {
                    let (batches, schema) = table.into_inner();
                    // An empty table may have no batches, but still replaces lower layers
                    if batches.is_empty() {
                        top.add(&name, RecordBatch::new_empty(schema));
                    }
                    for batch in batches {
                        top.add(&name, batch);
                    }
                    if patch {
                        top.set_patch(&name);
                    }
                }
                for (name, value) in defaults {
                    top.set_default(&name, value);
                }
                Box::new(top)
            }
        };
        source = Box::new(Layered(top, source));
    }
    source
}

#[pyclass(name = "Model", frozen)]
struct PyModel {
    model: Model,
    name: String,
    /// Result table index columns for each var and constraint
    columns: Arc<HashMap<Spur, Vec<String>>>,
}

#[pymethods]
impl PyModel {
    #[new]
    fn new(text: &str) -> PyResult<Self> {
        let model = Model::parse(text).map_err(to_py_err)?;
        Ok(Self::with_name(model, "model"))
    }

    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        let model = Model::from_file(path).map_err(to_py_err)?;
        Ok(Self::with_name(model, stem(path)))
    }

    fn compile(
        &self,
        py: Python,
        layers: Vec<Layer>,
        prune: bool,
        check: bool,
    ) -> PyResult<PyMatrix> {
        let source = layered(layers);
        let opts = GenOptions { prune, check };
        let compiled = py
            .detach(|| self.model.compile(source, &opts))
            .map_err(to_py_err)?;
        Ok(PyMatrix {
            compiled,
            name: self.name.clone(),
            columns: self.columns.clone(),
        })
    }

    fn declarations(&self) -> Vec<Declaration> {
        let model = &self.model;
        let sets = set_decls(model);
        let name = |spur| intern_resolve(spur).to_string();
        let mut decls: Vec<Declaration> = vec![];
        for set in &model.sets {
            let dimen = set_dimen(set, &sets);
            let index = columns(Some(&set.domain), &sets);
            decls.push(("set", name(set.name), index, Some(dimen)));
        }
        for param in &model.params {
            let index = columns(param.domain.as_ref(), &sets);
            decls.push(("param", name(param.name), index, None));
        }
        for var in &model.vars {
            decls.push(("var", name(var.name), self.columns[&var.name].clone(), None));
        }
        for con in &model.constraints {
            let kind = match con.row_type {
                RowType::Unconstrained => "objective",
                _ => "constraint",
            };
            decls.push((kind, name(con.name), self.columns[&con.name].clone(), None));
        }
        decls
    }
}

impl PyModel {
    fn with_name(model: Model, name: &str) -> Self {
        let sets = set_decls(&model);
        let vars = (model.vars.iter()).map(|v| (v.name, v.domain.as_ref()));
        let cons = (model.constraints.iter()).map(|c| (c.name, c.domain.as_ref()));
        let columns = (vars.chain(cons))
            .map(|(name, domain)| (name, columns(domain, &sets)))
            .collect();
        Self {
            name: name.into(),
            columns: Arc::new(columns),
            model,
        }
    }
}

/// A compiled matrix, which can be output or solved
#[pyclass(name = "Matrix", frozen)]
struct PyMatrix {
    compiled: Compiled,
    name: String,
    columns: Arc<HashMap<Spur, Vec<String>>>,
}

#[pymethods]
impl PyMatrix {
    /// Rows, including the objective
    #[getter]
    fn num_rows(&self) -> usize {
        self.compiled.cons.len()
    }

    #[getter]
    fn num_cols(&self) -> usize {
        self.compiled.vars.len()
    }

    #[getter]
    fn num_nonzeros(&self) -> usize {
        self.compiled.vars.values().map(|v| v.coeffs.len()).sum()
    }

    fn to_mps(&self, py: Python) -> String {
        py.detach(|| matrix_to_mps_string(&self.compiled, &self.name))
    }

    /// Returns the status, the objective value, then a table per variable and per constraint.
    /// `keep` limits the tables to those names.
    fn solve<'py>(
        &self,
        py: Python<'py>,
        options: Vec<(String, String)>,
        verbose: bool,
        keep: Option<HashSet<String>>,
    ) -> PyResult<(&'static str, f64, Tables<'py>, Tables<'py>)> {
        let solution = py
            .detach(|| highs_solve(&self.compiled, &options, verbose))
            .map_err(to_py_err)?;
        let kept = |rows: Vec<SolutionRow>| match &keep {
            Some(keep) => (rows.into_iter())
                .filter(|r| keep.contains(intern_resolve(r.id.0)))
                .collect(),
            None => rows,
        };
        Ok((
            status_name(solution.status),
            solution.objective_value,
            tables(py, &kept(solution.variables), &self.columns)?,
            tables(py, &kept(solution.constraints), &self.columns)?,
        ))
    }
}

fn set_decls(model: &Model) -> HashMap<Spur, &ir::Set> {
    model.sets.iter().map(|s| (s.name, s)).collect()
}

/// Index column names for a domain: per position, the set name if unique in the domain,
/// else the dummy; or `i0, i1, ...` if that doesn't give distinct names
fn columns(domain: Option<&Domain>, sets: &HashMap<Spur, &ir::Set>) -> Vec<String> {
    let Some(domain) = domain else {
        return vec![];
    };
    let arity: usize = (domain.parts.iter())
        .map(|p| match &p.var {
            DomainPartVar::Tuple(vars) => vars.len(),
            _ => expr_dimen(&p.expr, sets),
        })
        .sum();
    let names = domain.index_names();
    let set_count = |set| names.iter().filter(|n| n.set == Some(set)).count();
    let chosen: Vec<String> = (names.iter())
        .filter_map(|n| match n.set {
            Some(set) if set_count(set) == 1 => Some(set),
            _ => n.dummy,
        })
        .map(|spur| intern_resolve(spur).to_string())
        .collect();
    let mut seen = HashSet::from(["VALUE", "MARGINAL"]);
    if chosen.len() == arity && chosen.iter().all(|n| seen.insert(n)) {
        return chosen;
    }
    (0..arity).map(|i| format!("i{i}")).collect()
}

/// One pyarrow table per name, with index columns then `VALUE` and `MARGINAL`
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
            let names = match columns.get(&name) {
                Some(names) if names.len() == arity => names.clone(),
                _ => (0..arity).map(|i| format!("i{i}")).collect(),
            };
            let mut cols: Vec<(String, ArrayRef)> = (names.into_iter().enumerate())
                .map(|(c, col_name)| (col_name, index_column(&rows, c)))
                .collect();
            let value = Float64Array::from_iter_values(rows.iter().map(|r| r.value));
            let marginal = Float64Array::from_iter_values(rows.iter().map(|r| r.marginal));
            cols.push(("VALUE".into(), Arc::new(value)));
            cols.push(("MARGINAL".into(), Arc::new(marginal)));
            let batch =
                RecordBatch::try_from_iter(cols).map_err(|e| MosoxError::new_err(e.to_string()))?;
            let schema = batch.schema();
            let table = PyTable::try_new(vec![batch], schema)?.into_pyarrow(py)?;
            Ok((intern_resolve(name).to_string(), table))
        })
        .collect()
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
    m.add_class::<PyDat>()?;
    m.add_class::<PyModel>()?;
    m.add_class::<PyMatrix>()?;
    Ok(())
}
