use std::collections::HashMap;
use std::fmt;

use crate::gmpl::{Constraint, DataParam, DataSet, Entry, Objective, Param, Set, Var};

/// A set declaration with optional data
#[derive(Clone, Debug)]
pub struct SetWithData {
    pub decl: Set,
    pub data: Vec<DataSet>,
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

#[derive(Clone, Debug)]
pub struct ModelWithData {
    pub sets: Vec<SetWithData>,
    pub vars: Vec<Var>,
    pub pars: Vec<ParamWithData>,
    pub objective: Objective,
    pub constraints: Vec<Constraint>,
}

impl fmt::Display for ModelWithData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", &self.objective)?;

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

        // Group data sets by name
        let mut data_set_map: HashMap<String, Vec<DataSet>> = HashMap::new();
        for data_set in data_sets {
            data_set_map
                .entry(data_set.name.clone())
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
            panic!("Data set '{}' has no matching model declaration", name);
        }

        let mut param_map: HashMap<String, Param> = HashMap::new();
        for param in params {
            param_map.insert(param.name.clone(), param);
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
                    data_param.name
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

        ModelWithData {
            objective: objective.unwrap(),
            sets: matched_sets,
            pars: matched_params,
            vars,
            constraints,
        }
    }
}
