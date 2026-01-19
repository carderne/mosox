mod bounds;
mod constraints;
mod lookups;
pub mod output;
mod params;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use crate::gmpl::IndexVal;
use crate::model::ModelWithData;
use crate::mps::bounds::{MpsBounds, gen_bounds};
use crate::mps::constraints::{
    RowType, Term, algebra, domain_to_indexes, get_idx_val_map, recurse,
};
use crate::mps::lookups::Lookups;

//                    var     var_index                 con     con_index       val
type Cols = IndexMap<(String, Vec<IndexVal>), IndexMap<(String, Vec<IndexVal>), f64>>;
//                      con     con_index        type     rhs
type Rows = IndexMap<(String, Vec<IndexVal>), (RowType, Option<f64>)>;
//                      var     var_index       bounds
type Bounds = IndexMap<(String, Vec<IndexVal>), MpsBounds>;

pub struct Compiled {
    cols: Cols,
    rows: Rows,
    bounds: Bounds,
}

pub fn compile_mps(model: ModelWithData) -> Compiled {
    let lookups = Lookups::from_model(&model);
    let (cols, rows) = build_constraints(&model, &lookups);
    let bounds = gen_bounds(&cols, &lookups);
    Compiled { cols, rows, bounds }
}

fn build_constraints(model: &ModelWithData, lookups: &Lookups) -> (Cols, Rows) {
    let mut rows: Rows = IndexMap::new();
    let mut cols: Cols = IndexMap::new();

    // First the objective alone
    // Objective is always "singular": it has no domain
    rows.insert((model.objective.name.clone(), vec![]), (RowType::N, None));
    let pairs = recurse(model.objective.expr.clone(), lookups, &HashMap::new());
    for pair in &pairs {
        match pair {
            Term::Num(_) => panic!("unhandled: objective function has a const in it"),
            Term::Pair(pair) => {
                cols.entry((
                    pair.var.clone(),
                    pair.index.clone().unwrap_or_else(Vec::new),
                ))
                .or_default()
                .insert((model.objective.name.clone(), vec![]), pair.coeff);
            }
        }
    }

    // Then all the actual constraints
    let mut constraint_times: Vec<(&str, Duration)> = Vec::new();

    for constraint in &model.constraints {
        let tc = Instant::now();

        let con_indexes = domain_to_indexes(&constraint.domain, lookups, &HashMap::new());

        for con_index in con_indexes {
            let dir = RowType::from_rel_op(&constraint.constraint_expr.op);
            let idx_val_map = get_idx_val_map(constraint, &con_index);

            let lhs = recurse(
                constraint.constraint_expr.lhs.clone(),
                lookups,
                &idx_val_map,
            );
            let rhs = recurse(
                constraint.constraint_expr.rhs.clone(),
                lookups,
                &idx_val_map,
            );

            let (pairs, rhs_total) = algebra(lhs, rhs);

            rows.insert(
                (constraint.name.clone(), con_index.clone()),
                (dir, Some(rhs_total)),
            );

            for pair in &pairs {
                cols.entry((
                    pair.var.clone(),
                    pair.index.clone().unwrap_or_else(Vec::new),
                ))
                .or_default()
                .insert((constraint.name.clone(), con_index.clone()), pair.coeff);
            }
        }

        constraint_times.push((&constraint.name, tc.elapsed()));
    }

    // Print timing summary
    constraint_times.sort_by(|a, b| b.1.cmp(&a.1));
    eprintln!("Top constraints by time:");
    for (name, dur) in constraint_times.iter().take(10) {
        eprintln!("  {}: {:?}", name, dur);
    }

    (cols, rows)
}
