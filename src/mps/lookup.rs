use std::{collections::HashMap, sync::Arc};

use indexmap::IndexMap;

use crate::{
    gmpl,
    model::{ParamWithData, SetWithData},
    mps::{
        bound::Bounds,
        param::{Param, resolve_param},
        set::SetCont,
    },
};

pub struct Lookups {
    pub set_map: IndexMap<String, SetCont>,
    pub var_map: HashMap<String, Arc<Bounds>>,
    pub par_map: HashMap<String, Param>,
}

impl Lookups {
    pub fn from_model(
        sets: Vec<SetWithData>,
        vars: Vec<gmpl::Var>,
        pars: Vec<ParamWithData>,
    ) -> Self {
        Lookups {
            set_map: sets
                .into_iter()
                .map(|set| (set.decl.name.clone(), SetCont::from(set)))
                .collect(),
            var_map: vars
                .into_iter()
                .map(|var| (var.name, Arc::new(Bounds::from_gmpl_bounds(var.bounds))))
                .collect(),
            par_map: pars
                .into_iter()
                .map(|param| (param.decl.name.clone(), resolve_param(param)))
                .collect(),
        }
    }
}
