mod bounds;
mod constraints;
mod lookups;
pub mod output;
mod params;

use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use crate::gmpl::{Constraint, IndexVal, Objective};
use crate::model::ModelWithData;
use crate::mps::bounds::{Bounds, gen_bounds};
use crate::mps::constraints::{
    RowType, Term, algebra, domain_to_indexes, get_idx_val_map, recurse,
};
use crate::mps::lookups::Lookups;

//                    var     var_index                 con     con_index       val
type ColsMap =
    IndexMap<(Rc<String>, Rc<Vec<IndexVal>>), IndexMap<(Rc<String>, Rc<Vec<IndexVal>>), f64>>;
//                      con     con_index        type     rhs
type RowsMap = IndexMap<(Rc<String>, Rc<Vec<IndexVal>>), (RowType, Option<f64>)>;
//                      var     var_index       bounds
type BoundsMap = IndexMap<(Rc<String>, Rc<Vec<IndexVal>>), Rc<Bounds>>;

pub struct Compiled {
    cols: ColsMap,
    rows: RowsMap,
    bounds: BoundsMap,
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
    let (cols, rows) = build_constraints(objective, constraints, &lookups);
    let bounds = gen_bounds(&cols, lookups);

    Compiled { cols, rows, bounds }
}

fn build_constraints(
    objective: Objective,
    constraints: Vec<Constraint>,
    lookups: &Lookups,
) -> (ColsMap, RowsMap) {
    let mut rows: RowsMap = IndexMap::new();
    let mut cols: ColsMap = IndexMap::new();

    let obj_name = Rc::new(objective.name);

    // First the objective alone
    // Objective is always "singular": it has no domain
    rows.insert((obj_name.clone(), Rc::new(vec![])), (RowType::N, None));
    let pairs = recurse(&objective.expr, lookups, &HashMap::new());
    for pair in pairs {
        match pair {
            Term::Num(_) => panic!("unhandled: objective function has a const in it"),
            Term::Pair(pair) => {
                cols.entry((
                    Rc::new(pair.var),
                    Rc::new(pair.index.unwrap_or_else(Vec::new)),
                ))
                .or_default()
                .insert((obj_name.clone(), Rc::new(vec![])), pair.coeff);
            }
        }
    }

    // Then all the actual constraints
    let mut constraint_times: Vec<(Rc<String>, Duration)> = Vec::new();
    for constraint in constraints {
        let tc = Instant::now();

        let Constraint { name, domain, expr } = constraint;
        let name = Rc::new(name);

        let con_indexes = domain_to_indexes(domain.as_ref(), lookups, None);

        for con_index in con_indexes {
            let con_index = Rc::new(con_index);
            let dir = RowType::from_rel_op(&expr.op);
            let idx_val_map = get_idx_val_map(&domain, &con_index);

            let lhs = recurse(&expr.lhs, lookups, &idx_val_map);
            let rhs = recurse(&expr.rhs, lookups, &idx_val_map);

            let (pairs, rhs_total) = algebra(lhs, rhs);

            rows.insert((name.clone(), con_index.clone()), (dir, Some(rhs_total)));

            for pair in pairs {
                cols.entry((
                    Rc::new(pair.var),
                    Rc::new(pair.index.unwrap_or_else(Vec::new)),
                ))
                .or_default()
                .insert((name.clone(), con_index.clone()), pair.coeff);
            }
        }

        constraint_times.push((name, tc.elapsed()));
    }

    // Print timing summary
    constraint_times.sort_by(|a, b| b.1.cmp(&a.1));
    eprintln!("Top constraints by time:");
    for (name, dur) in constraint_times.iter().take(10) {
        eprintln!("  {}: {:?}", name, dur);
    }

    (cols, rows)
}
