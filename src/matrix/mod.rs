pub(crate) mod bound;
pub(crate) mod constraints;
mod lookup;
mod param;
mod set;

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use lasso::Spur;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smallvec::SmallVec;

use crate::ir::model::ModelWithData;
use crate::ir::{Constraint, ConstraintExpr, Domain, Expr, Index, Objective, RelOp};
use crate::matrix::bound::{Bounds, gen_bounds};
use crate::matrix::constraints::{Pair, algebra, domain_to_indexes, get_index_map, recurse};
use crate::matrix::lookup::Lookups;

//                    var     var_index                 con     con_index       val
pub(crate) type ColsMap = IndexMap<(Spur, Arc<Index>), IndexMap<(Spur, Arc<Index>), f64>>;
//                      con     con_index        type     rhs
pub(crate) type RowsMap = IndexMap<(Spur, Arc<Index>), (RowType, f64)>;
//                      var     var_index       bounds
pub(crate) type BoundsMap = IndexMap<(Spur, Arc<Index>), Bounds>;

struct ConstraintOrObj {
    name: Spur,
    domain: Option<Domain>,
    row_type: RowType,
    lhs: Expr,
    rhs: Expr,
}

pub struct Compiled {
    pub cols: ColsMap,
    pub rows: RowsMap,
    pub bounds: BoundsMap,
}

struct Con {
    name: Spur,
    idx: Arc<Index>,
    row_type: RowType,
    rhs: f64,
    pairs: Vec<Pair>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum RowType {
    L,
    E,
    G,
    N,
}

impl fmt::Display for RowType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RowType::L => write!(f, "L"),
            RowType::E => write!(f, "E"),
            RowType::G => write!(f, "G"),
            RowType::N => write!(f, "N"),
        }
    }
}

impl RowType {
    pub fn from_rel_op(op: &RelOp) -> Self {
        match op {
            RelOp::Lt => panic!("Less than not supported"),
            RelOp::Le => RowType::L,
            RelOp::Eq => RowType::E,
            RelOp::EqEq => RowType::E,
            RelOp::Ne => panic!("Not equal not supported"),
            RelOp::Ne2 => panic!("Not equal not supported"),
            RelOp::Ge => RowType::G,
            RelOp::Gt => panic!("Greater than not supported"),
        }
    }
}

pub fn compile_mps(model: ModelWithData) -> Compiled {
    let ModelWithData {
        sets,
        pars,
        vars,
        objective,
        constraints,
    } = model;

    let t0 = Instant::now();
    let lookups = Lookups::from_model(sets, vars, pars);
    eprintln!("  lookups: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let all_constraints = prep_constraints(objective, constraints);
    eprintln!("  prep: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let cons = build_constraints(all_constraints, &lookups);
    eprintln!("  cons: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let (cols, rows) = build_cols_and_rows(cons);
    eprintln!("  cols/rows: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let bounds = gen_bounds(&cols, lookups);
    eprintln!("  bounds: {:?}", t0.elapsed());

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
        rows.insert((name, idx.clone()), (row_type, rhs));
        for pair in pairs {
            cols.entry((pair.var, Arc::new(pair.index)))
                .or_default()
                .entry((name, idx.clone()))
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
                        Con {
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
