use std::collections::{HashMap, HashSet};

use crate::{
    ir::{
        ParamVal, SetArith, SetArithEnd, SetAtom, SetInfixOp, Subscript, SubscriptPartVar,
        SubscriptShift, interner::intern_resolve,
    },
    matrix::{
        constraint::{resolve_param, resolve_terms_to_num},
        param::ParamValEnum,
    },
};
use anyhow::{Context, Result, bail};
use lasso::Spur;

use crate::{
    ir::{
        self, DomainPartVar, Index, SetData, SetExpr, SetOf, SetRef, SetVal, SetValTerminal,
        SetVals, SetValue, model::SetWithData,
    },
    matrix::{
        constraint::{IdxValMap, domain_to_indexes, get_index_map},
        lookup::Lookups,
    },
};

pub struct SetCont {
    decl: ir::Set,
    data: HashMap<Index, SetVals>,
}

impl From<SetWithData> for SetCont {
    fn from(inner: SetWithData) -> Self {
        let SetWithData { decl, data } = inner;

        let data = data
            .into_iter()
            .map(
                |SetData {
                     name: _,
                     index,
                     values,
                 }| (index, values),
            )
            .collect();

        SetCont { decl, data }
    }
}

impl SetCont {
    pub fn resolve(&self, index: &Index, lookups: &Lookups) -> Result<SetVals> {
        // Data takes preference over expressions (probably)
        if let Some(set_data) = self.data.get(index) {
            // TODO Should also check that the within/cross conditions are met!
            return Ok(set_data.clone());
        }

        // I tried add a cache check here with a RwLock<HashMap<...>> but
        // there wasn't any speed up. Possibly because of cloning and expensive hashkeys

        let (domain, expr) = (&self.decl.domain, &self.decl.expr);

        // Try to resolve from expression
        if let Some(expr) = expr {
            let idx_val_map = get_index_map(&domain.parts, index)?;
            return resolve_set_expr(expr, &idx_val_map, lookups);
        }

        // Finally use default if available
        if let Some(default) = &self.decl.default {
            return match default {
                SetValue::Vals(vals) => Ok(vals.clone()),
                SetValue::Expr(expr) => {
                    let idx_val_map = get_index_map(&domain.parts, index)?;
                    resolve_set_expr(expr, &idx_val_map, lookups)
                }
            };
        }

        // No data, no expr, no default
        // TODO: Apply set dimension (dimen) validation at model generation time
        Ok(vec![].into())
    }
}

fn resolve_set_atom(expr: &SetAtom, idx_val_map: &IdxValMap, lookups: &Lookups) -> Result<SetVals> {
    match expr {
        // This is using a Set domain expression to actually build the values for the set,
        // rather than "get" them from one or more sets
        SetAtom::Domain(domain) => {
            Ok(domain_to_indexes(domain, lookups, idx_val_map)?
                .iter()
                // TODO we're handling only the special case of a single dimension
                // to handle more we must check if len > 1 and then build a SetVal::Tuple
                .map(|i| *i.first().unwrap())
                .collect::<Vec<_>>()
                .into())
        }
        SetAtom::SetOf(set_of) => resolve_set_of(set_of, idx_val_map, lookups),
        SetAtom::Ref(SetRef { spur, subscript }) => {
            let index = concrete_index(subscript, idx_val_map, lookups)?;
            lookups.set_map.get(spur).unwrap().resolve(&index, lookups)
        }
        SetAtom::Arith(arith) => resolve_set_arith(arith, idx_val_map, lookups),
    }
}

pub fn resolve_set_expr(
    expr: &SetExpr,
    idx_val_map: &IdxValMap,
    lookups: &Lookups,
) -> Result<SetVals> {
    match expr {
        SetExpr::Atom(atom) => resolve_set_atom(atom, idx_val_map, lookups),
        SetExpr::InfixOp { lhs, op, rhs } => {
            let lhs = resolve_set_expr(lhs, idx_val_map, lookups)?.0;
            let rhs = resolve_set_expr(rhs, idx_val_map, lookups)?.0;
            match op {
                SetInfixOp::Inter => Ok(intersect(lhs, rhs).into()),
                SetInfixOp::Union => Ok(union(lhs, rhs).into()),
            }
        }
    }
}

fn resolve_set_of(set_of: &SetOf, idx_val_map: &IdxValMap, lookups: &Lookups) -> Result<SetVals> {
    // Get all index combinations from the domain
    let domain_indexes = domain_to_indexes(&set_of.domain, lookups, idx_val_map)?;

    // Extract the integrand values for each domain element
    let mut result = Vec::new();
    for idx in domain_indexes {
        // Build a map from domain vars to their values for this iteration
        let iter_map: IdxValMap = set_of
            .domain
            .parts
            .iter()
            .zip(idx.iter())
            .map(|(part, val)| {
                Ok(match &part.var {
                    DomainPartVar::None => bail!("Need domain part var in setof expression"),
                    DomainPartVar::Single(id) => vec![(*id, *val)],
                    DomainPartVar::Tuple(ids) => {
                        // For tuple bindings, the val should be a Tuple
                        match val {
                            SetVal::Tuple([a, b]) => {
                                let mut mappings = Vec::new();
                                if let Some(id) = ids.first() {
                                    mappings.push((
                                        *id,
                                        match a {
                                            SetValTerminal::Str(s) => SetVal::Str(*s),
                                            SetValTerminal::Int(i) => SetVal::Int(*i),
                                        },
                                    ));
                                }
                                if let Some(id) = ids.get(1) {
                                    mappings.push((
                                        *id,
                                        match b {
                                            SetValTerminal::Str(s) => SetVal::Str(*s),
                                            SetValTerminal::Int(i) => SetVal::Int(*i),
                                        },
                                    ));
                                }
                                mappings
                            }
                            _ => vec![],
                        }
                    }
                })
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        // Extract integrand value(s)
        match &set_of.integrand {
            DomainPartVar::None => bail!("Need domain part var in setof expression"),
            DomainPartVar::Single(id) => {
                // Should this actually return an error if the idx_get fails?
                if let Ok(val) = idx_get(&iter_map, *id) {
                    result.push(val);
                }
            }
            DomainPartVar::Tuple(ids) => {
                // Build tuple from integrand vars
                let vals: Vec<SetValTerminal> = ids
                    .iter()
                    .filter_map(|id| idx_get(&iter_map, *id).ok())
                    .map(|v| match v {
                        SetVal::Str(s) => SetValTerminal::Str(s),
                        SetVal::Int(i) => SetValTerminal::Int(i),
                        _ => unreachable!(),
                    })
                    .collect();
                if vals.len() == 2 {
                    result.push(SetVal::Tuple([vals[0], vals[1]]));
                }
            }
        }
    }

    Ok(result.into())
}

/// Resolve a `DomainPartRange` (e.g. `1..NbYears` or `1..NbSeasons[y]`) to the sequence of
/// integer `SetVal`s it represents: `[lo, lo+1, ..., hi]` inclusive.
pub fn resolve_set_arith(
    arith: &SetArith,
    idx_val_map: &IdxValMap,
    lookups: &Lookups,
) -> Result<SetVals> {
    let lo = resolve_range_end(&arith.lo, idx_val_map, lookups)?;
    let hi = resolve_range_end(&arith.hi, idx_val_map, lookups)?;
    Ok((lo..=hi).map(SetVal::Int).collect::<Vec<_>>().into())
}

/// Resolve a single range endpoint to a concrete `u32`.
fn resolve_range_end(end: &SetArithEnd, idx_val_map: &IdxValMap, lookups: &Lookups) -> Result<u32> {
    match end {
        SetArithEnd::Int(n) => Ok(*n),
        SetArithEnd::Named {
            name,
            subscript,
            shift,
        } => {
            let index = concrete_index(subscript, idx_val_map, lookups)?;

            // Look up as a param — range ends must be scalar integers
            let param = lookups.par_map.get(name).with_context(|| {
                format!("range end '{}' not found in params", intern_resolve(*name))
            })?;

            let base = match &param.data {
                ParamValEnum::Arr(arr) => *arr
                    .get(&index)
                    .context("no value at index for range end param")?,
                _ => bail!("range end param must be a numeric scalar or array"),
            };

            let base = match base {
                ParamVal::Str(_) => bail!("Cannot have symbolic param in arithmetic set"),
                ParamVal::Num(num) => num,
            } as u32;

            match shift {
                Some(shift) => match shift {
                    SubscriptShift::Plus(offset) => Ok(base + offset),
                    SubscriptShift::Minus(offset) => Ok(base - offset),
                },
                None => Ok(base),
            }
        }
    }
}

fn intersect<T: Eq + std::hash::Hash + Clone>(a: Vec<T>, b: Vec<T>) -> Vec<T> {
    let set: HashSet<T> = b.into_iter().collect();
    a.into_iter().filter(|x| set.contains(x)).collect()
}

fn union<T: Eq + std::hash::Hash>(a: Vec<T>, b: Vec<T>) -> Vec<T> {
    let mut set: HashSet<T> = a.into_iter().collect();
    set.extend(b);
    set.into_iter().collect()
}

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

fn idx_val_or_get(var: &SubscriptPartVar, map: &IdxValMap, lookups: &Lookups) -> Result<SetVal> {
    match var {
        SubscriptPartVar::ValStr(val) => Ok(SetVal::Str(*val)),
        SubscriptPartVar::ValInt(val) => Ok(SetVal::Int(*val)),
        SubscriptPartVar::Var(inner) => match idx_get(map, inner.var) {
            // No way to know without checking whether the reference is to an index value or a
            // param. Eg MyParam[i, foo, bar[qux]]
            // i is probably an index, foo could be anything, bar definitely a param
            Ok(val) => Ok(val),
            Err(err) => match lookups.par_map.get(&inner.var) {
                Some(param) => {
                    let index = concrete_index(&inner.subscript, map, lookups)?;
                    let terms = resolve_param(param, &index, map, lookups)?;
                    let num = resolve_terms_to_num(&terms)?
                        .context("cannot reference variables inside subscript")?;
                    Ok(SetVal::Int(num as u32))
                }
                None => Err(err),
            },
        },
    }
}

/// Convert from symbolic ("dummy" in GMPL parlance) subscript
/// to actual indexable values
pub fn concrete_index(
    subscript: &Subscript,
    idx_val_map: &IdxValMap,
    lookups: &Lookups,
) -> Result<Index> {
    Ok(subscript
        .iter()
        .map(|i| {
            // First try to look up as a domain variable
            // If not found, check if it's a literal number
            let index_val = idx_val_or_get(&i.var, idx_val_map, lookups)?;
            match &i.shift {
                Some(shift) => match index_val {
                    SetVal::Str(_) => {
                        bail!("tried to index shift on string index val")
                    }
                    SetVal::Int(index_num) => match shift {
                        SubscriptShift::Plus(offset) => Ok(SetVal::Int(index_num + offset)),
                        SubscriptShift::Minus(offset) => Ok(SetVal::Int(index_num - offset)),
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
