use std::collections::HashMap;
use std::fmt;

use crate::data::{Constraint, DataParam, DataSet, Entry, Objective, Param, Set, Var};

/// A set declaration with optional data
#[derive(Clone, Debug)]
pub struct SetWithData {
    pub decl: Set,
    pub data: Option<DataSet>,
}

impl fmt::Display for SetWithData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.decl)?;
        if let Some(data) = &self.data {
            write!(f, "\n  {}", data)?;
        }
        Ok(())
    }
}

/// A parameter declaration with optional data
#[derive(Clone, Debug)]
pub struct ParamWithData {
    pub decl: Param,
    pub data: Option<DataParam>,
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

/// Complete model with sorted and matched declarations
#[derive(Clone, Debug)]
pub struct ModelWithData {
    pub objective: Option<Objective>,
    pub sets: Vec<SetWithData>,
    pub params: Vec<ParamWithData>,
    pub vars: Vec<Var>,
    pub constraints: Vec<Constraint>,
}

impl fmt::Display for ModelWithData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(obj) = &self.objective {
            writeln!(f, "{}", obj)?;
        }

        for set in &self.sets {
            writeln!(f, "{}", set)?;
        }

        for param in &self.params {
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
    pub fn from_entries(entries: &[Entry]) -> Result<Self, String> {
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
                        return Err("Multiple objectives found".to_string());
                    }
                    objective = Some(obj.clone());
                }
                Entry::Set(set) => sets.push(set.clone()),
                Entry::Param(param) => params.push(param.clone()),
                Entry::Var(var) => vars.push(var.clone()),
                Entry::Constraint(constraint) => constraints.push(constraint.clone()),
                Entry::DataSet(data_set) => data_sets.push(data_set.clone()),
                Entry::DataParam(data_param) => data_params.push(data_param.clone()),
            }
        }

        // Sort model statements alphabetically by name
        sets.sort_by(|a, b| a.name.cmp(&b.name));
        params.sort_by(|a, b| a.name.cmp(&b.name));
        vars.sort_by(|a, b| a.name.cmp(&b.name));
        constraints.sort_by(|a, b| a.name.cmp(&b.name));

        // Create lookup maps for matching
        let mut set_map: HashMap<String, Set> = HashMap::new();
        for set in sets {
            set_map.insert(set.name.clone(), set);
        }

        let mut param_map: HashMap<String, Param> = HashMap::new();
        for param in params {
            param_map.insert(param.name.clone(), param);
        }

        // Match data sets to model sets
        let mut matched_sets = Vec::new();
        for data_set in data_sets {
            if let Some(set_decl) = set_map.remove(&data_set.name) {
                matched_sets.push(SetWithData {
                    decl: set_decl,
                    data: Some(data_set),
                });
            } else {
                return Err(format!(
                    "Data set '{}' has no matching model declaration",
                    data_set.name
                ));
            }
        }

        // Add remaining sets without data
        for (_, set_decl) in set_map {
            matched_sets.push(SetWithData {
                decl: set_decl,
                data: None,
            });
        }
        matched_sets.sort_by(|a, b| a.decl.name.cmp(&b.decl.name));

        // Match data params to model params
        let mut matched_params = Vec::new();
        for data_param in data_params {
            if let Some(param_decl) = param_map.remove(&data_param.name) {
                matched_params.push(ParamWithData {
                    decl: param_decl,
                    data: Some(data_param),
                });
            } else {
                return Err(format!(
                    "Data param '{}' has no matching model declaration",
                    data_param.name
                ));
            }
        }

        // Add remaining params without data
        for (_, param_decl) in param_map {
            matched_params.push(ParamWithData {
                decl: param_decl,
                data: None,
            });
        }
        matched_params.sort_by(|a, b| a.decl.name.cmp(&b.decl.name));

        Ok(ModelWithData {
            objective,
            sets: matched_sets,
            params: matched_params,
            vars,
            constraints,
        })
    }
}
