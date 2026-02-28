use crate::ir::interner::intern_resolve;
use crate::ir::{
    BoolOp, Domain, DomainPart, DomainPartVar, Expr, Index, MathOp, ParamVal, RelOp, SetVal,
    SetValTerminal, Subscript,
};
use crate::ir::{LogicExpr, MemberOp, SetAtom, SetExpr, SubsetOp};
use crate::matrix::lookup::Lookups;
use crate::matrix::param::{Param, ParamValEnum};
use crate::matrix::set::{concrete_index, idx_get, resolve_set_expr};
use anyhow::{Context, Result, bail};
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
    Pair(Box<Pair>),
    // This is a special case only used in domain conditions
    // to eg check two domain indexes are the same
    // Also possible when a symbolic param is present
    Str(Spur),
}

impl From<&ParamVal> for Term {
    fn from(inner: &ParamVal) -> Self {
        match inner {
            ParamVal::Str(val) => Term::Str(*val),
            ParamVal::Num(val) => Term::Num(*val),
        }
    }
}

//                       index   index value
pub type IdxValMap = SmallVec<[(Spur, SetVal); 8]>;

// Helper to extend one IdxValMap with another
fn idx_extend(map: &mut IdxValMap, other: &IdxValMap) {
    for (k, v) in other.iter() {
        if !map.iter().any(|(mk, _)| *mk == *k) {
            map.push((*k, v.clone()));
        }
    }
}

pub fn resolve_param(
    param: &Param,
    index: &Index,
    idx_val_map: &IdxValMap,
    lookups: &Lookups,
) -> Result<Vec<Term>> {
    match &param.data {
        ParamValEnum::Arr(arr) => {
            if let Some(arr_val) = arr.get(index) {
                Ok(vec![arr_val.into()])
            } else {
                match &param.default {
                    Some(expr) => recurse(expr, lookups, idx_val_map),
                    None => bail!("tried to get uninitialized param"),
                }
            }
        }
        ParamValEnum::Expr(expr) => recurse(expr, lookups, idx_val_map),
        ParamValEnum::None => match &param.default {
            Some(expr) => recurse(expr, lookups, idx_val_map),
            None => bail!("tried to get uninitialized param"),
        },
    }
}

pub fn resolve_param_to_setval(
    name: &Spur,
    subscript: &Subscript,
    idx_val_map: &IdxValMap,
    lookups: &Lookups,
) -> Result<SetVal> {
    let index = concrete_index(subscript, idx_val_map, lookups)?;
    let param = lookups
        .par_map
        .get(name)
        .with_context(|| format!("range end '{}' not found in params", intern_resolve(*name)))?;
    let terms = resolve_param(param, &index, idx_val_map, lookups)?;
    if terms.len() != 1 {
        bail!("Need to resolve param to single term for setval");
    }
    Ok(match &terms[0] {
        Term::Str(spur) => SetVal::Str(*spur),
        Term::Num(num) => SetVal::Int(*num as u32),
        Term::Pair(_) => bail!("Cannot resolve vars to a param setval"),
    })
}

pub fn recurse(expr: &Expr, lookups: &Lookups, idx_val_map: &IdxValMap) -> Result<Vec<Term>> {
    match expr {
        Expr::Number(num) => Ok(vec![Term::Num(*num)]),
        Expr::VarSubscripted(var_or_param) => {
            let name = &var_or_param.var;
            let index = concrete_index(&var_or_param.subscript, idx_val_map, lookups)?;

            if lookups.var_map.contains_key(name) {
                Ok(vec![Term::Pair(Box::new(Pair {
                    coeff: 1.0,
                    index,
                    var: *name,
                }))])
            } else if let Some(param) = lookups.par_map.get(name) {
                resolve_param(param, &index, idx_val_map, lookups)
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
        Expr::FuncCard(func) => {
            let resolved = resolve_set_expr(&func.expr, idx_val_map, lookups)?;
            Ok(vec![Term::Num(resolved.len() as f64)])
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
                        let rhs_pairs: Vec<Box<Pair>> = rhs
                            .into_iter()
                            .filter_map(|p| if let Term::Pair(n) = p { Some(n) } else { None })
                            .collect();

                        let rhs_pairs_neg = rhs_pairs.into_iter().map(|pair| {
                            Term::Pair(Box::new(Pair {
                                var: pair.var,
                                index: pair.index,
                                coeff: -pair.coeff,
                            }))
                        });
                        Ok(lhs.into_iter().chain(rhs_pairs_neg).collect())
                    }
                    (None, Some(num)) => lhs
                        .into_iter()
                        .map(|p| match p {
                            Term::Str(_) => bail!("Cannot do math on a string term"),
                            Term::Num(inner) => Ok(Term::Num(inner - num)),
                            Term::Pair(pair) => Ok(Term::Pair(Box::new(Pair {
                                coeff: pair.coeff - num,
                                index: pair.index,
                                var: pair.var,
                            }))),
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
                                Term::Pair(pair) => Ok(Term::Pair(Box::new(Pair {
                                    coeff: pair.coeff * num,
                                    index: pair.index,
                                    var: pair.var,
                                }))),
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
                            Term::Pair(pair) => Ok(Term::Pair(Box::new(Pair {
                                coeff: pair.coeff / num,
                                index: pair.index,
                                var: pair.var,
                            }))),
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

    let cartesian: Box<dyn Iterator<Item = Vec<SetVal>>> = {
        // GMPL has a degenerate feature where in a domain expression like
        // { a in A, b in B[a] }
        // a later indexed set can refer to a set value from another one

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

                        Ok(resolve_set_expr(&part.expr, &idx_map, lookups)?
                            .iter()
                            .map(|val| {
                                let mut new_idx = existing.clone();
                                new_idx.push(val.clone());
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
                    _ => bail!("Can only do string == or != in logic expression"),
                },
                _ => bail!("Vars or mixed terms in domain condition"),
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

pub fn resolve_terms_to_num(terms: &[Term]) -> Result<Option<f64>> {
    let mut sum = 0.0;
    for t in terms {
        match t {
            Term::Str(spur) => bail!(
                "AAA Cannot do math on a string term: {}",
                intern_resolve(*spur)
            ),
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
        .filter_map(|p| {
            if let Term::Pair(n) = p {
                Some(*n)
            } else {
                None
            }
        })
        .collect();
    let rhs_pairs: Vec<Pair> = rhs
        .into_iter()
        .filter_map(|p| {
            if let Term::Pair(n) = p {
                Some(*n)
            } else {
                None
            }
        })
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

/// Merge references and values into a mapping
/// idx_val_map stores the current LOCATION
/// as a dict like:
/// { y => 2014, r: "Africa" }
/// I'd prefer this function to accept an Index only, but then I have to clone for the Vec->SmallVec conversion
/// This should be improved so that it also knows which set/dimension
/// each entry comes from...
pub fn get_index_map(parts: &[DomainPart], idx: &[SetVal]) -> Result<IdxValMap> {
    Ok(parts
        .iter()
        .zip(idx.iter().cloned())
        .map(|(part, idx_val)| -> Result<SmallVec<[(Spur, SetVal); 4]>> {
            Ok(match (&part.var, &idx_val) {
                (DomainPartVar::Single(s), val) => smallvec::smallvec![(*s, val.clone())],
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
                _ => bail!("Mismatched tuple/non-tuple indexes"),
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
    let which = if is_min { "min" } else { "max" };
    if domain.parts.len() > 1 {
        bail!("min/max func can only operate on one dimension");
    }
    match domain.parts.first() {
        None => bail!("no parts in func min/max domain"),
        Some(set_domain) => match &set_domain.expr {
            SetExpr::Atom(SetAtom::Ref(set_domain)) => {
                let index = concrete_index(&set_domain.subscript, idx_val_map, lookups)?;
                let resolved = lookups
                    .set_map
                    .get(&set_domain.spur)
                    .unwrap()
                    .resolve(&index, lookups)?;

                let val = resolved
                    .iter()
                    .try_fold(None::<u32>, |acc, si| {
                        let num = match si {
                            SetVal::Int(n) => *n,
                            _ => bail!("Func {which} needs integer index"),
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
            _ => bail!("Func {which} needs simple expression"),
        },
    }
}

fn negate_terms(terms: Vec<Term>) -> Result<Vec<Term>> {
    terms
        .into_iter()
        .map(|t| match t {
            Term::Str(_) => bail!("Cannot unary neg a string term"),
            Term::Num(n) => Ok(Term::Num(-n)),
            Term::Pair(p) => Ok(Term::Pair(Box::new(Pair {
                coeff: -p.coeff,
                var: p.var,
                index: p.index,
            }))),
        })
        .collect()
}
