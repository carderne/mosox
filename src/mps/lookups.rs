use std::{collections::HashMap, rc::Rc};

use indexmap::IndexMap;

use crate::{
    gmpl::{IndexVal, Var},
    model::{ParamWithData, SetWithData},
    mps::{
        bounds::Bounds,
        params::{ParamCont, resolve_param},
    },
};

pub struct Lookups {
    pub set_map: IndexMap<String, Vec<IndexVal>>,
    pub var_map: HashMap<String, Rc<Bounds>>,
    pub par_map: HashMap<String, ParamCont>,
}

impl Lookups {
    pub fn from_model(sets: Vec<SetWithData>, vars: Vec<Var>, pars: Vec<ParamWithData>) -> Self {
        Lookups {
            set_map: sets
                .into_iter()
                .map(|set| (set.decl.name, set.data.unwrap().values))
                .collect(),
            var_map: vars
                .into_iter()
                .map(|var| (var.name, Rc::new(Bounds::from_gmpl_bounds(var.bounds))))
                .collect(),
            par_map: pars
                .into_iter()
                .map(|param| (param.decl.name.clone(), resolve_param(param)))
                .collect(),
        }
    }
}
