use std::collections::HashMap;
use std::fmt;

use crate::gmpl::atoms::{
    BoolOp, Domain, DomainPart, DomainPartVar, IndexShift, RelOp, VarSubscripted,
};
use crate::gmpl::{Expr, atoms::MathOp};
use crate::gmpl::{LogicExpr, SetVal, SetValTerminal};
use crate::mps::lookup::Lookups;
use crate::mps::param::ParamVal;
use itertools::Itertools;

#[derive(Clone, Debug)]
pub struct Pair {
    pub var: String,
    pub index: Option<Vec<SetVal>>,
    pub coeff: f64,
}

#[derive(Clone, Debug)]
pub enum Term {
    Num(f64),
    Pair(Pair),
    // This is a special case only used in domain conditions
    // to eg check two domain indexes are the same
    Str(String),
}

//                       index   index value
type IdxValMap = HashMap<String, SetVal>;

pub fn recurse(expr: &Expr, lookups: &Lookups, idx_val_map: &IdxValMap) -> Vec<Term> {
    match expr {
        Expr::Number(num) => vec![Term::Num(*num)],
        Expr::VarSubscripted(var_or_param) => {
            let name = &var_or_param.var;

            let index = if let Some(c) = &var_or_param.index {
                // Already resolved by sum expansion
                Some(c.clone())
            } else {
                var_or_param.subscript.as_ref().map(|subscript| {
                    subscript
                        .indices
                        .iter()
                        .map(|i| {
                            let index_val = idx_val_map.get(&i.var).unwrap();
                            match &i.shift {
                                Some(shift) => match index_val {
                                    SetVal::Str(_) => {
                                        panic!("tried to index shift on string index val")
                                    }
                                    SetVal::Int(index_num) => match shift {
                                        IndexShift::Plus => SetVal::Int(index_num + 1),
                                        IndexShift::Minus => SetVal::Int(index_num - 1),
                                    },
                                    SetVal::Vec(_) => {
                                        panic!("tuple set not allowed in var subscript")
                                    }
                                },
                                None => index_val.clone(),
                            }
                        })
                        .collect()
                })
            };

            if lookups.var_map.contains_key(name) {
                vec![Term::Pair(Pair {
                    coeff: 1.0,
                    index,
                    var: name.clone(),
                })]
            } else if let Some(param) = lookups.par_map.get(name) {
                match &param.data {
                    ParamVal::Scalar(num) => vec![Term::Num(*num)],
                    ParamVal::Arr(arr) => {
                        let arr_idx = index.expect("index is none");
                        if let Some(arr_val) = arr.get(&arr_idx) {
                            vec![Term::Num(*arr_val)]
                        } else {
                            match &param.default {
                                Some(expr) => recurse(expr, lookups, idx_val_map),
                                None => panic!("tried to get uninitialized param: {}", &name),
                            }
                        }
                    }
                    ParamVal::Expr(expr) => {
                        let res = recurse(expr, lookups, idx_val_map);
                        res
                    }
                    ParamVal::None => match &param.default {
                        Some(expr) => recurse(expr, lookups, idx_val_map),
                        None => panic!("tried to get uninitialized param: {}", name),
                    },
                }
            } else if let Some(index_val) = idx_val_map.get(name) {
                // Use the current index value (eg y=>2014) as an actual value
                // Mostly (only?) used in domain condition expressions
                match index_val {
                    SetVal::Str(val) => vec![Term::Str(val.clone())],
                    SetVal::Int(num) => vec![Term::Num(*num as f64)],
                    SetVal::Vec(_) => panic!("tuple set not allowed in var subscript"),
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
            let val = eval_func_minmax(&func.domain, true, lookups, idx_val_map);
            vec![Term::Num(val)]
        }
        Expr::FuncMax(func) => {
            let val = eval_func_minmax(&func.domain, false, lookups, idx_val_map);
            vec![Term::Num(val)]
        }
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
                    Term::Str(_) => panic!("Cannot unary neg a string term"),
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
                            Term::Str(_) => panic!("Cannot do math on a string term"),
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
                                Term::Str(_) => panic!("Cannot do math on a string term"),
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
                            Term::Str(_) => panic!("Cannot do math on a string term"),
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
    domain: &Domain,
    lookups: &Lookups,
    idx_val_map: Option<&IdxValMap>,
) -> Vec<Vec<SetVal>> {
    let Domain { parts, condition } = domain;
    parts
        .iter()
        .map(|p| {
            let concrete_set_keys: Vec<_> = idx_val_map.map_or(vec![], |i| {
                p.idx
                    .iter()
                    .map(|k| i.get(&k.var).unwrap().clone())
                    .collect()
            });
            lookups
                .set_map
                .get(&p.set)
                .unwrap()
                .resolve(&concrete_set_keys, lookups)
        })
        .multi_cartesian_product()
        .filter_map(|idx| match &condition {
            None => Some(idx),
            Some(logic) => {
                let mut idx_map = index_map_from_parts(parts, &idx);
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
        .collect()
}

pub fn get_idx_val_map(domain: &Option<Domain>, con_index: &[SetVal]) -> IdxValMap {
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
            .zip(con_index.iter().cloned())
            .map(|(part, idx_val)| match &part.var {
                DomainPartVar::Single(val) => (val.clone(), idx_val),
                DomainPartVar::Tuple(_) => panic!("tuple domain part not valid in constraint def"),
            })
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
            let lhs_num = resolve_terms_to_term(&lhs);
            let rhs_num = resolve_terms_to_term(&rhs);

            match (lhs_num, rhs_num) {
                (Term::Num(lhs), Term::Num(rhs)) => match op {
                    RelOp::Eq => lhs == rhs,
                    RelOp::Ne => lhs != rhs,
                    RelOp::Gt => lhs > rhs,
                    RelOp::Ge => lhs >= rhs,
                    RelOp::Lt => lhs < rhs,
                    RelOp::Le => lhs <= rhs,
                    _ => panic!("unhandled logic expr: {}", logic),
                },
                (Term::Str(lhs), Term::Str(rhs)) => match op {
                    RelOp::Eq => lhs == rhs,
                    RelOp::Ne => lhs != rhs,
                    _ => panic!("unhandled logic expr: {}", logic),
                },
                _ => panic!("vars or mixed terms in domain condition"),
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
    domain_to_indexes(sum_domain, lookups, Some(idx_val_map))
        .into_iter()
        .map(|idx| {
            let mut idx_map = index_map_from_parts(&sum_domain.parts, &idx);
            idx_map.extend(idx_val_map.clone());

            substitute_vars(operand, lookups, &idx_map)
        })
        .reduce(|acc, expr| Expr::BinOp {
            lhs: Box::new(acc),
            op: MathOp::Add,
            rhs: Box::new(expr),
        })
        .unwrap_or(Expr::Number(0.0))
}

fn substitute_vars(expr: &Expr, lookups: &Lookups, idx_val_map: &IdxValMap) -> Expr {
    match expr {
        Expr::VarSubscripted(vs) => {
            if let Some(subscript) = &vs.subscript {
                let concrete: Vec<SetVal> = subscript
                    .indices
                    .iter()
                    .map(|i| match idx_val_map.get(&i.var) {
                        Some(s) => match &i.shift {
                            Some(shift) => match s {
                                SetVal::Str(_) => {
                                    panic!("tried to index shift on string index val")
                                }
                                SetVal::Int(index_num) => match shift {
                                    IndexShift::Plus => SetVal::Int(index_num + 1),
                                    IndexShift::Minus => SetVal::Int(index_num - 1),
                                },
                                SetVal::Vec(_) => panic!("tried to index shift on tuple index"),
                            },
                            None => s.clone(),
                        },
                        None => panic!("unbound variable: {}", i.var),
                    })
                    .collect();

                return Expr::VarSubscripted(VarSubscripted {
                    var: vs.var.clone(),
                    subscript: None,
                    index: Some(concrete),
                });
            }
            Expr::VarSubscripted(vs.clone())
        }
        Expr::BinOp { lhs, op, rhs } => Expr::BinOp {
            lhs: Box::new(substitute_vars(lhs, lookups, idx_val_map)),
            op: *op,
            rhs: Box::new(substitute_vars(rhs, lookups, idx_val_map)),
        },
        Expr::Number(n) => Expr::Number(*n),
        Expr::UnaryNeg(inner) => {
            // TODO need to be negated?
            Expr::UnaryNeg(Box::new(substitute_vars(inner, lookups, idx_val_map)))
        }
        Expr::FuncSum(func) => expand_sum(&func.operand, &func.domain, lookups, idx_val_map),
        Expr::FuncMin(func) => {
            let val = eval_func_minmax(&func.domain, true, lookups, idx_val_map);
            Expr::Number(val)
        }
        Expr::FuncMax(func) => {
            let val = eval_func_minmax(&func.domain, false, lookups, idx_val_map);
            Expr::Number(val)
        }
        _ => panic!("expr not supported in substition: {}", &expr),
    }
}

fn resolve_terms_to_num(terms: &[Term]) -> Option<f64> {
    terms.iter().try_fold(0.0, |acc, t| match t {
        Term::Str(_) => panic!("Cannot do math on a string term"),
        Term::Num(num) => Some(acc + num),
        Term::Pair(_) => None,
    })
}

fn resolve_terms_to_term(terms: &[Term]) -> Term {
    if terms.is_empty() {
        panic!("empty domain condition on one side");
    }

    match &terms[0] {
        Term::Str(s) => Term::Str(s.clone()),
        Term::Pair(_) => panic!("Cannot have variables in final domain condition check"),
        Term::Num(_) => Term::Num(terms.iter().fold(0.0, |acc, t| match t {
            Term::Str(_) => panic!("mixed term types"),
            Term::Num(num) => acc + num,
            Term::Pair(_) => panic!("mixed term types"),
        })),
    }
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

fn index_map_from_parts(parts: &[DomainPart], idx: &[SetVal]) -> IdxValMap {
    parts
        .iter()
        .zip(idx.iter().cloned())
        .flat_map(|(part, idx_val)| -> Vec<(String, SetVal)> {
            match (&part.var, idx_val) {
                (DomainPartVar::Single(s), val) => vec![(s.clone(), val)],
                (DomainPartVar::Tuple(vars), SetVal::Vec(vals)) => vars
                    .iter()
                    .zip(vals.iter())
                    .map(|(v, sv)| {
                        let set_val = match sv {
                            SetValTerminal::Str(s) => SetVal::Str(s.clone()),
                            SetValTerminal::Int(n) => SetVal::Int(*n),
                        };
                        (v.clone(), set_val)
                    })
                    .collect(),
                _ => panic!("mismatched tuple/non-tuple indexes"),
            }
        })
        .collect()
}

fn eval_func_minmax(
    domain: &Domain,
    is_min: bool,
    lookups: &Lookups,
    idx_val_map: &IdxValMap,
) -> f64 {
    // FuncMin looks like this:
    // min{y in YEAR} min(y)
    // Assumptions:
    // - always only one dimension
    // - always just getting the min of that set

    // Only support min/maxing a single dimension
    match domain.parts.first() {
        Some(set_domain) => {
            let concrete_set_keys: Vec<_> = set_domain
                .idx
                .iter()
                .map(|k| idx_val_map.get(&k.var).unwrap().clone())
                .collect();
            let resolved = lookups
                .set_map
                .get(&set_domain.set)
                .unwrap()
                .resolve(&concrete_set_keys, lookups);
            let iter = resolved.iter().map(|si| match si {
                SetVal::Str(_) => panic!("cannot use func min/max on string index"),
                SetVal::Int(num) => *num,
                SetVal::Vec(_) => panic!("cannot use func min/max with tuple index"),
            });
            let val = if is_min { iter.min() } else { iter.max() }.unwrap();
            val as f64
        }
        None => panic!("no parts in func min/max domain"),
    }
}
