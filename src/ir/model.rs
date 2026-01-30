use std::collections::HashMap;
use std::fmt;

use lasso::Spur;
use smallvec::smallvec;

use crate::ir::{
    Constraint, ConstraintExpr, Domain, Entry, Expr, Objective, Param, ParamAssign, ParamData, Set,
    SetData, Var, intern_resolve, op::RowType,
};

/// A set declaration with optional data
#[derive(Clone, Debug)]
pub struct SetWithData {
    pub decl: Set,
    /// Sets with indices, eg TIMESLICE[y] will have multiple data entries,
    /// one for each y
    pub data: Vec<SetData>,
}

impl fmt::Display for SetWithData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.decl)?;
        for data in &self.data {
            write!(f, "\n  {}", data)?;
        }
        Ok(())
    }
}

/// A parameter declaration with optional data
#[derive(Clone, Debug)]
pub struct ParamWithData {
    pub decl: Param,
    pub data: Option<ParamData>,
}

impl fmt::Display for ParamWithData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.decl)?;
        if let Some(data) = &self.data {
            write!(f, "\n  {}", data)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ConstraintOrObjective {
    pub name: Spur,
    pub domain: Option<Domain>,
    pub row_type: RowType,
    pub lhs: Expr,
    pub rhs: Expr,
}

impl fmt::Display for ConstraintOrObjective {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "constraint {}", intern_resolve(self.name))?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        write!(f, ": {}", self.lhs)?;
        write!(f, ": {}", self.rhs)
    }
}

#[derive(Clone, Debug)]
pub struct ModelWithData {
    pub sets: Vec<SetWithData>,
    pub vars: Vec<Var>,
    pub pars: Vec<ParamWithData>,
    pub constraints: Vec<ConstraintOrObjective>,
}

impl fmt::Display for ModelWithData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for set in &self.sets {
            writeln!(f, "{}", set)?;
        }

        for param in &self.pars {
            writeln!(f, "{}", param)?;
        }

        for var in &self.vars {
            writeln!(f, "{}", var)?;
        }

        for constraint in &self.constraints {
            writeln!(f, "{}", constraint)?;
        }

        Ok(())
    }
}

impl ModelWithData {
    /// Build a ModelWithData from a list of entries, matching data to model statements
    pub fn from_entries(entries: Vec<Entry>) -> Self {
        let mut objective = None;
        let mut sets = Vec::new();
        let mut params = Vec::new();
        let mut vars = Vec::new();
        let mut constraints = Vec::new();
        let mut data_sets = Vec::new();
        let mut data_params = Vec::new();

        // First pass: separate model and data entries
        for entry in entries {
            match entry {
                Entry::Objective(obj) => {
                    if objective.is_some() {
                        panic!("Multiple objectives found");
                    }
                    objective = Some(obj);
                }
                Entry::Set(set) => sets.push(set),
                Entry::Param(param) => params.push(param),
                Entry::Var(var) => vars.push(var),
                Entry::Constraint(constraint) => constraints.push(constraint),
                Entry::DataSet(data_set) => data_sets.push(data_set),
                Entry::DataParam(data_param) => data_params.push(data_param),
            }
        }

        // Convert inline set data to SetData entries
        for set in &sets {
            if let Some(ref inline_data) = set.inline_data {
                data_sets.push(SetData {
                    name: set.name,
                    index: smallvec![],
                    values: inline_data.clone(),
                });
            }
        }

        // Convert inline param data to ParamData entries
        for param in &params {
            if let Some(ParamAssign::Data(ref body)) = param.assign {
                data_params.push(ParamData {
                    name: param.name,
                    default: None,
                    body: Some(body.clone()),
                });
            }
        }

        // Group data sets by name
        let mut data_set_map: HashMap<Spur, Vec<SetData>> = HashMap::new();
        for data_set in data_sets {
            data_set_map
                .entry(data_set.name)
                .or_default()
                .push(data_set);
        }

        // Match data sets to model sets
        let mut matched_sets = Vec::new();
        for set in sets {
            let data = data_set_map.remove(&set.name).unwrap_or_default();
            matched_sets.push(SetWithData { decl: set, data });
        }

        // Check for orphaned data sets
        if let Some((name, _)) = data_set_map.into_iter().next() {
            panic!(
                "Data set '{}' has no matching model declaration",
                intern_resolve(name)
            );
        }

        let mut param_map: HashMap<Spur, Param> = HashMap::new();
        for param in params {
            param_map.insert(param.name, param);
        }

        // Match data params to model params
        let mut matched_params = Vec::new();
        for data_param in data_params {
            if let Some(param_decl) = param_map.remove(&data_param.name) {
                matched_params.push(ParamWithData {
                    decl: param_decl,
                    data: Some(data_param),
                });
            } else {
                panic!(
                    "Data param '{}' has no matching model declaration",
                    intern_resolve(data_param.name)
                );
            }
        }

        // Add remaining params without data
        for (_, param_decl) in param_map {
            matched_params.push(ParamWithData {
                decl: param_decl,
                data: None,
            });
        }

        let all_constraints = prep_constraints(objective.unwrap(), constraints);

        ModelWithData {
            sets: matched_sets,
            pars: matched_params,
            vars,
            constraints: all_constraints,
        }
    }
}

fn prep_constraints(
    objective: Objective,
    constraints: Vec<Constraint>,
) -> Vec<ConstraintOrObjective> {
    let mut all: Vec<ConstraintOrObjective> = constraints
        .into_iter()
        .map(|Constraint { name, domain, expr }| {
            let ConstraintExpr { lhs, rhs, op } = expr;
            ConstraintOrObjective {
                name,
                domain,
                row_type: RowType::from_rel_op(&op),
                lhs,
                rhs,
            }
        })
        .collect();
    let Objective {
        name,
        expr,
        sense: _,
    } = objective;
    all.push(ConstraintOrObjective {
        name,
        domain: None,
        row_type: RowType::Unconstrained,
        lhs: expr,
        rhs: Expr::Number(0.0),
    });
    all
}
