use std::io::{BufWriter, Write};

use highs::{ColProblem, Row, Sense};
use indexmap::IndexMap;

use crate::{
    ir::{ObjSense, interner::intern_resolve, op::RowType, write_index_vals},
    matrix::{Compiled, ConId, VarId},
};

pub fn highs_solve(compiled: Compiled) {
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
        cols.push(var_id);
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
        pb.add_column(col_factor, range, &row_factors);
    }

    let highs_sense = match sense {
        ObjSense::Minimize => Sense::Minimise,
        ObjSense::Maximize => Sense::Maximise,
    };
    let solved_model = pb.optimise(highs_sense).solve();
    let objective_value = solved_model.objective_value();
    let solution = solved_model.get_solution();

    let _ = writeln!(w, "OBJECTIVE VALUE\n{objective_value:.3}");
    let _ = writeln!(w, "\nCONSTRAINTS");
    for (((spur, con_idx), value), marginal) in
        rows.keys().zip(solution.rows()).zip(solution.dual_rows())
    {
        let name = intern_resolve(*spur);
        let _ = write!(w, "{name}");
        write_index_vals(&mut w, con_idx);
        let _ = writeln!(w, " = {value:.3}; marginal = {marginal:.3}");
    }

    let _ = writeln!(w, "\nVARS");
    for (((spur, var_idx), value), marginal) in cols
        .iter()
        .zip(solution.columns())
        .zip(solution.dual_columns())
    {
        let name = intern_resolve(*spur);
        let _ = write!(w, "{name}");
        write_index_vals(&mut w, var_idx);
        let _ = writeln!(w, " = {value:.3}; marginal = {marginal:.3}");
    }
}
