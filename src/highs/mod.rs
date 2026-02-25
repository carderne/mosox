mod config;
pub mod format;
mod output;

use std::io::BufWriter;

use anyhow::Result;
use highs::{ColProblem, Row, Sense};
use indexmap::IndexMap;

use crate::{
    highs::{config::apply_options, format::Format, output::format_name},
    ir::{ObjSense, VarType, op::RowType},
    matrix::{Compiled, ConId, VarId},
};

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

pub fn highs_solve(compiled: Compiled, format: Format, config: &[(String, String)]) -> Result<()> {
    let stdout = std::io::stdout();
    let mut w = BufWriter::with_capacity(256 * 1024, stdout.lock());

    let Compiled { sense, vars, cons } = compiled;

    let mut pb = ColProblem::new();
    let mut rows: IndexMap<ConId, Row> = IndexMap::new();
    let mut cols: Vec<VarId> = vec![];
    let mut objective: Option<ConId> = None;
    let mut obj_offset: Option<f64> = None;

    for (con_id, row_type, rhs) in cons {
        if row_type == RowType::Unconstrained {
            // The only way we tell which function is the objective is that it is unconstrained...
            // Seems like this should be a bit more robust
            objective = Some(con_id);

            obj_offset = Some(rhs);
        } else {
            let range = row_type.to_range(rhs);
            let row = pb.add_row(range);
            rows.insert(con_id, row);
        }
    }

    let objective = objective.expect("no objective function founds");
    let obj_offset = obj_offset.expect("no objective function rhs found");

    // HiGHS doesn't support adding the constant part of the objective function to the equation.
    // So we store the value above and add it as a negated constant variable
    #[allow(clippy::needless_borrows_for_generic_args)]
    pb.add_column(-obj_offset, 1.0..=1.0, &[]);

    for (var_id, con_map) in vars {
        let range = con_map.bounds.to_range();
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

        match con_map.var_type {
            VarType::Float => {
                pb.add_column(col_factor, range, &row_factors);
            }
            VarType::Integer => {
                pb.add_integer_column(col_factor, range, &row_factors);
            }
            VarType::Binary => {
                pb.add_integer_column(col_factor, 0..=1, &row_factors);
            }
        };
    }

    let highs_sense = match sense {
        ObjSense::Minimize => Sense::Minimise,
        ObjSense::Maximize => Sense::Maximise,
    };
    let mut model = pb.optimise(highs_sense);
    apply_options(&mut model, config)?;

    let solved_model = model.solve();
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
    };
    Ok(())
}
