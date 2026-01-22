use std::fmt;

use crate::{
    gmpl,
    mps::{BoundsMap, ColsMap, lookup::Lookups},
};

pub fn gen_bounds(cols: &ColsMap, lookups: Lookups) -> BoundsMap {
    cols.iter()
        .map(|((var_name, var_idx), _)| {
            (
                (var_name.clone(), var_idx.clone()),
                lookups.var_map.get(var_name.as_str()).unwrap().clone(),
            )
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct Bounds {
    pub op: BoundsOp,
    pub val: Option<f64>,
}

impl Bounds {
    pub fn from_gmpl_bounds(bounds: Option<gmpl::VarBounds>) -> Self {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundsOp {
    FR,
    LO,
    UP,
    FX,
}

impl BoundsOp {
    pub fn from_rel_op(op: &gmpl::RelOp) -> Self {
        match op {
            gmpl::RelOp::Lt => panic!("Less than not supported"),
            gmpl::RelOp::Le => BoundsOp::UP,
            gmpl::RelOp::Eq => BoundsOp::FX,
            gmpl::RelOp::EqEq => BoundsOp::FX,
            gmpl::RelOp::Ne => panic!("Not equal not supported"),
            gmpl::RelOp::Ne2 => panic!("Not equal not supported"),
            gmpl::RelOp::Ge => BoundsOp::LO,
            gmpl::RelOp::Gt => panic!("Greater than not supported"),
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
