mod bounds;
mod constraints;
mod lookups;
pub mod output;
mod params;

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::gmpl::{Constraint, IndexVal, Objective};
use crate::model::ModelWithData;
use crate::mps::bounds::{Bounds, gen_bounds};
use crate::mps::constraints::{
    Pair, RowType, Term, algebra, domain_to_indexes, get_idx_val_map, recurse,
};
use crate::mps::lookups::Lookups;

//                    var     var_index                 con     con_index       val
type ColsMap =
    IndexMap<(Arc<String>, Arc<Vec<IndexVal>>), IndexMap<(Arc<String>, Arc<Vec<IndexVal>>), f64>>;
//                      con     con_index        type     rhs
type RowsMap = IndexMap<(Arc<String>, Arc<Vec<IndexVal>>), (RowType, Option<f64>)>;
//                      var     var_index       bounds
type BoundsMap = IndexMap<(Arc<String>, Arc<Vec<IndexVal>>), Arc<Bounds>>;

pub struct Compiled {
    cols: ColsMap,
    rows: RowsMap,
    bounds: BoundsMap,
}

struct Con {
    name: Arc<String>,
    idx: Arc<Vec<IndexVal>>,
    row_type: RowType,
    rhs: Option<f64>,
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
    let obj_con = build_objective_constraint(objective, &lookups);
    let mut cons = build_constraints(constraints, &lookups);
    cons.push(obj_con);
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
            .insert((name.clone(), idx.clone()), pair.coeff);
        }
    }

    (cols, rows)
}

fn build_objective_constraint(objective: Objective, lookups: &Lookups) -> Con {
    let pairs = recurse(&objective.expr, lookups, &HashMap::new())
        .into_iter()
        .map(|term| match term {
            Term::Num(_) => panic!("unhandled: objective function has a const in it"),
            Term::Pair(pair) => pair,
        })
        .collect();

    Con {
        name: Arc::new(objective.name),
        // Objective is always "singular": it has no domain
        idx: Arc::new(vec![]),
        row_type: RowType::N,
        rhs: None,
        pairs,
    }
}

fn build_constraints(constraints: Vec<Constraint>, lookups: &Lookups) -> Vec<Con> {
    constraints
        .into_par_iter()
        .flat_map(|Constraint { name, domain, expr }| {
            let name = Arc::new(name);
            domain_to_indexes(domain.as_ref(), lookups, None)
                .into_par_iter()
                .map(|con_index| {
                    let con_index = Arc::new(con_index);
                    let idx_val_map = get_idx_val_map(&domain, &con_index);
                    let lhs = recurse(&expr.lhs, lookups, &idx_val_map);
                    let rhs = recurse(&expr.rhs, lookups, &idx_val_map);
                    let (pairs, rhs_total) = algebra(lhs, rhs);
                    Con {
                        name: name.clone(),
                        idx: con_index,
                        row_type: RowType::from_rel_op(&expr.op),
                        rhs: Some(rhs_total),
                        pairs,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
