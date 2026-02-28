use std::io::{BufWriter, Write};

use crate::matrix::{VarId, VarWithCoefficients};
use crate::mps::utils::write_index_vals;
use crate::{
    ir::{
        VarType,
        interner::intern_resolve,
        op::{Bounds, RowType},
    },
    matrix::{Compiled, ConsMap, VarsMap},
};

pub fn print_mps(compiled: Compiled, model_name: &str) {
    let stdout = std::io::stdout();
    let w = BufWriter::with_capacity(256 * 1024, stdout.lock());
    write_mps(compiled, model_name, w);
}

pub fn write_mps_to_file(
    compiled: Compiled,
    model_name: &str,
    path: &std::path::Path,
) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let w = BufWriter::with_capacity(256 * 1024, file);
    write_mps(compiled, model_name, w);
    Ok(())
}

fn write_mps(compiled: Compiled, model_name: &str, mut w: impl Write) {
    writeln!(w, "NAME {model_name}").unwrap();
    write_con_rows(&mut w, &compiled.cons);
    write_var_cols(&mut w, &compiled.vars);
    write_con_rhs(&mut w, &compiled.cons);
    write_var_bounds(&mut w, &compiled.vars);
    writeln!(w, "ENDATA").unwrap();
    // BufWriter flushes on drop
}

fn write_con_rows(w: &mut impl Write, rows: &ConsMap) {
    let mut idx_buf = String::with_capacity(128);
    writeln!(w, "ROWS").unwrap();
    for ((name, idx), row_type, _) in rows {
        let name = intern_resolve(*name);
        let idx_str = write_index_vals(&mut idx_buf, idx);
        writeln!(w, " {row_type}  {name}{idx_str}").unwrap();
    }
}

fn write_var_cols(w: &mut impl Write, cols: &VarsMap) {
    writeln!(w, "COLUMNS").unwrap();

    let mut cols_flt = vec![];
    let mut cols_int = vec![];
    cols.iter().for_each(|col| match col.1.var_type {
        VarType::Float => cols_flt.push(col),
        _ => cols_int.push(col),
    });

    write_col_lines(w, cols_flt);
    if cols_int.len() >= 1 {
        writeln!(w, " M0000001 'MARKER' 'INTORG'").unwrap();
        write_col_lines(w, cols_int);
        writeln!(w, " M0000001 'MARKER' 'INTEND'").unwrap();
    }
}

fn write_col_lines(w: &mut impl Write, cols: Vec<(&VarId, &VarWithCoefficients)>) {
    let mut var_idx_buf = String::with_capacity(128);
    let mut con_idx_buf = String::with_capacity(128);

    for ((var_name, var_index), con_map) in cols {
        let var_name = intern_resolve(*var_name);
        let var_idx_str = write_index_vals(&mut var_idx_buf, var_index);
        for ((con_name, con_index), val) in &con_map.coeffs {
            if *val != 0.0 {
                let con_name = intern_resolve(*con_name);
                let con_idx_str = write_index_vals(&mut con_idx_buf, con_index);
                writeln!(w, " {var_name}{var_idx_str} {con_name}{con_idx_str} {val}").unwrap();
            }
        }
    }
}

fn write_con_rhs(w: &mut impl Write, rows: &ConsMap) {
    let mut idx_buf = String::with_capacity(128);
    writeln!(w, "RHS").unwrap();
    for ((name, idx), row_type, val) in rows {
        // Skip N-type rows (objective function) - they should never have RHS
        if *row_type == RowType::Unconstrained {
            continue;
        }
        // MPS format assumes RHS is 0 if not provided
        // NB: -0 and +0 are different values
        if *val != 0.0 {
            let name = intern_resolve(*name);
            let idx_str = write_index_vals(&mut idx_buf, idx);
            writeln!(w, " RHS1 {name}{idx_str} {val}").unwrap();
        }
    }
}

fn write_var_bounds(w: &mut impl Write, vars: &VarsMap) {
    let mut idx_buf = String::with_capacity(128);
    writeln!(w, "BOUNDS").unwrap();

    for ((var_name, var_idx), var) in vars {
        let var_name = intern_resolve(*var_name);
        let idx_str = write_index_vals(&mut idx_buf, var_idx);
        let var_type = var.var_type;

        match var.bounds {
            Bounds::Fixed(val) => {
                writeln!(w, " FX BND1 {var_name}{idx_str} {val}").unwrap();
            }
            Bounds::Range(lower, upper) => {
                // Unconstrained (free) variable
                if lower == f64::NEG_INFINITY && upper == f64::INFINITY {
                    writeln!(w, " FR BND1 {var_name}{idx_str}").unwrap();
                } else {
                    // MPS lower bound is 0 by default, so we ignore that case
                    // different
                    if lower != 0.0 {
                        if lower == f64::NEG_INFINITY {
                            writeln!(w, " MI BND1 {var_name}{idx_str}").unwrap();
                        } else if var_type == VarType::Float {
                            writeln!(w, " LO BND1 {var_name}{idx_str} {lower}").unwrap();
                        } else {
                            writeln!(w, " LI BND1 {var_name}{idx_str} {lower}").unwrap();
                        }
                    }

                    if var_type == VarType::Float {
                        // MPS upper bound is +inf by default (so PL marker never used)
                        if upper != f64::INFINITY {
                            writeln!(w, " UP BND1 {var_name}{idx_str} {upper}").unwrap();
                        }
                    } else {
                        // upper bound is 1 by default for int/bin variables
                        if upper == f64::INFINITY {
                            writeln!(w, " PL BND1 {var_name}{idx_str}").unwrap();
                        } else {
                            writeln!(w, " UI BND1 {var_name}{idx_str} {upper}").unwrap();
                        }
                    }
                }
            }
        }
    }
}
