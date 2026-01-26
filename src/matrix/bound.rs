use std::fmt;

use crate::{
    ir,
    matrix::{BoundsMap, ColsMap, lookup::Lookups},
};

pub fn gen_bounds(cols: &ColsMap, lookups: Lookups) -> BoundsMap {
    cols.iter()
        .map(|((var_name, var_idx), _)| {
            (
                (*var_name, var_idx.clone()),
                *lookups.var_map.get(var_name).unwrap(),
            )
        })
        .collect()
}

#[derive(Copy, Clone, Debug)]
pub struct Bounds {
    pub op: BoundsOp,
    pub val: Option<f64>,
}

impl Bounds {
    pub fn from_gmpl_bounds(bounds: Option<ir::VarBounds>) -> Self {
        match bounds {
            Some(bounds) => Bounds {
                op: BoundsOp::from_rel_op(&bounds.op),
                val: Some(bounds.value),
            },
            None => Bounds {
                op: BoundsOp::FR,
                val: None,
            },
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BoundsOp {
    FR,
    LO,
    UP,
    FX,
}

impl BoundsOp {
    pub fn from_rel_op(op: &ir::RelOp) -> Self {
        match op {
            ir::RelOp::Lt => panic!("Less than not supported"),
            ir::RelOp::Le => BoundsOp::UP,
            ir::RelOp::Eq => BoundsOp::FX,
            ir::RelOp::EqEq => BoundsOp::FX,
            ir::RelOp::Ne => panic!("Not equal not supported"),
            ir::RelOp::Ne2 => panic!("Not equal not supported"),
            ir::RelOp::Ge => BoundsOp::LO,
            ir::RelOp::Gt => panic!("Greater than not supported"),
        }
    }
}

impl fmt::Display for BoundsOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BoundsOp::FR => write!(f, "FR"),
            BoundsOp::LO => write!(f, "LO"),
            BoundsOp::UP => write!(f, "UP"),
            BoundsOp::FX => write!(f, "FX"),
        }
    }
}
