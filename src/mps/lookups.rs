use std::collections::HashMap;

use indexmap::IndexMap;

use crate::{
    gmpl::IndexVal,
    model::ModelWithData,
    mps::{
        bounds::MpsBounds,
        params::{ParamCont, resolve_param},
    },
};

pub struct Lookups {
    pub set_map: IndexMap<String, Vec<IndexVal>>,
    pub var_map: HashMap<String, MpsBounds>,
    pub par_map: HashMap<String, ParamCont>,
}

impl Lookups {
    pub fn from_model(model: &ModelWithData) -> Self {
        Lookups {
            set_map: model
                .sets
                .clone()
                .into_iter()
                .map(|set| (set.decl.name, set.data.unwrap().values))
                .collect(),
            var_map: model
                .vars
                .clone()
                .into_iter()
                .map(|var| (var.name, MpsBounds::from_gmpl_bounds(var.bounds)))
                .collect(),
            par_map: model
                .params
                .clone()
                .into_iter()
                .map(|param| (param.decl.name.clone(), resolve_param(param)))
                .collect(),
        }
    }
}
