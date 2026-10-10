use anyhow::{Context, Result, bail};
use lasso::Spur;
use smallvec::smallvec;

use crate::data::{DatSource, DataSource};
use crate::gmpl::loader;
use crate::ir::{
    Check, Constraint, ConstraintExpr, Domain, Entry, Expr, ObjSense, Objective, Param,
    ParamAssign, ParamData, Set, SetData, Var, op::RowType,
};
use crate::matrix::{Compiled, GenOptions, gen_matrix};

#[derive(Clone, Debug)]
pub struct ConstraintOrObjective {
    pub name: Spur,
    pub domain: Option<Domain>,
    pub row_type: RowType,
    pub lhs: Expr,
    pub rhs: Expr,
}

/// A parsed model: declarations, plus any data given in the model file itself.
/// Can be compiled repeatedly against different data.
pub struct Model {
    pub sense: ObjSense,
    pub sets: Vec<Set>,
    pub params: Vec<Param>,
    pub vars: Vec<Var>,
    pub checks: Vec<Check>,
    pub constraints: Vec<ConstraintOrObjective>,
    /// Data from the model file, used for anything the compile-time source doesn't provide
    pub data: DatSource,
}

impl Model {
    pub fn from_file(path: &str) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("Cannot read file: {path}"))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut objective = None;
        let mut sets = Vec::new();
        let mut params = Vec::new();
        let mut vars = Vec::new();
        let mut checks = Vec::new();
        let mut constraints = Vec::new();
        let mut data = DatSource::default();

        for entry in loader::parse(text)? {
            match entry {
                Entry::Objective(obj) => {
                    if objective.is_some() {
                        bail!("Multiple objectives found");
                    }
                    objective = Some(obj);
                }
                Entry::Set(mut set) => {
                    if let Some(values) = set.inline_data.take() {
                        data.add_set(SetData {
                            name: set.name,
                            index: smallvec![],
                            values,
                        });
                    }
                    sets.push(*set);
                }
                Entry::Param(mut param) => {
                    match param.assign.take() {
                        Some(ParamAssign::Data(body)) => data.add_param(ParamData {
                            name: param.name,
                            default: None,
                            body: Some(body),
                        })?,
                        assign => param.assign = assign,
                    }
                    params.push(param);
                }
                Entry::Var(var) => vars.push(var),
                Entry::Check(check) => checks.push(check),
                Entry::Constraint(constraint) => constraints.push(constraint),
                Entry::DataSet(set_data) => data.add_set(set_data),
                Entry::DataParam(param_data) => data.add_param(param_data)?,
            }
        }

        let objective = objective.context("no objective function")?;
        Ok(Model {
            sense: objective.sense,
            sets,
            params,
            vars,
            checks,
            constraints: prep_constraints(objective, constraints)?,
            data,
        })
    }

    /// Compile against `source`; data in the model file fills anything it doesn't provide.
    pub fn compile(&self, source: impl DataSource, opts: &GenOptions) -> Result<Compiled> {
        gen_matrix(self, source, opts)
    }
}

fn prep_constraints(
    objective: Objective,
    constraints: Vec<Constraint>,
) -> Result<Vec<ConstraintOrObjective>> {
    let mut all: Vec<ConstraintOrObjective> = constraints
        .into_iter()
        .map(|Constraint { name, domain, expr }| {
            let ConstraintExpr { lhs, rhs, op } = expr;
            Ok(ConstraintOrObjective {
                name,
                domain,
                row_type: RowType::from_rel_op(&op)?,
                lhs,
                rhs,
            })
        })
        .collect::<Result<_>>()?;
    let Objective {
        name,
        expr,
        sense: _,
    } = objective;
    all.push(ConstraintOrObjective {
        name,
        domain: None,
        row_type: RowType::Unconstrained,
        lhs: expr,
        rhs: Expr::Number(0.0),
    });
    Ok(all)
}
