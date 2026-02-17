use crate::ir::{
    BoolOp, Domain, DomainPart, DomainPartVar, Expr, Index, MathOp, RelOp, SetVal, SetValTerminal,
    Subscript, SubscriptShift, interner::intern_resolve,
};
use crate::ir::{LogicExpr, MemberOp, SubscriptPartVar, SubsetOp};
use crate::matrix::lookup::Lookups;
use crate::matrix::param::ParamVal;
use crate::matrix::set::resolve_set_expr;
use anyhow::{Context, Result, bail};
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
pub fn idx_get(map: &IdxValMap, key: Spur) -> Result<SetVal> {
    map.iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
        .copied()
        .with_context(|| {
            let name = intern_resolve(key);
            format!("No idx val at {name}")
        })
}

pub fn idx_val_or_get(map: &IdxValMap, var: SubscriptPartVar) -> Result<SetVal> {
    match var {
        SubscriptPartVar::Var(var) => idx_get(map, var),
        SubscriptPartVar::ValStr(val) => Ok(SetVal::Str(val)),
        SubscriptPartVar::ValInt(val) => Ok(SetVal::Int(val)),
    }
}

// Helper to extend one IdxValMap with another
fn idx_extend(map: &mut IdxValMap, other: &IdxValMap) {
    for (k, v) in other.iter() {
        if !map.iter().any(|(mk, _)| *mk == *k) {
            map.push((*k, *v));
        }
    }
}

pub fn recurse(expr: &Expr, lookups: &Lookups, idx_val_map: &IdxValMap) -> Result<Vec<Term>> {
    match expr {
        Expr::Number(num) => Ok(vec![Term::Num(*num)]),
        Expr::VarSubscripted(var_or_param) => {
            let name = &var_or_param.var;
            // Need to convert from symbolic subscript references
            // to concrete index values
            let index = concrete_index(&var_or_param.subscript, idx_val_map)?;

            if lookups.var_map.contains_key(name) {
                Ok(vec![Term::Pair(Pair {
                    coeff: 1.0,
                    index,
                    var: *name,
                })])
            } else if let Some(param) = lookups.par_map.get(name) {
                match &param.data {
                    ParamVal::Scalar(num) => Ok(vec![Term::Num(*num)]),
                    ParamVal::Arr(arr) => {
                        if let Some(arr_val) = arr.get(&index) {
                            Ok(vec![Term::Num(*arr_val)])
                        } else {
                            match &param.default {
                                Some(expr) => recurse(expr, lookups, idx_val_map),
                                None => bail!("tried to get uninitialized param"),
                            }
                        }
                    }
                    ParamVal::Expr(expr) => recurse(expr, lookups, idx_val_map),
                    ParamVal::None => match &param.default {
                        Some(expr) => recurse(expr, lookups, idx_val_map),
                        None => bail!("tried to get uninitialized param"),
                    },
                    ParamVal::Symbolic => {
                        bail!("symbolic params are not supported in constraint evaluation")
                    }
                }
            } else {
                // Use the current index value (eg y=>2014) as an actual value
                // Mostly (only?) used in domain condition expressions
                Ok(match idx_get(idx_val_map, *name)? {
                    SetVal::Str(val) => vec![Term::Str(val)],
                    SetVal::Int(num) => vec![Term::Num(num as f64)],
                    SetVal::Tuple(_) => bail!("tuple set not allowed in var subscript"),
                })
            }
        }
        Expr::FuncSum(func) => expand_sum(&func.operand, &func.domain, lookups, idx_val_map),
        Expr::FuncMin(func) => {
            let val = eval_func_minmax(&func.domain, true, lookups, idx_val_map)?;
            Ok(vec![Term::Num(val)])
        }
        Expr::FuncMax(func) => {
            let val = eval_func_minmax(&func.domain, false, lookups, idx_val_map)?;
            Ok(vec![Term::Num(val)])
        }
        Expr::Conditional(conditional) => {
            let default;
            let expr: &Expr =
                if check_logic_condition(&conditional.condition, lookups, idx_val_map)? {
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
            let terms = recurse(inner, lookups, idx_val_map)?;
            negate_terms(terms)
        }
        Expr::BinOp { lhs, op, rhs } => {
            let lhs = recurse(lhs, lookups, idx_val_map)?;
            let rhs = recurse(rhs, lookups, idx_val_map)?;

            let lhs_num = resolve_terms_to_num(&lhs)?;
            let rhs_num = resolve_terms_to_num(&rhs)?;

            match op {
                MathOp::Add => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => Ok(vec![Term::Num(lhs + rhs)]),
                    _ => Ok(lhs.into_iter().chain(rhs).collect()),
                },
                MathOp::Sub => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => Ok(vec![Term::Num(lhs - rhs)]),
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
                        Ok(lhs.into_iter().chain(rhs_pairs_neg).collect())
                    }
                    (None, Some(num)) => lhs
                        .into_iter()
                        .map(|p| match p {
                            Term::Str(_) => bail!("Cannot do math on a string term"),
                            Term::Num(inner) => Ok(Term::Num(inner - num)),
                            Term::Pair(pair) => Ok(Term::Pair(Pair {
                                coeff: pair.coeff - num,
                                index: pair.index,
                                var: pair.var,
                            })),
                        })
                        .collect(),
                    _ => bail!("no vars allowed in expr sub"),
                },
                MathOp::Mul => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => Ok(vec![Term::Num(lhs * rhs)]),
                    (Some(num), None) | (None, Some(num)) => {
                        let terms = if lhs_num.is_some() { rhs } else { lhs };
                        terms
                            .into_iter()
                            .map(|p| match p {
                                Term::Str(_) => bail!("Cannot do math on a string term"),
                                Term::Num(inner) => Ok(Term::Num(inner * num)),
                                Term::Pair(pair) => Ok(Term::Pair(Pair {
                                    coeff: pair.coeff * num,
                                    index: pair.index,
                                    var: pair.var,
                                })),
                            })
                            .collect()
                    }
                    _ => bail!("no vars allowed in expr mul"),
                },
                MathOp::Div => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => Ok(vec![Term::Num(lhs / rhs)]),
                    (None, Some(num)) => lhs
                        .into_iter()
                        .map(|p| match p {
                            Term::Str(_) => bail!("Cannot do math on a string term"),
                            Term::Num(inner) => Ok(Term::Num(inner / num)),
                            Term::Pair(pair) => Ok(Term::Pair(Pair {
                                coeff: pair.coeff / num,
                                index: pair.index,
                                var: pair.var,
                            })),
                        })
                        .collect(),
                    _ => bail!("no vars allowed in expr div"),
                },
                MathOp::Pow => match (lhs_num, rhs_num) {
                    (Some(lhs), Some(rhs)) => Ok(vec![Term::Num(lhs.powf(rhs))]),
                    _ => bail!("no vars allowed in expr pow"),
                },
            }
        }
    }
}

pub fn domain_to_indexes(
    domain: &Domain,
    lookups: &Lookups,
    idx_val_map: &IdxValMap,
) -> Result<Vec<Index>> {
    let Domain { parts, condition } = domain;
    let cartesian: Box<dyn Iterator<Item = Vec<SetVal>>> =
        if parts.iter().all(|part| part.subscript.is_empty()) {
            Box::new(
                parts
                    .iter()
                    .map(|part| -> Result<Vec<SetVal>> {
                        let concrete_idx: Index = vec![].into();
                        Ok(lookups
                            .set_map
                            .get(&part.set)
                            .unwrap()
                            .resolve(&concrete_idx, lookups)?
                            .0)
                    })
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
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
                cartesian = Box::new(
                    cartesian
                        .map(|existing| -> Result<Vec<Vec<SetVal>>> {
                            let mut idx_map = get_index_map(parts, &existing)?;
                            idx_extend(&mut idx_map, idx_val_map);
                            let concrete_idx = concrete_index(&part.subscript, &idx_map)?;

                            Ok(lookups
                                .set_map
                                .get(&part.set)
                                .unwrap()
                                .resolve(&concrete_idx, lookups)?
                                .iter()
                                .map(|val| {
                                    let mut new_idx = existing.clone();
                                    new_idx.push(*val);
                                    new_idx
                                })
                                .collect::<Vec<_>>())
                        })
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten(),
                );
            }
            cartesian
        };

    cartesian
        .map(|idx| -> Result<Option<Index>> {
            let idx = Index::from(idx);
            match &condition {
                None => Ok(Some(idx)),
                Some(logic) => {
                    let mut idx_map = get_index_map(parts, &idx)?;
                    idx_extend(&mut idx_map, idx_val_map);
                    if check_logic_condition(logic, lookups, &idx_map)? {
                        Ok(Some(idx))
                    } else {
                        Ok(None)
                    }
                }
            }
        })
        .filter_map(|r| r.transpose())
        .collect::<Result<Vec<Index>>>()
}

pub fn check_logic_condition(
    logic: &LogicExpr,
    lookups: &Lookups,
    idx_val_map: &IdxValMap,
) -> Result<bool> {
    match logic {
        LogicExpr::Comparison { lhs, op, rhs } => {
            let lhs = recurse(lhs, lookups, idx_val_map)?;
            let rhs = recurse(rhs, lookups, idx_val_map)?;

            // no algebra allowed here!
            let lhs_num = resolve_terms_to_term(&lhs)?;
            let rhs_num = resolve_terms_to_term(&rhs)?;

            Ok(match (lhs_num, rhs_num) {
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
                    _ => bail!("unhandled logic expr: {}", logic),
                },
                _ => bail!("vars or mixed terms in domain condition"),
            })
        }
        LogicExpr::Membership { lhs, op, rhs } => {
            let rhs = resolve_set_expr(rhs, idx_val_map, lookups)?;
            Ok(match op {
                MemberOp::In => lhs.iter().all(|elem| rhs.contains(elem)),
                MemberOp::NotIn => lhs.iter().all(|elem| !rhs.contains(elem)),
            })
        }
        LogicExpr::Subset { lhs, op, rhs } => {
            let lhs = resolve_set_expr(lhs, idx_val_map, lookups)?;
            let rhs = resolve_set_expr(rhs, idx_val_map, lookups)?;
            Ok(match op {
                SubsetOp::Within => lhs.iter().all(|elem| rhs.contains(elem)),
                SubsetOp::NotWithin => lhs.iter().all(|elem| !rhs.contains(elem)),
            })
        }
        LogicExpr::BoolOp { lhs, op, rhs } => {
            let lhs = check_logic_condition(lhs, lookups, idx_val_map)?;
            let rhs = check_logic_condition(rhs, lookups, idx_val_map)?;
            Ok(match op {
                BoolOp::And => lhs && rhs,
                BoolOp::Or => lhs || rhs,
            })
        }
    }
}

fn expand_sum(
    operand: &Expr,
    sum_domain: &Domain,
    lookups: &Lookups,
    idx_val_map: &IdxValMap,
) -> Result<Vec<Term>> {
    Ok(domain_to_indexes(sum_domain, lookups, idx_val_map)?
        .into_iter()
        .map(|idx| {
            let mut idx_map = get_index_map(&sum_domain.parts, &idx)?;
            idx_extend(&mut idx_map, idx_val_map);
            recurse(operand, lookups, &idx_map)
        })
        .collect::<Result<Vec<Vec<_>>>>()?
        .into_iter()
        .flatten()
        .collect())
}

fn resolve_terms_to_num(terms: &[Term]) -> Result<Option<f64>> {
    let mut sum = 0.0;
    for t in terms {
        match t {
            Term::Str(_) => bail!("Cannot do math on a string term"),
            Term::Num(num) => sum += num,
            Term::Pair(_) => return Ok(None),
        }
    }
    Ok(Some(sum))
}

fn resolve_terms_to_term(terms: &[Term]) -> Result<Term> {
    if terms.is_empty() {
        bail!("empty domain condition on one side");
    }

    match &terms[0] {
        Term::Str(s) => Ok(Term::Str(*s)),
        Term::Pair(pair) => bail!(
            "Cannot have variables in final domain condition check: {:?}",
            pair
        ),
        Term::Num(_) => {
            let sum = terms.iter().try_fold(0.0, |acc, t| match t {
                Term::Num(num) => Ok(acc + num),
                _ => bail!("mixed term types"),
            })?;
            Ok(Term::Num(sum))
        }
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
pub fn get_index_map(parts: &[DomainPart], idx: &[SetVal]) -> Result<IdxValMap> {
    // idx_val_map stores the current LOCATION
    // as a dict like:
    // { y => 2014, r: "Africa" }
    //
    // This should be improved so that it also knows which set/dimension
    // each entry comes from...
    Ok(parts
        .iter()
        .zip(idx.iter().cloned())
        .map(|(part, idx_val)| -> Result<SmallVec<[(Spur, SetVal); 4]>> {
            Ok(match (&part.var, idx_val) {
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
                _ => bail!(
                    "Mismatched tuple/non-tuple indexes: idx_val: {}, var: {}",
                    idx_val,
                    part.var
                ),
            })
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

fn eval_func_minmax(
    domain: &Domain,
    is_min: bool,
    lookups: &Lookups,
    idx_val_map: &IdxValMap,
) -> Result<f64> {
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
                .map(|k| idx_val_or_get(idx_val_map, k.var))
                .collect::<Result<Vec<_>>>()?
                .into();
            let resolved = lookups
                .set_map
                .get(&set_domain.set)
                .unwrap()
                .resolve(&concrete_set_keys, lookups)?;

            let val = resolved
                .iter()
                .try_fold(None::<u32>, |acc, si| {
                    let num = match si {
                        SetVal::Int(n) => *n,
                        _ => bail!("cannot use func min/max on non-integer index"),
                    };
                    Ok(Some(match acc {
                        Some(a) if is_min => a.min(num),
                        Some(a) => a.max(num),
                        None => num,
                    }))
                })?
                .context("empty set for min/max")?;
            Ok(val as f64)
        }
        None => bail!("no parts in func min/max domain"),
    }
}

fn negate_terms(terms: Vec<Term>) -> Result<Vec<Term>> {
    terms
        .into_iter()
        .map(|t| match t {
            Term::Str(_) => bail!("Cannot unary neg a string term"),
            Term::Num(n) => Ok(Term::Num(-n)),
            Term::Pair(p) => Ok(Term::Pair(Pair {
                coeff: -p.coeff,
                var: p.var,
                index: p.index,
            })),
        })
        .collect()
}

fn concrete_index(susbcript: &Subscript, idx_val_map: &IdxValMap) -> Result<Index> {
    Ok(susbcript
        .iter()
        .map(|i| {
            // First try to look up as a domain variable
            // If not found, check if it's a literal number
            let index_val = idx_val_or_get(idx_val_map, i.var)?;
            match &i.shift {
                Some(shift) => match index_val {
                    SetVal::Str(_) => {
                        bail!("tried to index shift on string index val")
                    }
                    SetVal::Int(index_num) => match shift {
                        SubscriptShift::Plus => Ok(SetVal::Int(index_num + 1)),
                        SubscriptShift::Minus => Ok(SetVal::Int(index_num - 1)),
                    },
                    SetVal::Tuple(_) => {
                        bail!("tuple set not allowed in var subscript")
                    }
                },
                None => Ok(index_val),
            }
        })
        .collect::<Result<Vec<_>>>()?
        .into())
}
