use std::collections::{HashMap, HashSet};

use crate::{
    ir::model::SetWithData,
    ir::{
        self, DomainPartVar, Index, SetData, SetExpr, SetOf, SetVal, SetValTerminal, SetVals,
        SetValue,
    },
    matrix::{
        constraint::{IdxValMap, domain_to_indexes, idx_get},
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
    pub fn resolve(&self, index: &Index, lookups: &Lookups) -> SetVals {
        // Data takes preference over expressions (probably)
        if let Some(set_data) = self.data.get(index) {
            // Should also check that the within/cross conditions are met!
            return set_data.clone();
        }

        // I tried add a cache check here with a RwLock<HashMap<...>> but
        // there wasn't any speed up. Possibly because of cloning and expensive hashkeys

        let (dims, expr) = (&self.decl.dims, &self.decl.expr);

        // Try to resolve from expression
        if let Some(expr) = expr {
            return self.resolve_set_expr(expr, dims, index, lookups);
        }

        // Finally use default if available
        if let Some(default) = &self.decl.default {
            return match default {
                SetValue::Vals(vals) => vals.clone(),
                SetValue::Expr(expr) => self.resolve_set_expr(expr, dims, index, lookups),
            };
        }

        // No data, no expr, no default
        // TODO: Apply set dimension (dimen) validation at model generation time
        vec![].into()
    }

    fn resolve_set_expr(
        &self,
        expr: &SetExpr,
        dims: &[ir::SetDomainPart],
        index: &Index,
        lookups: &Lookups,
    ) -> SetVals {
        match expr {
            // This is using a Set domain expression to actually build the values for the set,
            // rather than "get" them from one or more sets
            SetExpr::Domain(domain) => {
                let idx_val_map: IdxValMap = dims
                    .iter()
                    .zip(index.iter().cloned())
                    .map(|(part, idx_val)| {
                        (
                            part.id
                                .expect("need id in set domain when using set domain expr"),
                            idx_val,
                        )
                    })
                    .collect();
                domain_to_indexes(domain, lookups, &idx_val_map)
                    .iter()
                    // TODO we're handling only the special case of a single dimension
                    // to handle more we must check if len > 1 and then build a SetVal::Tuple
                    .map(|i| *i.first().unwrap())
                    .collect::<Vec<_>>()
                    .into()
            }
            SetExpr::SetMath(set_math) => {
                let idx_val_map: IdxValMap = dims
                    .iter()
                    .zip(index.iter().cloned())
                    .map(|(part, idx_val)| {
                        (
                            part.id.expect("need id in set domain when using set expr"),
                            idx_val,
                        )
                    })
                    .collect();

                let sets: Vec<Vec<SetVal>> = set_math
                    .intersection
                    .iter()
                    .map(|v| {
                        let index_concrete: Index = v
                            .subscript
                            .iter()
                            .map(|i| *idx_get(&idx_val_map, i.var).unwrap())
                            .collect::<Vec<_>>()
                            .into();
                        lookups
                            .set_map
                            .get(&v.var)
                            .unwrap()
                            .resolve(&index_concrete, lookups)
                            .0
                    })
                    .collect();

                intersect(sets).into()
            }
            SetExpr::SetOf(set_of) => resolve_set_of(set_of, dims, index, lookups),
        }
    }
}

fn resolve_set_of(
    set_of: &SetOf,
    dims: &[ir::SetDomainPart],
    index: &Index,
    lookups: &Lookups,
) -> SetVals {
    let idx_val_map: IdxValMap = dims
        .iter()
        .zip(index.iter().cloned())
        .map(|(part, idx_val)| {
            (
                part.id
                    .expect("need id in set domain when using setof expr"),
                idx_val,
            )
        })
        .collect();

    // Get all index combinations from the domain
    let domain_indexes = domain_to_indexes(&set_of.domain, lookups, &idx_val_map);

    // Extract the integrand values for each domain element
    let mut result = Vec::new();
    for idx in domain_indexes {
        // Build a map from domain vars to their values for this iteration
        let iter_map: IdxValMap = set_of
            .domain
            .parts
            .iter()
            .zip(idx.iter())
            .flat_map(|(part, val)| match &part.var {
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
            .collect();

        // Extract integrand value(s)
        match &set_of.integrand {
            DomainPartVar::Single(id) => {
                if let Some(val) = idx_get(&iter_map, *id) {
                    result.push(*val);
                }
            }
            DomainPartVar::Tuple(ids) => {
                // Build tuple from integrand vars
                let vals: Vec<SetValTerminal> = ids
                    .iter()
                    .filter_map(|id| idx_get(&iter_map, *id))
                    .map(|v| match v {
                        SetVal::Str(s) => SetValTerminal::Str(*s),
                        SetVal::Int(i) => SetValTerminal::Int(*i),
                        _ => unreachable!(),
                    })
                    .collect();
                if vals.len() == 2 {
                    result.push(SetVal::Tuple([vals[0], vals[1]]));
                }
            }
        }
    }

    result.into()
}

fn intersect<T: Eq + std::hash::Hash + Clone>(vecs: Vec<Vec<T>>) -> Vec<T> {
    let mut iter = vecs.into_iter();
    let mut result: HashSet<T> = iter.next().unwrap_or_default().into_iter().collect();

    for v in iter {
        let set: HashSet<T> = v.into_iter().collect();
        result.retain(|x| set.contains(x));
    }

    result.into_iter().collect()
}
