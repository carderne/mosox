mod check;
mod constraint;
mod lookup;
mod param;
mod set;

use std::sync::Arc;

use anyhow::Result;
use indexmap::IndexMap;
use lasso::Spur;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smallvec::SmallVec;

use crate::ir::model::{ConstraintOrObjective, ModelWithData};
use crate::ir::op::{Bounds, RowType};
use crate::ir::{Index, ObjSense, VarType};
use crate::matrix::check::check_checks;
use crate::matrix::constraint::{Pair, algebra, domain_to_indexes, get_index_map, recurse};
use crate::matrix::lookup::Lookups;

pub type ConId = (Spur, Arc<Index>);
pub type VarId = (Spur, Arc<Index>);

pub struct VarWithCoefficients {
    pub var_type: VarType,
    pub bounds: Vec<Bounds>,
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

pub fn gen_matrix(model: ModelWithData) -> Result<Compiled> {
    let ModelWithData {
        sense,
        sets,
        pars,
        vars,
        checks,
        constraints,
    } = model;
    let lookups = Lookups::from_model(sets, vars, pars);
    check_checks(checks, &lookups)?;
    let cons = build_constraints(constraints, &lookups);
    build_cols_and_rows(sense, cons, &lookups)
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
                        bounds: v.bounds.clone(),
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
    constraints: Vec<ConstraintOrObjective>,
    lookups: &Lookups,
) -> Vec<SolvedConstraint> {
    constraints
        .into_par_iter()
        .flat_map(
            |ConstraintOrObjective {
                 name,
                 domain,
                 row_type,
                 lhs,
                 rhs,
             }| {
                // let row_type = Arc::new(row_type);

                let (indexes, parts) = domain
                    .map(|d| (domain_to_indexes(&d, lookups, &SmallVec::new()), d.parts))
                    .unwrap_or_else(|| (vec![vec![].into()], vec![]));

                indexes
                    .into_par_iter()
                    .map(|con_index| {
                        let con_index = Arc::new(con_index);
                        let idx_val_map = get_index_map(&parts, &con_index);
                        let lhs = recurse(&lhs, lookups, &idx_val_map);
                        let rhs = recurse(&rhs, lookups, &idx_val_map);
                        let (pairs, rhs_total) = algebra(lhs, rhs);
                        SolvedConstraint {
                            name,
                            idx: con_index,
                            row_type,
                            rhs: rhs_total,
                            pairs,
                        }
                    })
                    .collect::<Vec<_>>()
            },
        )
        .collect()
}
