mod constraint;
mod lookup;
mod param;
mod set;

use std::sync::Arc;

use indexmap::IndexMap;
use lasso::Spur;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smallvec::SmallVec;

use crate::ir::Index;
use crate::ir::model::{ConstraintOrObjective, ModelWithData};
use crate::ir::op::{Bounds, RowType};
use crate::matrix::constraint::{Pair, algebra, domain_to_indexes, get_index_map, recurse};
use crate::matrix::lookup::Lookups;

pub struct VarWithCoefficients {
    pub bounds: Bounds,
    /// coeffs is a map of (constraint_name, constraint_index) -> coefficient
    pub coeffs: IndexMap<(Spur, Arc<Index>), f64>,
}

/// VarsMap is a map of (var_name, var_index) -> var bounds & coefficients
pub(crate) type VarsMap = IndexMap<(Spur, Arc<Index>), VarWithCoefficients>;
/// ConsMap is an array of (constraint_name, constraint_index, row_type, rhs)
pub(crate) type ConsMap = Vec<(Spur, Arc<Index>, RowType, f64)>;

/// The compiled matrix with vars (cols) and cons (rows).
pub struct Compiled {
    pub vars: VarsMap,
    pub cons: ConsMap,
}

pub fn gen_matrix(model: ModelWithData) -> Compiled {
    let ModelWithData {
        sets,
        pars,
        vars,
        constraints,
    } = model;
    let lookups = Lookups::from_model(sets, vars, pars);
    let cons = build_constraints(constraints, &lookups);
    build_cols_and_rows(cons, &lookups)
}

fn build_cols_and_rows(cons: Vec<SolvedConstraint>, lookups: &Lookups) -> Compiled {
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
        rows.push((name, idx.clone(), row_type, rhs));
        for pair in pairs {
            cols.entry((pair.var, Arc::new(pair.index)))
                .or_insert_with(|| VarWithCoefficients {
                    bounds: *lookups.var_map.get(&pair.var).unwrap(),
                    coeffs: IndexMap::new(),
                })
                .coeffs
                .entry((name, idx.clone()))
                // With big sums, the same Var can appear multiple times, so we must accumulate the
                // coefficients
                .and_modify(|v| *v += pair.coeff)
                .or_insert(pair.coeff);
        }
    }

    Compiled {
        vars: cols,
        cons: rows,
    }
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
