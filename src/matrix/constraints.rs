use crate::ir::LogicExpr;
use crate::ir::{
    BoolOp, Domain, DomainPart, DomainPartVar, Expr, Index, MathOp, RelOp, SetVal, SetValTerminal,
    Subscript, SubscriptShift,
};
use crate::matrix::lookup::Lookups;
use crate::matrix::param::ParamVal;
use itertools::Itertools;
use lasso::Spur;
use smallvec::SmallVec;

#[derive(Clone, Debug)]
pub struct Pair {
    pub var: Spur,
    pub index: Index,
    pub coeff: f64,
}

#[derive(Clone, Debug)]
pub enum Term {
    Num(f64),
    Pair(Pair),
    // This is a special case only used in domain conditions
    // to eg check two domain indexes are the same
    Str(Spur),
}

//                       index   index value
pub type IdxValMap = SmallVec<[(Spur, SetVal); 8]>;

// Helper function to get a value from IdxValMap
pub fn idx_get(map: &IdxValMap, key: Spur) -> Option<&SetVal> {
    map.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

// Helper to extend one IdxValMap with another
fn idx_extend(map: &mut IdxValMap, other: &IdxValMap) {
    for (k, v) in other.iter() {
        if !map.iter().any(|(mk, _)| *mk == *k) {
            map.push((*k, *v));
        }
    }
}

pub fn recurse(expr: &Expr, lookups: &Lookups, idx_val_map: &IdxValMap) -> Vec<Term> {
    match expr {
        Expr::Number(num) => vec![Term::Num(*num)],
        Expr::VarSubscripted(var_or_param) => {
            let name = &var_or_param.var;
            // Need to convert from symbolic subscript references
            // to concrete index values
            let index = concrete_index(&var_or_param.subscript, idx_val_map);

            if lookups.var_map.contains_key(name) {
                vec![Term::Pair(Pair {
                    coeff: 1.0,
                    index,
                    var: *name,
                })]
            } else if let Some(param) = lookups.par_map.get(name) {
                match &param.data {
                    ParamVal::Scalar(num) => vec![Term::Num(*num)],
                    ParamVal::Arr(arr) => {
                        if let Some(arr_val) = arr.get(&index) {
                            vec![Term::Num(*arr_val)]
                        } else {
                            match &param.default {
                                Some(expr) => recurse(expr, lookups, idx_val_map),
                                None => panic!("tried to get uninitialized param"),
                            }
                        }
                    }
                    ParamVal::Expr(expr) => {
                        let res = recurse(expr, lookups, idx_val_map);
                        res
                    }
                    ParamVal::None => match &param.default {
                        Some(expr) => recurse(expr, lookups, idx_val_map),
                        None => panic!("tried to get uninitialized param"),
                    },
                }
            } else if let Some(index_val) = idx_get(idx_val_map, *name) {
                // Use the current index value (eg y=>2014) as an actual value
                // Mostly (only?) used in domain condition expressions
                match index_val {
                    SetVal::Str(val) => vec![Term::Str(*val)],
                    SetVal::Int(num) => vec![Term::Num(*num as f64)],
                    SetVal::Tuple(_) => panic!("tuple set not allowed in var subscript"),
                }
            } else {
                panic!("symbol does not point to a valid var or param",);
            }
        }
        Expr::FuncSum(func) => expand_sum(&func.operand, &func.domain, lookups, idx_val_map),
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
            negate_terms(terms)
        }
        Expr::BinOp { lhs, op, rhs } => {
            let lhs = recurse(lhs, lookups, idx_val_map);
            let rhs = recurse(rhs, lookups, idx_val_map);

            let lhs_num = resolve_terms_to_num(&lhs);
            let rhs_num = resolve_terms_to_num(&rhs);

            match op {
                MathOp::Add => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => vec![Term::Num(lhs + rhs)],
                    _ => lhs.into_iter().chain(rhs).collect(),
                },
                MathOp::Sub => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => vec![Term::Num(lhs - rhs)],
                    (None, None) => {
                        let rhs_pairs: Vec<Pair> = rhs
                            .into_iter()
                            .filter_map(|p| if let Term::Pair(n) = p { Some(n) } else { None })
                            .collect();

                        let rhs_pairs_neg = rhs_pairs.into_iter().map(|pair| {
                            Term::Pair(Pair {
                                var: pair.var,
                                index: pair.index,
                                coeff: -pair.coeff,
                            })
                        });
                        lhs.into_iter().chain(rhs_pairs_neg).collect()
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

pub fn domain_to_indexes(
    domain: &Domain,
    lookups: &Lookups,
    idx_val_map: &IdxValMap,
) -> Vec<Index> {
    let Domain { parts, condition } = domain;
    let cartesian: Box<dyn Iterator<Item = Vec<SetVal>>> =
        if parts.iter().all(|part| part.subscript.is_empty()) {
            Box::new(
                parts
                    .iter()
                    .map(|part| {
                        let concrete_idx: Index = vec![].into();
                        lookups
                            .set_map
                            .get(&part.set)
                            .unwrap()
                            .resolve(&concrete_idx, lookups)
                            .0
                    })
                    .multi_cartesian_product(),
            )
        } else {
            // GMPL has a degenerate feature where in a domain expression like
            // { a in A, b in B[a] }
            // a later indexed set can refer to a set value from another one
            // Plausibly this could go twice like
            // { a in A, b in B[a], c in C[b] }
            // but I'm hoping not to support that

            // The Box dyn is just to keep the variable as an iterator so we can
            // reassign to it but not have to collect it until we're done iterating
            let mut cartesian: Box<dyn Iterator<Item = Vec<SetVal>>> =
                Box::new(vec![vec![]].into_iter());
            for part in parts {
                cartesian = Box::new(cartesian.flat_map(|existing| {
                    let mut idx_map = get_index_map(parts, &existing);
                    idx_extend(&mut idx_map, idx_val_map);
                    let concrete_idx = concrete_index(&part.subscript, &idx_map);

                    lookups
                        .set_map
                        .get(&part.set)
                        .unwrap()
                        .resolve(&concrete_idx, lookups)
                        .iter()
                        .map(|val| {
                            let mut new_idx = existing.clone();
                            new_idx.push(*val);
                            new_idx
                        })
                        .collect::<Vec<_>>()
                }));
            }
            cartesian
        };

    cartesian
        .filter_map(|idx| {
            let idx = Index::from(idx);
            match &condition {
                None => Some(idx),
                Some(logic) => {
                    let mut idx_map = get_index_map(parts, &idx);
                    idx_extend(&mut idx_map, idx_val_map);
                    if check_domain_condition(logic, lookups, &idx_map) {
                        Some(idx)
                    } else {
                        None
                    }
                }
            }
        })
        .collect::<Vec<Index>>()
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
                    RelOp::EqEq => lhs == rhs,
                    RelOp::Ne => lhs != rhs,
                    RelOp::Ne2 => lhs != rhs,
                    RelOp::Gt => lhs > rhs,
                    RelOp::Ge => lhs >= rhs,
                    RelOp::Lt => lhs < rhs,
                    RelOp::Le => lhs <= rhs,
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
) -> Vec<Term> {
    domain_to_indexes(sum_domain, lookups, idx_val_map)
        .into_iter()
        .flat_map(|idx| {
            let mut idx_map = get_index_map(&sum_domain.parts, &idx);
            idx_extend(&mut idx_map, idx_val_map);
            recurse(operand, lookups, &idx_map)
        })
        .collect()
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
        Term::Str(s) => Term::Str(*s),
        Term::Pair(pair) => panic!(
            "Cannot have variables in final domain condition check: {:?}",
            pair
        ),
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
    let pairs = lhs_pairs.into_iter().chain(rhs_pairs_neg).collect();
    (pairs, rhs_total)
}

// I'd prefer this function to accept an Index only, but then I have to clone for the Vec->SmallVec
// conversion
pub fn get_index_map(parts: &[DomainPart], idx: &[SetVal]) -> IdxValMap {
    // idx_val_map stores the current LOCATION
    // as a dict like:
    // { y => 2014, r: "Africa" }
    //
    // This should be improved so that it also knows which set/dimension
    // each entry comes from...
    parts
        .iter()
        .zip(idx.iter().cloned())
        .flat_map(|(part, idx_val)| -> SmallVec<[(Spur, SetVal); 4]> {
            match (&part.var, idx_val) {
                (DomainPartVar::Single(s), val) => smallvec::smallvec![(*s, val)],
                (DomainPartVar::Tuple(vars), SetVal::Tuple(vals)) => vars
                    .iter()
                    .zip(vals.iter())
                    .map(|(v, sv)| {
                        let set_val = match sv {
                            SetValTerminal::Str(s) => SetVal::Str(*s),
                            SetValTerminal::Int(n) => SetVal::Int(*n),
                        };
                        (*v, set_val)
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
            let concrete_set_keys: Index = set_domain
                .subscript
                .iter()
                .map(|k| *idx_get(idx_val_map, k.var).unwrap())
                .collect::<Vec<_>>()
                .into();
            let resolved = lookups
                .set_map
                .get(&set_domain.set)
                .unwrap()
                .resolve(&concrete_set_keys, lookups);
            let iter = resolved.iter().map(|si| match si {
                SetVal::Str(_) => panic!("cannot use func min/max on string index"),
                SetVal::Int(num) => *num,
                SetVal::Tuple(_) => panic!("cannot use func min/max with tuple index"),
            });
            let val = if is_min { iter.min() } else { iter.max() }.unwrap();
            val as f64
        }
        None => panic!("no parts in func min/max domain"),
    }
}

fn negate_terms(terms: Vec<Term>) -> Vec<Term> {
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

fn concrete_index(susbcript: &Subscript, idx_val_map: &IdxValMap) -> Index {
    susbcript
        .iter()
        .map(|i| {
            let index_val =
                idx_get(idx_val_map, i.var).unwrap_or_else(|| panic!("No idx val available at",));
            match &i.shift {
                Some(shift) => match index_val {
                    SetVal::Str(_) => {
                        panic!("tried to index shift on string index val")
                    }
                    SetVal::Int(index_num) => match shift {
                        SubscriptShift::Plus => SetVal::Int(index_num + 1),
                        SubscriptShift::Minus => SetVal::Int(index_num - 1),
                    },
                    SetVal::Tuple(_) => {
                        panic!("tuple set not allowed in var subscript")
                    }
                },
                None => *index_val,
            }
        })
        .collect::<Vec<_>>()
        .into()
}
