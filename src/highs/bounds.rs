use std::ops::RangeInclusive;

use anyhow::{Context, Result};

use crate::ir::op::Bounds;

pub fn bounds_vec_to_range(bounds: Vec<Bounds>) -> Result<RangeInclusive<f64>> {
    if bounds.is_empty() {
        Ok(f64::NEG_INFINITY..=f64::INFINITY)
    } else if bounds.len() == 1 {
        Ok(bounds[0].to_range())
    } else if bounds.len() == 2 {
        let mut lower: Option<f64> = None;
        let mut upper: Option<f64> = None;
        for bound in bounds {
            match bound {
                Bounds::Free => {}
                Bounds::Lower(val) => {
                    lower = Some(val);
                }
                Bounds::Upper(val) => {
                    upper = Some(val);
                }
                Bounds::Fixed(_) => {}
            }
        }

        let lower = lower.context("No lower bound for variable with two bounds")?;
        let upper = upper.context("No upper bound for variable with two bounds")?;
        Ok(lower..=upper)
    } else {
        Err(anyhow::anyhow!(
            "Cannot handle more than two bounds for variable"
        ))
    }
}
