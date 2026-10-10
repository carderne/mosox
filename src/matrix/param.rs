use std::collections::HashMap;

use crate::data::ParamValues;
use crate::ir::{self, Expr, Index, ParamAssign, ParamVal};

#[derive(Debug, Clone)]
pub struct Param {
    pub decl: ir::Param,
    pub data: ParamValEnum,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub enum ParamValEnum {
    Arr(HashMap<Index, ParamVal>),
    Expr(Expr),
    None,
}

pub fn create_param(mut decl: ir::Param, provided: Option<ParamValues>) -> Param {
    let (values, data_default) = provided.map_or_else(Default::default, |p| (p.values, p.default));
    let assign = decl.assign.take();
    let data = match assign {
        _ if !values.is_empty() => ParamValEnum::Arr(values),
        Some(ParamAssign::Expr(expr)) => ParamValEnum::Expr(expr),
        _ => ParamValEnum::None,
    };
    // A default given with the data overrides the model's
    let default = data_default
        .map(|val| match val {
            ParamVal::Num(n) => Expr::Number(n),
            ParamVal::Str(s) => Expr::Str(s),
        })
        .or_else(|| decl.default.clone());
    Param {
        decl,
        data,
        default,
    }
}
