use std::collections::HashMap;

use indexmap::IndexMap;
use lasso::Spur;

use crate::{
    ir::{
        self,
        model::{ParamWithData, SetWithData},
    },
    matrix::{
        bound::Bounds,
        param::{Param, resolve_param},
        set::SetCont,
    },
};

pub struct Lookups {
    pub set_map: IndexMap<Spur, SetCont>,
    pub var_map: HashMap<Spur, Bounds>,
    pub par_map: HashMap<Spur, Param>,
}

impl Lookups {
    pub fn from_model(
        sets: Vec<SetWithData>,
        vars: Vec<ir::Var>,
        pars: Vec<ParamWithData>,
    ) -> Self {
        Lookups {
            set_map: sets
                .into_iter()
                .map(|set| (set.decl.name, SetCont::from(set)))
                .collect(),
            var_map: vars
                .into_iter()
                .map(|var| (var.name, Bounds::from_gmpl_bounds(var.bounds)))
                .collect(),
            par_map: pars
                .into_iter()
                .map(|param| (param.decl.name, resolve_param(param)))
                .collect(),
        }
    }
}
