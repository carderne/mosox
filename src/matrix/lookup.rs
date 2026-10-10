use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use lasso::Spur;

use crate::{
    data::{DataSource, set_dimen},
    ir::{self, VarType, interner::intern_resolve, model::Model, op::Bounds},
    matrix::{
        param::{Param, create_param},
        set::SetCont,
    },
};

pub struct VarCont {
    pub var_type: VarType,
    pub bounds: Bounds,
}

pub struct Lookups {
    pub set_map: IndexMap<Spur, SetCont>,
    pub var_map: HashMap<Spur, VarCont>,
    pub par_map: HashMap<Spur, Param>,
}

impl Lookups {
    pub fn new(model: &Model, mut source: impl DataSource) -> Result<Self> {
        let declared: HashSet<Spur> = (model.sets.iter().map(|s| s.name))
            .chain(model.params.iter().map(|p| p.name))
            .collect();
        if let Some(name) = source.names().into_iter().find(|n| !declared.contains(n)) {
            bail!(
                "Data for '{}' has no matching model declaration",
                intern_resolve(name)
            );
        }

        let decls: HashMap<Spur, &ir::Set> = model.sets.iter().map(|s| (s.name, s)).collect();
        let set_map = (model.sets.iter())
            .map(|decl| {
                let name = intern_resolve(decl.name);
                let data = source
                    .take_set(decl, set_dimen(decl, &decls))
                    .with_context(|| format!("in data for set '{name}'"))?;
                let decl = decl.clone();
                Ok((
                    decl.name,
                    SetCont {
                        decl,
                        data: data.unwrap_or_default(),
                    },
                ))
            })
            .collect::<Result<_>>()?;
        let par_map = (model.params.iter())
            .map(|decl| {
                let name = intern_resolve(decl.name);
                let values = source
                    .take_param(decl)
                    .with_context(|| format!("in data for param '{name}'"))?;
                Ok((decl.name, create_param(decl.clone(), values)))
            })
            .collect::<Result<_>>()?;
        let var_map = (model.vars.iter())
            .map(|var| {
                let cont = VarCont {
                    var_type: var.var_type,
                    bounds: Bounds::from_gmpl_bounds(var.clone())?,
                };
                Ok((var.name, cont))
            })
            .collect::<Result<_>>()?;

        Ok(Lookups {
            set_map,
            var_map,
            par_map,
        })
    }
}
