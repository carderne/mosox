use std::collections::HashSet;
use std::slice;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use smallvec::SmallVec;

use crate::ir::interner::intern_resolve;
use crate::ir::{
    Check, Domain, DomainPartVar, Expr, Index, ParamType, ParamVal, RelOp, SetAtom, SetExpr,
    SetVal, SetValTerminal,
};
use crate::matrix::constraint::{
    IdxValMap, Term, check_logic_condition, compare_terms, domain_to_indexes, get_index_map,
    recurse, resolve_terms_to_term,
};
use crate::matrix::lookup::Lookups;
use crate::matrix::param::{Param, ParamValEnum};
use crate::matrix::set::{SetCont, resolve_set_expr};

const MAX_SHOWN: usize = 20;

type Flat = SmallVec<[SetValTerminal; 8]>;

/// Enforce check statements, set `within`/`dimen`, param types/conditions/`in`,
/// and that set and param data lie within their declared domains.
pub fn validate(checks: &[Check], lookups: &Lookups) -> Result<()> {
    let sets: Vec<&SetCont> = lookups.set_map.values().collect();
    let mut errors: Vec<String> = sets
        .into_par_iter()
        .map(|s| {
            validate_set(s, lookups).with_context(|| {
                let name = intern_resolve(s.decl.name);
                format!("validating set {name} (line {})", s.decl.line_no)
            })
        })
        .chain(lookups.par_map.par_iter().map(|(_, p)| {
            validate_param(p, lookups).with_context(|| {
                let name = intern_resolve(p.decl.name);
                format!("validating param {name} (line {})", p.decl.line_no)
            })
        }))
        .chain(checks.par_iter().map(|c| {
            validate_check(c, lookups)
                .with_context(|| format!("evaluating check (line {})", c.line_no))
        }))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    if errors.is_empty() {
        return Ok(());
    }
    errors.sort();
    let total = errors.len();
    let more = if total > MAX_SHOWN {
        format!("\n  ... and {} more", total - MAX_SHOWN)
    } else {
        String::new()
    };
    errors.truncate(MAX_SHOWN);
    bail!(
        "{total} validation error(s):\n  {}{more}",
        errors.join("\n  ")
    )
}

fn validate_set(set: &SetCont, lookups: &Lookups) -> Result<Vec<String>> {
    let decl = &set.decl;
    let name = intern_resolve(decl.name);
    let domain = Members::new(Some(&decl.domain), lookups)?;
    let fixed_within = (decl.within.iter().all(is_static))
        .then(|| Product::new(&decl.within, &IdxValMap::new(), lookups))
        .transpose()?;

    let mut errors = vec![];
    for (index, vals) in &set.data {
        let at = format!(
            "set {name} (line {}): {name}{}",
            decl.line_no,
            fmt_index(index)
        );
        if !domain.contains(index) {
            errors.push(format!("{at} is outside its domain"));
            continue;
        }
        let owned;
        let within = match &fixed_within {
            Some(within) => within,
            None => {
                let idx_val_map = get_index_map(&decl.domain.parts, index)?;
                owned = Product::new(&decl.within, &idx_val_map, lookups)?;
                &owned
            }
        };
        let dimen = match (decl.dimen, decl.within.is_empty()) {
            (Some(dimen), _) => dimen as usize,
            (None, false) => within.arity(),
            (None, true) => vals.first().map_or(1, arity),
        };
        for val in vals.iter() {
            if arity(val) != dimen {
                errors.push(format!(
                    "{at} element ({val}) has dimension {}, expected {dimen}",
                    arity(val)
                ));
            } else if !decl.within.is_empty() && !within.contains(&flatten(slice::from_ref(val))) {
                errors.push(format!(
                    "{at} element ({val}) is not within its declared superset"
                ));
            }
        }
    }
    Ok(errors)
}

/// A param rule's right-hand side: evaluated once if it doesn't depend on the
/// param's own index, otherwise per element.
enum Rhs<'a, T, E> {
    Fixed(T),
    PerIndex(&'a E),
}

fn validate_param(param: &Param, lookups: &Lookups) -> Result<Vec<String>> {
    let decl = &param.decl;
    let name = intern_resolve(decl.name);
    let at = |index: &Index| {
        format!(
            "param {name} (line {}): {name}{}",
            decl.line_no,
            fmt_index(index)
        )
    };
    let empty = IdxValMap::new();

    // If it evaluates without index values it doesn't depend on them
    let conditions: Vec<(RelOp, Rhs<Term, Expr>)> = decl
        .conditions
        .iter()
        .map(|c| {
            let rhs =
                match recurse(&c.value, lookups, &empty).and_then(|t| resolve_terms_to_term(&t)) {
                    Ok(term) => Rhs::Fixed(term),
                    Err(_) => Rhs::PerIndex(&c.value),
                };
            (c.op, rhs)
        })
        .collect();
    let in_set = decl
        .param_in
        .as_ref()
        .map(|expr| match resolve_set_expr(expr, &empty, lookups) {
            Ok(vals) => Rhs::Fixed(vals.0.into_iter().collect::<HashSet<_>>()),
            Err(_) => Rhs::PerIndex(expr),
        });

    let per_index = conditions
        .iter()
        .any(|(_, rhs)| matches!(rhs, Rhs::PerIndex(_)))
        || matches!(in_set, Some(Rhs::PerIndex(_)));

    // `index` is None for the default value, where index-dependent rules are skipped
    let violation = |index: Option<&Index>, val: ParamVal| -> Result<Option<String>> {
        let num = match val {
            ParamVal::Num(n) => Some(n),
            ParamVal::Str(_) => None,
        };
        let type_err = match (&decl.param_type, num) {
            (ParamType::Symbolic, _) => None,
            (_, None) => Some("is not numeric"),
            (ParamType::Integer, Some(n)) if n.fract() != 0.0 => Some("is not integer"),
            (ParamType::Binary, Some(n)) if n != 0.0 && n != 1.0 => Some("is not binary"),
            _ => None,
        };
        if let Some(err) = type_err {
            return Ok(Some(err.to_string()));
        }

        let idx_val_map = index
            .filter(|_| per_index)
            .map(|index| flat_index_map(decl.domain.as_ref(), index));
        for (op, rhs) in &conditions {
            let rhs = match (rhs, &idx_val_map) {
                (Rhs::Fixed(term), _) => term.clone(),
                (Rhs::PerIndex(expr), Some(map)) => {
                    resolve_terms_to_term(&recurse(expr, lookups, map)?)?
                }
                (Rhs::PerIndex(_), None) => continue,
            };
            if !compare_terms(Term::from(&val), op, rhs.clone())? {
                return Ok(Some(format!("violates {op} {}", fmt_term(&rhs))));
            }
        }

        let elem = match val {
            ParamVal::Str(s) => Some(SetVal::Str(s)),
            ParamVal::Num(n) if n >= 0.0 && n.fract() == 0.0 => Some(SetVal::Int(n as u32)),
            ParamVal::Num(_) => None,
        };
        let in_ok = match (&in_set, &idx_val_map) {
            (None, _) | (Some(Rhs::PerIndex(_)), None) => true,
            (Some(Rhs::Fixed(set)), _) => elem.is_some_and(|e| set.contains(&e)),
            (Some(Rhs::PerIndex(expr)), Some(map)) => {
                let set = resolve_set_expr(expr, map, lookups)?;
                elem.is_some_and(|e| set.contains(&e))
            }
        };
        Ok((!in_ok).then(|| "is not in its `in` set".to_string()))
    };

    let mut errors = vec![];
    if let Some(Expr::Number(n)) = &param.default
        && let Some(err) = violation(None, ParamVal::Num(*n))?
    {
        errors.push(format!(
            "param {name} (line {}): default {n} {err}",
            decl.line_no
        ));
    }
    if let ParamValEnum::Arr(arr) = &param.data {
        let domain = Members::new(decl.domain.as_ref(), lookups)?;
        errors.par_extend(arr.par_iter().filter_map(|(index, val)| {
            if !domain.contains(index) {
                return Some(format!("{} is outside its domain", at(index)));
            }
            let err = match violation(Some(index), *val) {
                Ok(err) => err?,
                Err(e) => format!("could not be checked: {e:#}"),
            };
            Some(format!("{} = {} {err}", at(index), fmt_val(val)))
        }));
    }
    Ok(errors)
}

fn validate_check(check: &Check, lookups: &Lookups) -> Result<Vec<String>> {
    let (indexes, parts) = match &check.domain {
        Some(d) => (
            domain_to_indexes(d, lookups, &IdxValMap::new())?,
            d.parts.as_slice(),
        ),
        None => (vec![Index::new()], &[][..]),
    };
    Ok(indexes
        .into_par_iter()
        .filter_map(|index| {
            let at = format!("check (line {}) failed{}", check.line_no, fmt_index(&index));
            match get_index_map(parts, &index)
                .and_then(|map| check_logic_condition(&check.expr, lookups, &map))
            {
                Ok(true) => None,
                Ok(false) => Some(at),
                Err(e) => Some(format!("{at}: {e:#}")),
            }
        })
        .collect())
}

/// Cartesian product of sets, checked component-wise without building it.
struct Product(Vec<(usize, HashSet<SetVal>)>);

impl Product {
    fn new<'a>(
        exprs: impl IntoIterator<Item = &'a SetExpr>,
        idx_val_map: &IdxValMap,
        lookups: &Lookups,
    ) -> Result<Self> {
        exprs
            .into_iter()
            .map(|expr| {
                let vals = resolve_set_expr(expr, idx_val_map, lookups)?.0;
                Ok((vals.first().map_or(1, arity), vals.into_iter().collect()))
            })
            .collect::<Result<_>>()
            .map(Product)
    }

    fn arity(&self) -> usize {
        self.0.iter().map(|(n, _)| n).sum()
    }

    fn contains(&self, flat: &[SetValTerminal]) -> bool {
        if flat.len() != self.arity() {
            return false;
        }
        let mut rest = flat;
        self.0.iter().all(|(n, set)| {
            let (head, tail) = rest.split_at(*n);
            rest = tail;
            set.contains(&unflatten(head))
        })
    }
}

/// Members of a declared domain, for checking that data keys lie within it.
enum Members {
    Product(Product),
    Enumerated(HashSet<Flat>),
}

impl Members {
    fn new(domain: Option<&Domain>, lookups: &Lookups) -> Result<Self> {
        let empty = IdxValMap::new();
        match domain {
            None => Ok(Members::Product(Product(vec![]))),
            Some(d) if d.condition.is_none() && d.parts.iter().all(|p| is_static(&p.expr)) => {
                let exprs = d.parts.iter().map(|p| &p.expr);
                Ok(Members::Product(Product::new(exprs, &empty, lookups)?))
            }
            Some(d) => Ok(Members::Enumerated(
                domain_to_indexes(d, lookups, &empty)?
                    .iter()
                    .map(|index| flatten(index))
                    .collect(),
            )),
        }
    }

    fn contains(&self, index: &Index) -> bool {
        let flat = flatten(index);
        match self {
            Members::Product(product) => product.contains(&flat),
            Members::Enumerated(set) => set.contains(&flat),
        }
    }
}

fn is_static(expr: &SetExpr) -> bool {
    matches!(expr, SetExpr::Atom(SetAtom::Ref(r)) if r.subscript.0.is_empty())
}

fn arity(val: &SetVal) -> usize {
    match val {
        SetVal::Tuple(t) => t.len(),
        _ => 1,
    }
}

fn flatten(vals: &[SetVal]) -> Flat {
    let mut flat = Flat::new();
    for val in vals {
        match val {
            SetVal::Str(s) => flat.push(SetValTerminal::Str(*s)),
            SetVal::Int(n) => flat.push(SetValTerminal::Int(*n)),
            SetVal::Tuple(t) => flat.extend_from_slice(t),
        }
    }
    flat
}

fn unflatten(flat: &[SetValTerminal]) -> SetVal {
    match flat {
        [single] => single.into(),
        _ => SetVal::Tuple(flat.into()),
    }
}

/// Bind domain dummies to a (flat) data key.
fn flat_index_map(domain: Option<&Domain>, index: &Index) -> IdxValMap {
    let mut map = IdxValMap::new();
    let mut vals = index.iter();
    for part in domain.map_or(&[][..], |d| &d.parts) {
        match &part.var {
            DomainPartVar::None => {
                vals.next();
            }
            DomainPartVar::Single(var) => map.extend(vals.next().map(|v| (*var, v.clone()))),
            DomainPartVar::Tuple(vars) => map.extend(
                vars.iter()
                    .zip(vals.by_ref())
                    .map(|(var, v)| (*var, v.clone())),
            ),
        }
    }
    map
}

fn fmt_index(index: &Index) -> String {
    if index.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = index.iter().map(|v| v.to_string()).collect();
    format!("[{}]", parts.join(","))
}

fn fmt_val(val: &ParamVal) -> String {
    match val {
        ParamVal::Num(n) => n.to_string(),
        ParamVal::Str(s) => intern_resolve(*s).to_string(),
    }
}

fn fmt_term(term: &Term) -> String {
    match term {
        Term::Num(n) => n.to_string(),
        Term::Str(s) => intern_resolve(*s).to_string(),
        Term::Pair(_) => "<var>".to_string(),
    }
}
