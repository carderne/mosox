mod bounds;
mod output;

use std::fmt::Write as _;
use std::io::BufWriter;

use clap::ValueEnum;
use highs::{ColProblem, Row, Sense};
use indexmap::IndexMap;

use crate::{
    highs::bounds::bounds_vec_to_range,
    ir::{ObjSense, interner::intern_resolve, op::RowType},
    matrix::{Compiled, ConId, VarId},
};

#[derive(Clone, Debug, ValueEnum)]
pub enum Format {
    Txt,
    Csv,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Txt => write!(f, "txt"),
            Format::Csv => write!(f, "csv"),
        }
    }
}

pub struct SolutionRow {
    pub name: String,
    pub value: f64,
    pub marginal: f64,
}

pub struct SolutionData {
    pub objective_name: String,
    pub objective_value: f64,
    pub constraints: Vec<SolutionRow>,
    pub variables: Vec<SolutionRow>,
}

fn format_name(spur: lasso::Spur, idx: &crate::ir::Index) -> String {
    let name = intern_resolve(spur);
    let mut s = name.to_string();
    if !idx.is_empty() {
        s.push('[');
        let mut first = true;
        for item in idx.iter() {
            if !first {
                s.push(',');
            }
            first = false;
            write!(s, "{item}").unwrap();
        }
        s.push(']');
    }
    s
}

pub fn highs_solve(compiled: Compiled, format: Format) {
    let stdout = std::io::stdout();
    let mut w = BufWriter::with_capacity(256 * 1024, stdout.lock());

    let Compiled { sense, vars, cons } = compiled;

    let mut pb = ColProblem::new();
    let mut rows: IndexMap<ConId, Row> = IndexMap::new();
    let mut cols: Vec<VarId> = vec![];
    let mut objective: Option<ConId> = None;

    for (con_id, row_type, rhs) in cons {
        if row_type == RowType::Unconstrained {
            objective = Some(con_id);
        } else {
            let range = row_type.to_range(rhs);
            let row = pb.add_row(range);
            rows.insert(con_id, row);
        }
    }

    let objective = objective.expect("no objective function founds");

    for (var_id, con_map) in vars {
        let range = bounds_vec_to_range(con_map.bounds);
        let range = match range {
            Err(e) => {
                let var_name = intern_resolve(var_id.0);
                panic!("Failed to parse {var_name}: {e}");
            }
            Ok(r) => r,
        };
        let mut row_factors: Vec<(Row, f64)> = vec![];

        let mut col_factor: f64 = 0.;

        for (con_id, val) in &con_map.coeffs {
            if *con_id == objective {
                col_factor = *val;
            } else {
                let row = rows.get(con_id).expect("no row at con_id");
                row_factors.push((*row, *val));
            }
        }
        cols.push(var_id);
        pb.add_column(col_factor, range, &row_factors);
    }

    let highs_sense = match sense {
        ObjSense::Minimize => Sense::Minimise,
        ObjSense::Maximize => Sense::Maximise,
    };
    let solved_model = pb.optimise(highs_sense).solve();
    let objective_value = solved_model.objective_value();
    let solution = solved_model.get_solution();

    let objective_name = format_name(objective.0, &objective.1);

    let constraints: Vec<SolutionRow> = rows
        .keys()
        .zip(solution.rows())
        .zip(solution.dual_rows())
        .map(|(((spur, con_idx), value), marginal)| SolutionRow {
            name: format_name(*spur, con_idx),
            value: *value,
            marginal: *marginal,
        })
        .collect();

    let variables: Vec<SolutionRow> = cols
        .iter()
        .zip(solution.columns())
        .zip(solution.dual_columns())
        .map(|(((spur, var_idx), value), marginal)| SolutionRow {
            name: format_name(*spur, var_idx),
            value: *value,
            marginal: *marginal,
        })
        .collect();

    let data = SolutionData {
        objective_name,
        objective_value,
        constraints,
        variables,
    };

    match format {
        Format::Txt => output::write_txt(&mut w, &data),
        Format::Csv => output::write_csv(&mut w, &data),
    }
}
