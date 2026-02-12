use std::sync::Arc;

use anyhow::Result;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smallvec::SmallVec;

use crate::matrix::constraint::{check_logic_condition, domain_to_indexes, get_index_map};
use crate::{ir::Check, matrix::lookup::Lookups};

pub fn check_checks(checks: Vec<Check>, lookups: &Lookups) -> Result<()> {
    checks.into_par_iter().try_for_each(|check| {
        let Check {
            line_no,
            domain,
            expr,
        } = check;
        let (indexes, parts) = domain
            .map(|d| (domain_to_indexes(&d, lookups, &SmallVec::new()), d.parts))
            .unwrap_or_else(|| (vec![vec![].into()], vec![]));

        indexes.into_par_iter().try_for_each(|con_index| {
            let con_index = Arc::new(con_index);
            let idx_val_map = get_index_map(&parts, &con_index);
            if check_logic_condition(&expr, lookups, &idx_val_map) {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "Check condition failed at line {}",
                    line_no
                ))
            }
        })
    })
}
