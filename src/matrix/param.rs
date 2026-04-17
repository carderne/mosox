use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::ir::model::ParamWithData;
use crate::ir::{
    Expr, Index, ParamAssign, ParamDataBody, ParamDataPlainValue, ParamDataTarget, ParamVal, SetVal,
};

#[derive(Debug, Clone)]
pub struct Param {
    pub data: ParamValEnum,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub enum ParamValEnum {
    Arr(HashMap<Index, ParamVal>),
    Expr(Expr),
    None,
}

pub fn create_param(param: ParamWithData) -> Result<Param> {
    let default = resolve_param_default(&param)?;
    if let Some(data) = param.data
        && let Some(body) = data.body
    {
        match body {
            ParamDataBody::Plain(plain_entries) => {
                if plain_entries.is_empty() {
                    Ok(Param {
                        data: ParamValEnum::None,
                        default,
                    })
                } else {
                    let mut arr: HashMap<Index, ParamVal> = HashMap::new();
                    for entry in plain_entries {
                        let target_idxs = param_target_to_index(entry.target);
                        match entry.value {
                            ParamDataPlainValue::Scalar(val) => {
                                arr.insert(target_idxs.into(), val);
                            }
                            ParamDataPlainValue::Pairs(pairs) => {
                                for pair in pairs {
                                    arr.insert(
                                        [target_idxs.clone(), vec![pair.key]].concat().into(),
                                        pair.value,
                                    );
                                }
                            }
                        }
                    }
                    Ok(Param {
                        data: ParamValEnum::Arr(arr),
                        default,
                    })
                }
            }
            ParamDataBody::Tabbing(_) => {
                // Tabbing is resolved to Plain during model merge; reaching
                // here means resolve_tabbing was skipped.
                bail!("internal: unresolved tabbing body reached matrix resolution")
            }
            ParamDataBody::Tabular(tables) => {
                let mut arr: HashMap<Index, ParamVal> = HashMap::new();
                for table in tables {
                    let target_idxs = param_target_to_index(table.target);
                    for row in table.rows {
                        for (col, value) in table.cols.iter().zip(row.values.iter()) {
                            arr.insert(
                                [target_idxs.clone(), vec![row.label.clone(), col.clone()]]
                                    .concat()
                                    .into(),
                                *value,
                            );
                        }
                    }
                }
                Ok(Param {
                    data: ParamValEnum::Arr(arr),
                    default,
                })
            }
        }
    } else if let Some(ParamAssign::Expr(expr)) = param.decl.assign {
        Ok(Param {
            data: ParamValEnum::Expr(expr),
            default,
        })
    } else {
        Ok(Param {
            data: ParamValEnum::None,
            default,
        })
    }
}

fn param_target_to_index(target: Option<Vec<ParamDataTarget>>) -> Vec<SetVal> {
    // Expressions like:
    // [Atlantis_00A,NGCC,NOx,*,*]:
    // Become prefixes for the indexes down below
    match target {
        Some(targets) => targets
            .into_iter()
            .filter_map(|t| match t {
                ParamDataTarget::IndexVar(idx) => Some(idx),
                ParamDataTarget::Any => None,
            })
            .collect(),
        None => vec![],
    }
}

fn resolve_param_default(param: &ParamWithData) -> Result<Option<Expr>> {
    if let Some(data) = &param.data {
        if let Some(default) = data.default {
            match default {
                ParamVal::Num(num) => return Ok(Some(Expr::Number(num))),
                ParamVal::Str(_) => bail!("no support for symbolic default param value"),
            }
        };
    } else if let Some(default) = &param.decl.default {
        return Ok(Some(default.clone()));
    };

    Ok(None)
}
