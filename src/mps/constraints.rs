use std::collections::HashMap;
use std::fmt;

use crate::gmpl::atoms::{BoolOp, Domain, IndexShift, RelOp, VarSubscripted};
use crate::gmpl::{Expr, atoms::MathOp};
use crate::gmpl::{IndexVal, LogicExpr};
use crate::mps::lookups::Lookups;
use crate::mps::params::ParamArr;
use itertools::Itertools;

#[derive(Clone, Debug)]
pub struct Pair {
    pub var: String,
    pub index: Option<Vec<IndexVal>>,
    pub coeff: f64,
}

#[derive(Clone, Debug)]
pub enum Term {
    Num(f64),
    Pair(Pair),
}

//                       index   index value
type IdxValMap = HashMap<String, IndexVal>;

pub fn recurse(expr: &Expr, lookups: &Lookups, idx_val_map: &IdxValMap) -> Vec<Term> {
    match expr {
        Expr::Number(num) => vec![Term::Num(*num)],
        Expr::VarSubscripted(var_or_param) => {
            let name = &var_or_param.var;

            let concrete: Option<Vec<IndexVal>> = if let Some(c) = &var_or_param.concrete {
                // Already resolved by sum expansion
                Some(c.clone())
            } else {
                var_or_param.subscript.as_ref().map(|subscript| {
                    subscript
                        .indices
                        .iter()
                        .map(|i| {
                            let index_val = idx_val_map.get(&i.var).unwrap().clone();
                            match &i.shift {
                                Some(shift) => match index_val {
                                    IndexVal::Str(_) => {
                                        panic!("tried to index shift on string index val")
                                    }
                                    IndexVal::Int(index_num) => match shift {
                                        IndexShift::Plus => IndexVal::Int(index_num + 1),
                                        IndexShift::Minus => IndexVal::Int(index_num - 1),
                                    },
                                },
                                None => index_val,
                            }
                        })
                        .collect()
                })
            };

            if lookups.var_map.contains_key(name) {
                vec![Term::Pair(Pair {
                    coeff: 1.0,
                    index: concrete,
                    var: name.clone(),
                })]
            } else if let Some(param) = lookups.par_map.get(name) {
                match &param.data {
                    ParamArr::Scalar(num) => vec![Term::Num(*num)],
                    ParamArr::Arr(arr) => {
                        let arr_idx = concrete.expect("concrete is none");
                        if let Some(arr_val) = arr.get(&arr_idx) {
                            vec![Term::Num(*arr_val)]
                        } else {
                            match &param.default {
                                Some(expr) => recurse(expr, lookups, idx_val_map),
                                None => panic!("tried to get uninitialized param: {}", &name),
                            }
                        }
                    }
                    ParamArr::Expr(expr) => {
                        let res = recurse(expr, lookups, idx_val_map);
                        res
                    }
                    ParamArr::None => match &param.default {
                        Some(expr) => recurse(expr, lookups, idx_val_map),
                        None => panic!("tried to get uninitialized param: {}", name),
                    },
                }
            } else if let Some(index_val) = idx_val_map.get(name) {
                // Use the current index value (eg y=>2014) as an actual value
                // Mostly (only?) used in domain condition expressions
                match index_val {
                    IndexVal::Str(_) => panic!("cannot use a string SetIndex here"),
                    IndexVal::Int(num) => vec![Term::Num(*num as f64)],
                }
            } else {
                panic!(
                    "symbol does not point to a valid var or param. symbol: {} // constraint: {}",
                    &name, &expr,
                );
            }
        }
        Expr::FuncSum(func) => {
            let new_expr = expand_sum(&func.operand, &func.domain, lookups, idx_val_map);
            recurse(&new_expr, lookups, idx_val_map)
        }
        Expr::FuncMin(func) => {
            // FuncMin looks like this:
            // min{y in YEAR} min(y)
            // Assumptions:
            // - always only one dimension
            // - always just getting the min of that set
            match func.domain.parts.first() {
                Some(set_domain) => {
                    let min_val = lookups
                        .set_map
                        .get(&set_domain.set)
                        .unwrap()
                        .iter()
                        .map(|si| match si {
                            IndexVal::Str(_) => panic!("cannot use func min on string index"),
                            IndexVal::Int(num) => num,
                        })
                        .min()
                        .unwrap();
                    vec![Term::Num(*min_val as f64)]
                }
                None => panic!("no parts in funcMin domain"),
            }
        }
        Expr::FuncMax(func) => match func.domain.parts.first() {
            Some(set_domain) => {
                let max_val = lookups
                    .set_map
                    .get(&set_domain.set)
                    .unwrap()
                    .iter()
                    .map(|si| match si {
                        IndexVal::Str(_) => panic!("cannot use func max on string index"),
                        IndexVal::Int(num) => num,
                    })
                    .max()
                    .unwrap();
                vec![Term::Num(*max_val as f64)]
            }
            None => panic!("no parts in func max domain"),
        },
        Expr::Conditional(conditional) => {
            let default;
            let expr: &Expr =
                if check_domain_condition(&conditional.condition, lookups, idx_val_map) {
                    &conditional.then_expr
                } else if let Some(otherwise) = &conditional.else_expr {
                    otherwise
                } else {
                    default = Box::new(Expr::Number(0.0));
                    &default
                };

            recurse(expr, lookups, idx_val_map)
        }
        Expr::UnaryNeg(inner) => {
            let terms = recurse(inner, lookups, idx_val_map);
            terms
                .into_iter()
                .map(|t| match t {
                    Term::Num(n) => Term::Num(-n),
                    Term::Pair(p) => Term::Pair(Pair {
                        coeff: -p.coeff,
                        var: p.var,
                        index: p.index,
                    }),
                })
                .collect()
        }
        Expr::BinOp { lhs, op, rhs } => {
            let lhs = recurse(lhs, lookups, idx_val_map);
            let rhs = recurse(rhs, lookups, idx_val_map);

            let lhs_num = resolve_terms_to_num(&lhs);
            let rhs_num = resolve_terms_to_num(&rhs);

            match op {
                MathOp::Add => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => vec![Term::Num(lhs + rhs)],
                    _ => [lhs, rhs].concat(),
                },
                MathOp::Sub => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => vec![Term::Num(lhs - rhs)],
                    (None, None) => {
                        let rhs_pairs: Vec<Pair> = rhs
                            .into_iter()
                            .filter_map(|p| if let Term::Pair(n) = p { Some(n) } else { None })
                            .collect();

                        let rhs_pairs_neg: Vec<Term> = rhs_pairs
                            .into_iter()
                            .map(|pair| {
                                Term::Pair(Pair {
                                    var: pair.var,
                                    index: pair.index,
                                    coeff: -pair.coeff,
                                })
                            })
                            .collect();
                        [lhs, rhs_pairs_neg].concat()
                    }
                    (None, Some(num)) => lhs
                        .into_iter()
                        .map(|p| match p {
                            Term::Num(inner) => Term::Num(inner - num),
                            Term::Pair(pair) => Term::Pair(Pair {
                                coeff: pair.coeff - num,
                                index: pair.index,
                                var: pair.var,
                            }),
                        })
                        .collect(),
                    _ => panic!("no vars allowed in expr sub"),
                },
                MathOp::Mul => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => vec![Term::Num(lhs * rhs)],
                    (Some(num), None) | (None, Some(num)) => {
                        let terms = if lhs_num.is_some() { rhs } else { lhs };
                        terms
                            .into_iter()
                            .map(|p| match p {
                                Term::Num(inner) => Term::Num(inner * num),
                                Term::Pair(pair) => Term::Pair(Pair {
                                    coeff: pair.coeff * num,
                                    index: pair.index,
                                    var: pair.var,
                                }),
                            })
                            .collect()
                    }
                    _ => panic!("no vars allowed in expr mul"),
                },
                MathOp::Div => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => vec![Term::Num(lhs / rhs)],
                    (None, Some(num)) => lhs
                        .into_iter()
                        .map(|p| match p {
                            Term::Num(inner) => Term::Num(inner / num),
                            Term::Pair(pair) => Term::Pair(Pair {
                                coeff: pair.coeff / num,
                                index: pair.index,
                                var: pair.var,
                            }),
                        })
                        .collect(),
                    _ => panic!("no vars allowed in expr div"),
                },
                MathOp::Pow => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => vec![Term::Num(lhs.powf(rhs))],
                    _ => panic!("no vars allowed in expr pow"),
                },
            }
        }
    }
}

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

pub fn domain_to_indexes(
    domain: Option<&Domain>,
    lookups: &Lookups,
    idx_val_map: Option<&IdxValMap>,
) -> Vec<Vec<IndexVal>> {
    match domain {
        None => vec![vec![]],
        Some(Domain { parts, condition }) => parts
            .iter()
            .map(|p| lookups.set_map.get(&p.set).unwrap().clone())
            .multi_cartesian_product()
            .filter_map(|idx| match &condition {
                None => Some(idx),
                Some(logic) => {
                    let mut idx_map: IdxValMap = parts
                        .iter()
                        .zip(idx.iter())
                        .map(|(part, idx)| (part.var.clone(), idx.clone()))
                        .collect();
                    if let Some(idx_val_map) = idx_val_map {
                        idx_map.extend(idx_val_map.clone());
                    }

                    if check_domain_condition(logic, lookups, &idx_map) {
                        Some(idx)
                    } else {
                        None
                    }
                }
            })
            .collect(),
    }
}

pub fn get_idx_val_map(domain: &Option<Domain>, con_index: &[IndexVal]) -> IdxValMap {
    // idx_val_map stores the current LOCATION
    // as a dict like:
    // { y => 2014, r: "Africa" }
    //
    // This should be improved so that it also knows which set/dimension
    // each entry comes from...

    if let Some(domain) = domain {
        domain
            .parts
            .iter()
            .zip(con_index.iter())
            .map(|(part, idx)| (part.var.clone(), idx.clone()))
            .collect()
    } else {
        HashMap::new()
    }
}

fn check_domain_condition(logic: &LogicExpr, lookups: &Lookups, idx_val_map: &IdxValMap) -> bool {
    match logic {
        LogicExpr::Comparison { lhs, op, rhs } => {
            let lhs = recurse(lhs, lookups, idx_val_map);
            let rhs = recurse(rhs, lookups, idx_val_map);

            // no algebra allowed here!
            let lhs_num = resolve_terms_to_num(&lhs);
            let rhs_num = resolve_terms_to_num(&rhs);

            match (lhs_num, rhs_num) {
                (Some(lhs), Some(rhs)) => match op {
                    RelOp::Eq => lhs == rhs,
                    RelOp::Ne => lhs != rhs,
                    RelOp::Gt => lhs > rhs,
                    RelOp::Ge => lhs >= rhs,
                    RelOp::Lt => lhs < rhs,
                    RelOp::Le => lhs <= rhs,
                    _ => panic!("unhandled logic expr: {}", logic),
                },
                _ => panic!("no vars allowed in domain conditions"),
            }
        }
        LogicExpr::BoolOp { lhs, op, rhs } => {
            let lhs = check_domain_condition(lhs, lookups, idx_val_map);
            let rhs = check_domain_condition(rhs, lookups, idx_val_map);
            match op {
                BoolOp::And => lhs && rhs,
                BoolOp::Or => lhs || rhs,
            }
        }
    }
}

fn expand_sum(
    operand: &Expr,
    sum_domain: &Domain,
    lookups: &Lookups,
    idx_val_map: &IdxValMap,
) -> Expr {
    let sum_indexes = domain_to_indexes(Some(sum_domain), lookups, Some(idx_val_map));

    sum_indexes
        .into_iter()
        .map(|idx_combo| {
            let mut idx_map: IdxValMap = sum_domain
                .parts
                .iter()
                .zip(idx_combo.iter())
                .map(|(part, idx)| (part.var.clone(), idx.clone()))
                .collect();
            idx_map.extend(idx_val_map.clone());

            substitute_vars(operand, &idx_map)
        })
        .reduce(|acc, expr| Expr::BinOp {
            lhs: Box::new(acc),
            op: MathOp::Add,
            rhs: Box::new(expr),
        })
        .unwrap_or(Expr::Number(0.0))
}

fn substitute_vars(expr: &Expr, con_index_vals: &IdxValMap) -> Expr {
    match expr {
        Expr::VarSubscripted(vs) => {
            if let Some(subscript) = &vs.subscript {
                let concrete: Vec<IndexVal> = subscript
                    .indices
                    .iter()
                    .map(|i| match con_index_vals.get(&i.var) {
                        Some(s) => match &i.shift {
                            Some(shift) => match s {
                                IndexVal::Str(_) => {
                                    panic!("tried to index shift on string index val")
                                }
                                IndexVal::Int(index_num) => match shift {
                                    IndexShift::Plus => IndexVal::Int(index_num + 1),
                                    IndexShift::Minus => IndexVal::Int(index_num - 1),
                                },
                            },
                            None => s.clone(),
                        },
                        None => panic!("unbound variable: {}", i.var),
                    })
                    .collect();

                return Expr::VarSubscripted(VarSubscripted {
                    var: vs.var.clone(),
                    subscript: None,
                    concrete: Some(concrete),
                });
            }
            Expr::VarSubscripted(vs.clone())
        }
        Expr::BinOp { lhs, op, rhs } => Expr::BinOp {
            lhs: Box::new(substitute_vars(lhs, con_index_vals)),
            op: *op,
            rhs: Box::new(substitute_vars(rhs, con_index_vals)),
        },
        Expr::Number(n) => Expr::Number(*n),
        Expr::UnaryNeg(inner) => Expr::UnaryNeg(Box::new(substitute_vars(inner, con_index_vals))),
        _ => panic!("expr not supported in substition: {}", &expr),
    }
}

fn resolve_terms_to_num(terms: &[Term]) -> Option<f64> {
    terms.iter().try_fold(0.0, |acc, t| match t {
        Term::Num(num) => Some(acc + num),
        Term::Pair(_) => None,
    })
}

pub fn algebra(lhs: Vec<Term>, rhs: Vec<Term>) -> (Vec<Pair>, f64) {
    let lhs_nums: Vec<f64> = lhs
        .iter()
        .filter_map(|p| if let Term::Num(n) = p { Some(*n) } else { None })
        .collect();
    let rhs_nums: Vec<f64> = rhs
        .iter()
        .filter_map(|p| if let Term::Num(n) = p { Some(*n) } else { None })
        .collect();

    let lhs_pairs: Vec<Pair> = lhs
        .into_iter()
        .filter_map(|p| if let Term::Pair(n) = p { Some(n) } else { None })
        .collect();
    let rhs_pairs: Vec<Pair> = rhs
        .into_iter()
        .filter_map(|p| if let Term::Pair(n) = p { Some(n) } else { None })
        .collect();

    let rhs_pairs_neg: Vec<Pair> = rhs_pairs
        .into_iter()
        .map(|pair| Pair {
            var: pair.var,
            index: pair.index,
            coeff: -pair.coeff,
        })
        .collect();

    let lhs_nums_neg: Vec<f64> = lhs_nums.into_iter().map(|n| -n).collect();

    let rhs_total: f64 = [rhs_nums, lhs_nums_neg].into_iter().flatten().sum();
    let pairs = [lhs_pairs, rhs_pairs_neg].concat();
    (pairs, rhs_total)
}
