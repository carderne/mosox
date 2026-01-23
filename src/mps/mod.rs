mod bound;
mod constraints;
mod lookup;
pub mod output;
mod param;
mod set;

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::gmpl::{Constraint, ConstraintExpr, Domain, Expr, Objective, SetVal};
use crate::model::ModelWithData;
use crate::mps::bound::{Bounds, gen_bounds};
use crate::mps::constraints::{Pair, RowType, algebra, domain_to_indexes, get_index_map, recurse};
use crate::mps::lookup::Lookups;

//                    var     var_index                 con     con_index       val
type ColsMap =
    IndexMap<(Arc<String>, Arc<Vec<SetVal>>), IndexMap<(Arc<String>, Arc<Vec<SetVal>>), f64>>;
//                      con     con_index        type     rhs
type RowsMap = IndexMap<(Arc<String>, Arc<Vec<SetVal>>), (Arc<RowType>, f64)>;
//                      var     var_index       bounds
type BoundsMap = IndexMap<(Arc<String>, Arc<Vec<SetVal>>), Arc<Bounds>>;

struct ConstraintOrObj {
    name: String,
    domain: Option<Domain>,
    row_type: RowType,
    lhs: Expr,
    rhs: Expr,
}

pub struct Compiled {
    cols: ColsMap,
    rows: RowsMap,
    bounds: BoundsMap,
}

struct Con {
    name: Arc<String>,
    idx: Arc<Vec<SetVal>>,
    row_type: Arc<RowType>,
    rhs: f64,
    pairs: Vec<Pair>,
}

pub fn compile_mps(model: ModelWithData) -> Compiled {
    let ModelWithData {
        sets,
        pars,
        vars,
        objective,
        constraints,
    } = model;

    let lookups = Lookups::from_model(sets, vars, pars);
    let all_constraints = prep_constraints(objective, constraints);
    let cons = build_constraints(all_constraints, &lookups);
    let (cols, rows) = build_cols_and_rows(cons);
    let bounds = gen_bounds(&cols, lookups);

    Compiled { cols, rows, bounds }
}

fn build_cols_and_rows(cons: Vec<Con>) -> (ColsMap, RowsMap) {
    let mut rows: RowsMap = IndexMap::new();
    let mut cols: ColsMap = IndexMap::new();
    for Con {
        name,
        idx,
        row_type,
        rhs,
        pairs,
    } in cons
    {
        rows.insert((name.clone(), idx.clone()), (row_type, rhs));
        for pair in pairs {
            cols.entry((
                Arc::new(pair.var),
                Arc::new(pair.index.unwrap_or_else(Vec::new)),
            ))
            .or_default()
            .entry((name.clone(), idx.clone()))
            // With big sums, the same Var can appear multiple times, so we must accumulate the
            // coefficients
            .and_modify(|v| *v += pair.coeff)
            .or_insert(pair.coeff);
        }
    }

    (cols, rows)
}

fn prep_constraints(objective: Objective, constraints: Vec<Constraint>) -> Vec<ConstraintOrObj> {
    let mut all: Vec<ConstraintOrObj> = constraints
        .into_iter()
        .map(|Constraint { name, domain, expr }| {
            let ConstraintExpr { lhs, rhs, op } = expr;
            ConstraintOrObj {
                name,
                domain,
                row_type: RowType::from_rel_op(&op),
                lhs,
                rhs,
            }
        })
        .collect();
    let Objective {
        name,
        expr,
        sense: _,
    } = objective;
    all.push(ConstraintOrObj {
        name,
        domain: None,
        row_type: RowType::N,
        lhs: expr,
        rhs: Expr::Number(0.0),
    });
    all
}

fn build_constraints(constraints: Vec<ConstraintOrObj>, lookups: &Lookups) -> Vec<Con> {
    constraints
        .into_par_iter()
        .flat_map(
            |ConstraintOrObj {
                 name,
                 domain,
                 row_type,
                 lhs,
                 rhs,
             }| {
                let name = Arc::new(name);
                let row_type = Arc::new(row_type);

                let (indexes, parts) = domain
                    .map(|d| (domain_to_indexes(&d, lookups, &HashMap::new()), d.parts))
                    .unwrap_or_else(|| (vec![vec![]], vec![]));

                indexes
                    .into_par_iter()
                    .map(|con_index| {
                        let con_index = Arc::new(con_index);
                        let idx_val_map = get_index_map(&parts, &con_index);
                        let lhs = recurse(&lhs, lookups, &idx_val_map);
                        let rhs = recurse(&rhs, lookups, &idx_val_map);
                        let (pairs, rhs_total) = algebra(lhs, rhs);
                        Con {
                            name: name.clone(),
                            idx: con_index,
                            row_type: row_type.clone(),
                            rhs: rhs_total,
                            pairs,
                        }
                    })
                    .collect::<Vec<_>>()
            },
        )
        .collect()
}
