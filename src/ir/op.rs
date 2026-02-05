//! These operators are slightly further generalised from
//! the structs in ir/mod.rs.

use std::fmt;

use crate::ir::{self, RelOp};

#[derive(Copy, Clone, Debug)]
pub enum Bounds {
    Free,
    Lower(f64),
    Upper(f64),
    Fixed(f64),
}

impl Bounds {
    pub fn from_gmpl_bounds(bounds: Option<ir::VarBounds>) -> Self {
        match bounds {
            Some(bounds) => match bounds.op {
                ir::RelOp::Lt => panic!("Less than not supported"),
                ir::RelOp::Le => Self::Upper(bounds.value),
                ir::RelOp::Eq => Self::Fixed(bounds.value),
                ir::RelOp::EqEq => Self::Fixed(bounds.value),
                ir::RelOp::Ne => panic!("Not equal not supported"),
                ir::RelOp::Ne2 => panic!("Not equal not supported"),
                ir::RelOp::Ge => Self::Lower(bounds.value),
                ir::RelOp::Gt => panic!("Greater than not supported"),
            },
            None => Self::Free,
        }
    }

    pub fn to_range(self) -> std::ops::RangeInclusive<f64> {
        match self {
            Self::Free => f64::NEG_INFINITY..=f64::INFINITY,
            Self::Lower(v) => v..=f64::INFINITY,
            Self::Upper(v) => f64::NEG_INFINITY..=v,
            Self::Fixed(v) => v..=v,
        }
    }
}

impl fmt::Display for Bounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Free => write!(f, "FR"),
            Self::Lower(_) => write!(f, "LO"),
            Self::Upper(_) => write!(f, "UP"),
            Self::Fixed(_) => write!(f, "FX"),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RowType {
    LessThanOrEqual,
    Equal,
    GreaterThanOrEqual,
    /// Used for the objective function
    Unconstrained,
}

impl fmt::Display for RowType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RowType::LessThanOrEqual => write!(f, "L"),
            RowType::Equal => write!(f, "E"),
            RowType::GreaterThanOrEqual => write!(f, "G"),
            RowType::Unconstrained => write!(f, "N"),
        }
    }
}

impl RowType {
    pub fn from_rel_op(op: &RelOp) -> Self {
        match op {
            RelOp::Lt => panic!("Less than not supported"),
            RelOp::Le => RowType::LessThanOrEqual,
            RelOp::Eq => RowType::Equal,
            RelOp::EqEq => RowType::Equal,
            RelOp::Ne => panic!("Not equal not supported"),
            RelOp::Ne2 => panic!("Not equal not supported"),
            RelOp::Ge => RowType::GreaterThanOrEqual,
            RelOp::Gt => panic!("Greater than not supported"),
        }
    }

    pub fn to_range(self, rhs: f64) -> std::ops::RangeInclusive<f64> {
        match self {
            Self::LessThanOrEqual => f64::NEG_INFINITY..=rhs,
            Self::GreaterThanOrEqual => rhs..=f64::INFINITY,
            Self::Equal => rhs..=rhs,
            Self::Unconstrained => f64::NEG_INFINITY..=f64::INFINITY,
        }
    }
}
