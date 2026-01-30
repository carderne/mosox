use std::collections::HashMap;

use crate::ir::model::ParamWithData;
use crate::ir::{Expr, Index, ParamAssign, ParamDataBody, ParamDataTarget, SetVal};

pub struct Param {
    pub data: ParamVal,
    pub default: Option<Expr>,
}
pub enum ParamVal {
    Arr(HashMap<Index, f64>),
    Scalar(f64),
    Expr(Expr),
    None,
}

pub fn resolve_param(param: ParamWithData) -> Param {
    let default = resolve_param_default(&param);
    if let Some(data) = param.data
        && let Some(body) = data.body
    {
        match body {
            ParamDataBody::Num(num) => Param {
                data: ParamVal::Scalar(num),
                default,
            },
            ParamDataBody::List(pairs) => {
                let mut arr: HashMap<Index, f64> = HashMap::new();
                for pair in pairs {
                    arr.insert(vec![pair.key].into(), pair.value);
                }
                Param {
                    data: ParamVal::Arr(arr),
                    default,
                }
            }
            ParamDataBody::Tables(tables) => {
                let mut arr: HashMap<Index, f64> = HashMap::new();
                for table in tables {
                    // Expressions like:
                    // [Atlantis_00A,NGCC,NOx,*,*]:
                    // Become prefixes for the indexes down below
                    // NOTE: Current implementation ONLY supports having exactly two * (Any)
                    // targets, and they must be the last two
                    let target_idxs: Vec<SetVal> = match table.target {
                        Some(targets) => targets
                            .into_iter()
                            .filter_map(|t| match t {
                                ParamDataTarget::IndexVar(idx) => Some(idx),
                                ParamDataTarget::Any => None,
                            })
                            .collect(),
                        None => vec![],
                    };
                    for row in table.rows {
                        for (col, value) in table.cols.iter().zip(row.values.iter()) {
                            arr.insert(
                                [target_idxs.clone(), vec![row.label, *col]].concat().into(),
                                *value,
                            );
                        }
                    }
                }
                Param {
                    data: ParamVal::Arr(arr),
                    default,
                }
            }
        }
    } else if let Some(ParamAssign::Expr(expr)) = param.decl.assign {
        Param {
            data: ParamVal::Expr(expr),
            default,
        }
    } else {
        Param {
            data: ParamVal::None,
            default,
        }
    }
}

fn resolve_param_default(param: &ParamWithData) -> Option<Expr> {
    if let Some(data) = &param.data {
        if let Some(default) = data.default {
            return Some(Expr::Number(default));
        };
    } else if let Some(default) = &param.decl.default {
        return Some(default.clone());
    };

    None
}
