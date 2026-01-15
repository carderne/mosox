mod bounds;
mod constraints;
mod lookups;
pub mod output;
mod params;

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::gmpl::IndexVal;
use crate::model::ModelWithData;
use crate::mps::bounds::{MpsBounds, gen_bounds};
use crate::mps::constraints::{
    RowType, Term, build_constraint, domain_to_indexes, get_idx_val_map, recurse,
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
    for constraint in &model.constraints {
        dbg!(&constraint.name);
        let con_indexes = domain_to_indexes(&constraint.domain, lookups, &HashMap::new());

        for con_index in con_indexes {
            let dir = RowType::from_rel_op(&constraint.constraint_expr.op);
            let idx_val_map = get_idx_val_map(constraint, &con_index);
            let built = build_constraint(constraint, lookups, &idx_val_map);

            rows.insert(
                (constraint.name.clone(), con_index.clone()),
                (dir, built.rhs),
            );

            for pair in &built.pairs {
                cols.entry((
                    pair.var.clone(),
                    pair.index.clone().unwrap_or_else(Vec::new),
                ))
                .or_default()
                .insert((constraint.name.clone(), con_index.clone()), pair.coeff);
            }
        }
    }

    (cols, rows)
}
