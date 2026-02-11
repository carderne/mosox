use std::io::{BufWriter, Write};

use crate::{
    ir::{
        interner::intern_resolve,
        op::{Bounds, RowType},
        write_index_vals,
    },
    matrix::{Compiled, ConsMap, VarsMap},
};

pub fn print_mps(compiled: Compiled, model_name: &str) {
    let stdout = std::io::stdout();
    let mut w = BufWriter::with_capacity(256 * 1024, stdout.lock());

    writeln!(w, "NAME {model_name}").unwrap();
    write_con_rows(&mut w, &compiled.cons);
    write_var_cols(&mut w, &compiled.vars);
    write_con_rhs(&mut w, &compiled.cons);
    write_var_bounds(&mut w, &compiled.vars);
    writeln!(w, "ENDATA").unwrap();
    // BufWriter flushes on drop
}

fn write_con_rows(w: &mut impl Write, rows: &ConsMap) {
    writeln!(w, "ROWS").unwrap();
    for ((name, idx), dir, _) in rows {
        let name = intern_resolve(*name);
        write!(w, " {dir}  {name}").unwrap();
        write_index_vals(w, idx);
        writeln!(w).unwrap();
    }
}

fn write_var_cols(w: &mut impl Write, cols: &VarsMap) {
    writeln!(w, "COLUMNS").unwrap();
    for ((var_name, var_index), con_map) in cols {
        let var_name = intern_resolve(*var_name);
        for ((con_name, con_index), val) in &con_map.coeffs {
            if *val != 0.0 {
                let con_name = intern_resolve(*con_name);
                write!(w, " {var_name}").unwrap();
                write_index_vals(w, var_index);
                write!(w, " {con_name}").unwrap();
                write_index_vals(w, con_index);
                writeln!(w, " {val}").unwrap();
            }
        }
    }
}

fn write_con_rhs(w: &mut impl Write, rows: &ConsMap) {
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
            write!(w, " RHS1 {name}").unwrap();
            write_index_vals(w, idx);
            writeln!(w, " {val}").unwrap();
        }
    }
}

fn write_var_bounds(w: &mut impl Write, vars: &VarsMap) {
    writeln!(w, "BOUNDS").unwrap();

    for ((var_name, var_idx), var) in vars {
        let var_name = intern_resolve(*var_name);
        if var.bounds.is_empty() {
            write!(w, " FR BND1 {var_name}").unwrap();
            write_index_vals(w, var_idx);
            writeln!(w).unwrap();
        } else {
            for bound in &var.bounds {
                if let Bounds::Lower(v) = bound
                    && *v == 0.0
                {
                    // exclude vars with >= 0, as that is default in MPS
                    continue;
                }

                write!(w, " {bound} BND1 {var_name}").unwrap();
                write_index_vals(w, var_idx);

                match bound {
                    Bounds::Free => writeln!(w).unwrap(),
                    Bounds::Lower(v) | Bounds::Upper(v) | Bounds::Fixed(v) => {
                        writeln!(w, " {v}").unwrap()
                    }
                }
            }
        }
    }
}
