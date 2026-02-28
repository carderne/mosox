use std::collections::HashMap;

use anyhow::Result;
use indexmap::IndexMap;
use lasso::Spur;

use crate::{
    ir::{
        self, VarType,
        model::{ParamWithData, SetWithData},
        op::Bounds,
    },
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
    pub fn from_model(
        sets: Vec<SetWithData>,
        vars: Vec<ir::Var>,
        pars: Vec<ParamWithData>,
    ) -> Result<Self> {
        Ok(Lookups {
            set_map: sets
                .into_iter()
                .map(|set| (set.decl.name, SetCont::from(set)))
                .collect(),
            var_map: vars
                .into_iter()
                .map(|var| {
                    Ok((
                        var.name,
                        VarCont {
                            var_type: var.var_type,
                            bounds: Bounds::from_gmpl_bounds(var.bounds)?,
                        },
                    ))
                })
                .collect::<Result<_>>()?,
            par_map: pars
                .into_iter()
                .map(|param| Ok((param.decl.name, create_param(param)?)))
                .collect::<Result<_>>()?,
        })
    }
}
