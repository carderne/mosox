use std::collections::{HashMap, HashSet};

use crate::{
    ir::{
        Domain, SetArith, SetArithEnd, SetAtom, SetInfixOp, Subscript, SubscriptPartVar,
        SubscriptShift, interner::intern_resolve,
    },
    matrix::constraint::resolve_param_to_setval,
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
        Ok(vec![].into())
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

/// Build a Set from one of the expression variants enumerated below.
fn resolve_set_atom(expr: &SetAtom, idx_val_map: &IdxValMap, lookups: &Lookups) -> Result<SetVals> {
    match expr {
        // This is using a Set domain expression to actually build the values for the set,
        // rather than "get" them from one or more sets
        SetAtom::Domain(domain) => resolve_set_from_domain(domain, idx_val_map, lookups),
        SetAtom::SetOf(set_of) => resolve_set_of(set_of, idx_val_map, lookups),
        SetAtom::Ref(SetRef { spur, subscript }) => {
            let index = concrete_index(subscript, idx_val_map, lookups)?;
            lookups.set_map.get(spur).unwrap().resolve(&index, lookups)
        }
        SetAtom::Arith(arith) => resolve_set_arith(arith, idx_val_map, lookups),
    }
}

/// Build a set by enumerating a domain
/// Given these sets:
///     set YEAR;
///     set FUEL;
///     set BLOB{FUEL} dimen 2;
///
/// Then this expression:
///     set BOB := {y in YEAR, f in FUEL, (b1,b2) in BLOB[f]};
///
/// Will produce [(y,f,b1,b2), ...]
/// (With values interpolated.)
fn resolve_set_from_domain(
    domain: &Domain,
    idx_val_map: &IdxValMap,
    lookups: &Lookups,
) -> Result<SetVals> {
    Ok(domain_to_indexes(domain, lookups, idx_val_map)?
        .iter()
        .map(|index| {
            if index.is_empty() {
                bail!("Need at least one dimension to create set from domain");
            } else {
                let mut arr: Vec<SetValTerminal> = vec![];
                for i in index {
                    match i {
                        SetVal::Str(val) => arr.push(SetValTerminal::Str(*val)),
                        SetVal::Int(val) => arr.push(SetValTerminal::Int(*val)),
                        SetVal::Tuple(vals) => arr.extend(vals),
                    }
                }
                if arr.len() == 1 {
                    Ok(arr.first().unwrap().into())
                } else {
                    Ok(SetVal::Tuple(arr.into()))
                }
            }
        })
        .collect::<Result<Vec<_>>>()?
        .into())
}

/// Resolve a GMPL `setof` expression.
///
/// Iterates over the domain, evaluating the integrand for each combination of dummy indices
/// to produce the result set.
///
/// - **Single integrand** (`setof{i in S} i`): produces a set of 1-tuples (scalar values).
/// - **Tuple integrand** (`setof{i in S} (i, f[i])`): produces a set of m-tuples.
fn resolve_set_of(set_of: &SetOf, idx_val_map: &IdxValMap, lookups: &Lookups) -> Result<SetVals> {
    Ok(domain_to_indexes(&set_of.domain, lookups, idx_val_map)?
        .into_iter()
        .map(|idx| {
            let local_map: IdxValMap = set_of
                .domain
                .parts
                .iter()
                .zip(idx.iter())
                .map(|(part, val)| {
                    Ok(match &part.var {
                        DomainPartVar::None => bail!("Need domain part var in setof expression"),
                        DomainPartVar::Single(id) => vec![(*id, val.clone())],
                        DomainPartVar::Tuple(ids) => match val {
                            SetVal::Tuple(tuple) => ids
                                .iter()
                                .zip(tuple.iter())
                                .map(|(id, val)| (*id, val.into()))
                                .collect(),
                            _ => bail!("Cannot have tuple index pointing at non-tuple value"),
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            Ok(match &set_of.integrand {
                DomainPartVar::None => bail!("Need domain part var in setof expression"),
                DomainPartVar::Single(id) => idx_get(&local_map, *id)?,
                DomainPartVar::Tuple(ids) => {
                    // Build tuple from integrand vars
                    let vals: Vec<SetValTerminal> = ids
                        .iter()
                        .filter_map(|id| idx_get(&local_map, *id).ok())
                        .map(|v| match v {
                            SetVal::Str(s) => SetValTerminal::Str(s),
                            SetVal::Int(i) => SetValTerminal::Int(i),
                            _ => unreachable!(),
                        })
                        .collect();
                    SetVal::Tuple(vals.into())
                }
            })
        })
        .collect::<Result<Vec<_>>>()?
        .into())
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
            let base = match resolve_param_to_setval(name, subscript, idx_val_map, lookups)? {
                SetVal::Int(val) => val,
                _ => bail!("Can only use integer param in set arithmetic expr"),
            };
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
        .cloned()
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
            Err(_) => Ok(resolve_param_to_setval(
                &inner.var,
                &inner.subscript,
                map,
                lookups,
            )?),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        Domain, DomainPart, DomainPartVar, SetAtom, SetExpr, SetOf, SetVal, SetValTerminal,
        interner::intern, model::SetWithData,
    };
    use indexmap::IndexMap;
    use smallvec::smallvec;
    use std::collections::HashMap;

    /// Build a minimal `Lookups` containing only the given named sets.
    fn lookups_with_sets(sets: Vec<SetWithData>) -> Lookups {
        Lookups {
            set_map: sets
                .into_iter()
                .map(|s| (s.decl.name, SetCont::from(s)))
                .collect::<IndexMap<_, _>>(),
            var_map: HashMap::new(),
            par_map: HashMap::new(),
        }
    }

    /// Helper: create a `SetWithData` with no domain and literal values.
    fn simple_set(name: &str, vals: Vec<SetVal>) -> SetWithData {
        SetWithData {
            decl: ir::Set {
                name: intern(name),
                domain: Domain::default(),
                dimen: None,
                within: None,
                cross: None,
                expr: None,
                inline_data: None,
                default: None,
            },
            data: vec![ir::SetData {
                name: intern(name),
                index: smallvec![],
                values: vals.into(),
            }],
        }
    }

    /// `setof{p in PEOPLE} p` over `PEOPLE := {10, 20, 30}`
    /// should produce `{10, 20, 30}`.
    #[test]
    fn test_setof_single_identity() {
        let p = intern("p");
        let people_name = intern("PEOPLE");

        let people = simple_set(
            "PEOPLE",
            vec![SetVal::Int(10), SetVal::Int(20), SetVal::Int(30)],
        );
        let lookups = lookups_with_sets(vec![people]);

        let set_of = SetOf {
            domain: Domain {
                parts: vec![DomainPart {
                    var: DomainPartVar::Single(p),
                    expr: SetExpr::Atom(SetAtom::Ref(SetRef {
                        spur: people_name,
                        subscript: Subscript::default(),
                    })),
                }],
                condition: None,
            },
            integrand: DomainPartVar::Single(p),
        };

        let result = resolve_set_of(&set_of, &smallvec![], &lookups).unwrap();
        assert_eq!(
            result.0,
            vec![SetVal::Int(10), SetVal::Int(20), SetVal::Int(30)]
        );
    }

    /// `setof{p in PEOPLE} (p, p)` over `PEOPLE := {"alice", "bob"}`
    /// should produce `{("alice","alice"), ("bob","bob")}`.
    #[test]
    fn test_setof_tuple_integrand() {
        let p = intern("p");
        let people_name = intern("PEOPLE");

        let alice = intern("alice");
        let bob = intern("bob");

        let people = simple_set("PEOPLE", vec![SetVal::Str(alice), SetVal::Str(bob)]);
        let lookups = lookups_with_sets(vec![people]);

        let set_of = SetOf {
            domain: Domain {
                parts: vec![DomainPart {
                    var: DomainPartVar::Single(p),
                    expr: SetExpr::Atom(SetAtom::Ref(SetRef {
                        spur: people_name,
                        subscript: Subscript::default(),
                    })),
                }],
                condition: None,
            },
            integrand: DomainPartVar::Tuple(vec![p, p]),
        };

        let result = resolve_set_of(&set_of, &smallvec![], &lookups).unwrap();
        assert_eq!(
            result.0,
            vec![
                SetVal::Tuple(smallvec![
                    SetValTerminal::Str(alice),
                    SetValTerminal::Str(alice)
                ]),
                SetVal::Tuple(smallvec![
                    SetValTerminal::Str(bob),
                    SetValTerminal::Str(bob)
                ]),
            ]
        );
    }
}
