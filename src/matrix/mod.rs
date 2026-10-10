mod constraint;
mod lookup;
mod param;
mod set;
mod validate;

use std::sync::Arc;

use anyhow::Result;
use indexmap::IndexMap;
use lasso::Spur;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use smallvec::SmallVec;

use crate::data::{DataSource, Layered};
use crate::ir::model::{ConstraintOrObjective, Model};
use crate::ir::op::{Bounds, RowType};
use crate::ir::{Index, ObjSense, VarType};
use crate::matrix::constraint::{Pair, algebra, domain_to_indexes, get_index_map, recurse};
use crate::matrix::lookup::Lookups;
use crate::matrix::set::materialize_sets;
use crate::matrix::validate::validate;

pub type ConId = (Spur, Arc<Index>);
pub type VarId = (Spur, Arc<Index>);

pub struct VarWithCoefficients {
    pub var_type: VarType,
    pub bounds: Bounds,
    /// coeffs is a map of (constraint_name, constraint_index) -> coefficient
    pub coeffs: IndexMap<ConId, f64>,
}

/// VarsMap is a map of (var_name, var_index) -> var bounds & coefficients
pub(crate) type VarsMap = IndexMap<VarId, VarWithCoefficients>;
/// ConsMap is an array of (constraint_name, constraint_index, row_type, rhs)
pub(crate) type ConsMap = Vec<(ConId, RowType, f64)>;

/// The compiled matrix with vars (cols) and cons (rows).
pub struct Compiled {
    pub sense: ObjSense,
    pub vars: VarsMap, // cols
    pub cons: ConsMap, // rows
}

pub struct GenOptions {
    /// Drop zero coefficients and vars with no nonzero coefficients (as GLPK does)
    pub prune: bool,
    /// Enforce check statements and set/param declaration constraints on the data
    pub check: bool,
}

pub fn gen_matrix(model: &Model, source: impl DataSource, opts: &GenOptions) -> Result<Compiled> {
    let mut lookups = Lookups::new(model, Layered(source, model.data.clone()))?;
    materialize_sets(&mut lookups);
    if opts.check {
        validate(&model.checks, &lookups)?;
    }
    let cons = build_constraints(&model.constraints, &lookups)?;
    let mut compiled = build_cols_and_rows(model.sense, cons, &lookups)?;
    if opts.prune {
        prune_zeros(&mut compiled.vars);
    }
    Ok(compiled)
}

/// Remove zero coefficients, then any vars left without coefficients.
/// Must run after all coefficients are accumulated, as terms can cancel out.
fn prune_zeros(cols: &mut VarsMap) {
    cols.retain(|_, v| {
        v.coeffs.retain(|_, c| *c != 0.0);
        !v.coeffs.is_empty()
    });
}

fn build_cols_and_rows(
    sense: ObjSense,
    cons: Vec<SolvedConstraint>,
    lookups: &Lookups,
) -> Result<Compiled> {
    let mut rows: ConsMap = vec![];
    let mut cols: VarsMap = IndexMap::new();
    for SolvedConstraint {
        name,
        idx,
        row_type,
        rhs,
        pairs,
    } in cons
    {
        rows.push(((name, idx.clone()), row_type, rhs));
        for pair in pairs {
            cols.entry((pair.var, Arc::new(pair.index)))
                .or_insert_with(|| {
                    let v = lookups.var_map.get(&pair.var).unwrap();
                    VarWithCoefficients {
                        var_type: v.var_type,
                        bounds: v.bounds,
                        coeffs: IndexMap::new(),
                    }
                })
                .coeffs
                .entry((name, idx.clone()))
                // With big sums, the same Var can appear multiple times, so we must accumulate the
                // coefficients
                .and_modify(|v| *v += pair.coeff)
                .or_insert(pair.coeff);
        }
    }

    Ok(Compiled {
        sense,
        vars: cols,
        cons: rows,
    })
}

struct SolvedConstraint {
    name: Spur,
    idx: Arc<Index>,
    row_type: RowType,
    rhs: f64,
    pairs: Vec<Pair>,
}

fn build_constraints(
    constraints: &[ConstraintOrObjective],
    lookups: &Lookups,
) -> Result<Vec<SolvedConstraint>> {
    Ok(constraints
        .par_iter()
        .map(
            |ConstraintOrObjective {
                 name,
                 domain,
                 row_type,
                 lhs,
                 rhs,
             }|
             -> Result<Vec<SolvedConstraint>> {
                let (indexes, parts) = match domain {
                    Some(d) => (
                        domain_to_indexes(d, lookups, &SmallVec::new())?,
                        d.parts.as_slice(),
                    ),
                    None => (vec![vec![].into()], &[][..]),
                };

                indexes
                    .into_par_iter()
                    .map(|con_index| -> Result<SolvedConstraint> {
                        let con_index = Arc::new(con_index);
                        let idx_val_map = get_index_map(parts, &con_index)?;
                        let lhs = recurse(lhs, lookups, &idx_val_map)?;
                        let rhs = recurse(rhs, lookups, &idx_val_map)?;
                        let (pairs, rhs_total) = algebra(lhs, rhs);
                        Ok(SolvedConstraint {
                            name: *name,
                            idx: con_index,
                            row_type: *row_type,
                            rhs: rhs_total,
                            pairs,
                        })
                    })
                    .collect()
            },
        )
        .collect::<Result<Vec<Vec<_>>>>()?
        .into_iter()
        .flatten()
        .collect())
}
