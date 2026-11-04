mod dat;
mod table;

use std::collections::HashMap;

use anyhow::Result;
use lasso::Spur;

pub use dat::DatSource;
pub use table::TableSource;

use crate::ir::{self, DomainPartVar, Index, ParamVal, SetAtom, SetExpr, SetVals};

/// Values for one set: members per index of an indexed set (`[]` for a plain set)
pub type SetValues = HashMap<Index, SetVals>;

pub struct ParamValues {
    pub values: HashMap<Index, ParamVal>,
    pub default: Option<ParamVal>,
    /// Whether `values` update those of lower layers, rather than replacing them
    pub patch: bool,
}

impl ParamValues {
    /// These values layered over `lower`, whose default applies unless this gives one
    fn over(self, mut lower: ParamValues) -> ParamValues {
        let values = match self.patch {
            true => {
                lower.values.extend(self.values);
                lower.values
            }
            false => self.values,
        };
        ParamValues {
            values,
            default: self.default.or(lower.default),
            patch: self.patch && lower.patch,
        }
    }
}

/// Provides set and param data to a model.
///
/// The compiler asks for each declared set and param in turn, passing the declaration
/// (needed to interpret the data), and takes ownership of what's returned.
pub trait DataSource {
    /// Names of all sets and params with data, so undeclared data can be rejected
    fn names(&self) -> Vec<Spur>;
    fn take_set(&mut self, decl: &ir::Set, dimen: usize) -> Result<Option<SetValues>>;
    fn take_param(&mut self, decl: &ir::Param) -> Result<Option<ParamValues>>;
}

impl<T: DataSource + ?Sized> DataSource for Box<T> {
    fn names(&self) -> Vec<Spur> {
        (**self).names()
    }

    fn take_set(&mut self, decl: &ir::Set, dimen: usize) -> Result<Option<SetValues>> {
        (**self).take_set(decl, dimen)
    }

    fn take_param(&mut self, decl: &ir::Param) -> Result<Option<ParamValues>> {
        (**self).take_param(decl)
    }
}

/// Combines two sources, with `.0` layered over `.1`:
/// its sets replace those of `.1`, and its params replace or patch them
pub(crate) struct Layered<A, B>(pub A, pub B);

impl<A: DataSource, B: DataSource> DataSource for Layered<A, B> {
    fn names(&self) -> Vec<Spur> {
        let mut names = self.0.names();
        names.extend(self.1.names());
        names
    }

    fn take_set(&mut self, decl: &ir::Set, dimen: usize) -> Result<Option<SetValues>> {
        match self.0.take_set(decl, dimen)? {
            Some(values) => Ok(Some(values)),
            None => self.1.take_set(decl, dimen),
        }
    }

    fn take_param(&mut self, decl: &ir::Param) -> Result<Option<ParamValues>> {
        Ok(match (self.0.take_param(decl)?, self.1.take_param(decl)?) {
            (Some(top), Some(lower)) => Some(top.over(lower)),
            (top, lower) => top.or(lower),
        })
    }
}

/// Member dimension of a set: declared `dimen`, else implied by `within` or its expression.
pub(crate) fn set_dimen(decl: &ir::Set, sets: &HashMap<Spur, &ir::Set>) -> usize {
    if let Some(dimen) = decl.dimen {
        return dimen as usize;
    }
    if !decl.within.is_empty() {
        return decl.within.iter().map(|e| expr_dimen(e, sets)).sum();
    }
    decl.expr.as_ref().map_or(1, |e| expr_dimen(e, sets))
}

fn expr_dimen(expr: &SetExpr, sets: &HashMap<Spur, &ir::Set>) -> usize {
    match expr {
        SetExpr::InfixOp { lhs, .. } => expr_dimen(lhs, sets),
        SetExpr::Atom(SetAtom::Ref(r)) => sets.get(&r.spur).map_or(1, |s| set_dimen(s, sets)),
        SetExpr::Atom(SetAtom::Domain(d)) => d
            .parts
            .iter()
            .map(|p| match &p.var {
                DomainPartVar::Tuple(vars) => vars.len(),
                _ => expr_dimen(&p.expr, sets),
            })
            .sum(),
        SetExpr::Atom(SetAtom::SetOf(s)) => match &s.integrand {
            DomainPartVar::Tuple(vars) => vars.len(),
            _ => 1,
        },
        SetExpr::Atom(SetAtom::Literal(vals)) => vals.first().map_or(1, |v| match v {
            ir::SetVal::Tuple(t) => t.len(),
            _ => 1,
        }),
        SetExpr::Atom(SetAtom::Arith(_)) => 1,
    }
}

#[cfg(test)]
mod equivalence_test;
