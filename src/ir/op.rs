//! These operators are slightly further generalised from
//! the structs in ir/mod.rs.

use std::fmt;

use crate::ir::{self, RelOp};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Bounds {
    Fixed(f64),
    Range(f64, f64),
}

impl Bounds {
    pub fn from_gmpl_bounds(bounds: Vec<ir::VarBounds>) -> Self {
        if bounds.is_empty() {
            return Self::Range(f64::NEG_INFINITY, f64::INFINITY);
        };

        let mut lower: Option<f64> = None;
        let mut upper: Option<f64> = None;
        let mut fixed: Option<f64> = None;

        bounds.into_iter().for_each(|bound| match bound.op {
            ir::RelOp::Lt => panic!("Less than not supported"),
            ir::RelOp::Le => {
                upper = Some(bound.value);
            }
            ir::RelOp::Eq => {
                fixed = Some(bound.value);
            }
            ir::RelOp::EqEq => {
                fixed = Some(bound.value);
            }
            ir::RelOp::Ne => panic!("Not equal not supported"),
            ir::RelOp::Ne2 => panic!("Not equal not supported"),
            ir::RelOp::Ge => {
                lower = Some(bound.value);
            }
            ir::RelOp::Gt => panic!("Greater than not supported"),
        });

        if let Some(fixed) = fixed {
            if lower.is_some() || upper.is_some() {
                panic!("Cannot specify fixed and inequality var bounds");
            };
            Self::Fixed(fixed)
        } else {
            let lower = lower.unwrap_or(f64::NEG_INFINITY);
            let upper = upper.unwrap_or(f64::INFINITY);
            Self::Range(lower, upper)
        }
    }

    pub fn to_range(self) -> std::ops::RangeInclusive<f64> {
        match self {
            Self::Range(lower, upper) => lower..=upper,
            Self::Fixed(v) => v..=v,
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
