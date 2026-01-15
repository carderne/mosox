use std::fmt;

use crate::{
    gmpl::{VarBounds, atoms::RelOp},
    mps::{Bounds, Cols, lookups::Lookups},
};

pub fn gen_bounds(cols: &Cols, lookups: &Lookups) -> Bounds {
    cols.into_iter()
        .map(|((var_name, var_idx), _)| {
            (
                (var_name.clone(), var_idx.clone()),
                lookups.var_map.get(var_name).unwrap().clone(),
            )
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct MpsBounds {
    pub op: BoundsOp,
    pub val: Option<f64>,
}

impl MpsBounds {
    pub fn from_gmpl_bounds(bounds: Option<VarBounds>) -> Self {
        match bounds {
            Some(bounds) => MpsBounds {
                op: BoundsOp::from_rel_op(&bounds.op),
                val: Some(bounds.value),
            },
            None => MpsBounds {
                op: BoundsOp::FR,
                val: None,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundsOp {
    FR,
    LO,
    UP,
    FX,
}

impl BoundsOp {
    pub fn from_rel_op(op: &RelOp) -> Self {
        match op {
            RelOp::Lt => panic!("Less than not supported"),
            RelOp::Le => BoundsOp::UP,
            RelOp::Eq => BoundsOp::FX,
            RelOp::EqEq => BoundsOp::FX,
            RelOp::Ne => panic!("Not equal not supported"),
            RelOp::Ne2 => panic!("Not equal not supported"),
            RelOp::Ge => BoundsOp::LO,
            RelOp::Gt => panic!("Greater than not supported"),
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
