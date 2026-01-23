use std::collections::{HashMap, HashSet};

use crate::{
    gmpl::{self, Index, SetData, SetExpr, SetVal, SetVals},
    model::SetWithData,
    mps::{constraints::domain_to_indexes, lookup::Lookups},
};

pub struct SetCont {
    decl: gmpl::Set,
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
        match expr {
            Some(expr) => match expr {
                // This is using a Set domain expression to actually build the values for the set,
                // rather than "get" them from one or more sets
                SetExpr::Domain(domain) => {
                    let idx_val_map: HashMap<String, SetVal> = dims
                        .iter()
                        .zip(index.iter().cloned())
                        .map(|(part, idx_val)| (part.id.clone(), idx_val))
                        .collect();
                    domain_to_indexes(domain, lookups, &idx_val_map)
                        .iter()
                        // TODO we're handling only the special case of a single dimension
                        // to handle more we must check if len > 1 and then build a SetVal::Vec
                        .map(|i| i.first().unwrap().clone())
                        .collect::<Vec<_>>()
                        .into()
                }
                SetExpr::SetMath(set_math) => {
                    let idx_val_map: HashMap<String, SetVal> = dims
                        .iter()
                        .zip(index.iter().cloned())
                        .map(|(part, idx_val)| (part.id.clone(), idx_val))
                        .collect();

                    let sets: Vec<Vec<SetVal>> = set_math
                        .intersection
                        .iter()
                        .map(|v| {
                            let index_concrete: Index = v
                                .subscript
                                .iter()
                                .map(|i| idx_val_map.get(&i.var).unwrap().clone())
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
            },
            None => vec![].into(),
        }
    }
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
