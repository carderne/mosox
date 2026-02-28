//! These operators are slightly further generalised from
//! the structs in ir/mod.rs.

use std::fmt;

use anyhow::{Result, bail};

use crate::ir::{self, RelOp, VarType};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Bounds {
    Fixed(f64),
    Range(f64, f64),
}

impl Bounds {
    pub fn from_gmpl_bounds(var: ir::Var) -> Result<Self> {
        match var.var_type {
            VarType::Binary => Ok(Self::Range(0.0, 1.0)),
            _ => {
                if var.bounds.is_empty() {
                    return Ok(Self::Range(f64::NEG_INFINITY, f64::INFINITY));
                };

                let mut lower: Option<f64> = None;
                let mut upper: Option<f64> = None;
                let mut fixed: Option<f64> = None;

                for bound in var.bounds {
                    match bound.op {
                        ir::RelOp::Lt => bail!("less than not supported in var bounds"),
                        ir::RelOp::Le => upper = Some(bound.value),
                        ir::RelOp::Eq | ir::RelOp::EqEq => fixed = Some(bound.value),
                        ir::RelOp::Ne | ir::RelOp::Ne2 => {
                            bail!("not equal not supported in var bounds")
                        }
                        ir::RelOp::Ge => lower = Some(bound.value),
                        ir::RelOp::Gt => bail!("greater than not supported in var bounds"),
                    }
                }

                if let Some(fixed) = fixed {
                    if lower.is_some() || upper.is_some() {
                        bail!("cannot specify fixed and inequality var bounds");
                    }
                    Ok(Self::Fixed(fixed))
                } else {
                    let lower = lower.unwrap_or(f64::NEG_INFINITY);
                    let upper = upper.unwrap_or(f64::INFINITY);
                    Ok(Self::Range(lower, upper))
                }
            }
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

// TODO move to mps
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
    pub fn from_rel_op(op: &RelOp) -> Result<Self> {
        match op {
            RelOp::Lt => bail!("less than not supported in constraints"),
            RelOp::Le => Ok(RowType::LessThanOrEqual),
            RelOp::Eq | RelOp::EqEq => Ok(RowType::Equal),
            RelOp::Ne | RelOp::Ne2 => bail!("not equal not supported in constraints"),
            RelOp::Ge => Ok(RowType::GreaterThanOrEqual),
            RelOp::Gt => bail!("greater than not supported in constraints"),
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
