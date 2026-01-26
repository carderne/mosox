use std::io::{BufWriter, Write};

use crate::{
    ir::{Index, resolve},
    matrix::{BoundsMap, ColsMap, Compiled, RowType, RowsMap, bound::BoundsOp},
};

pub fn print_mps(compiled: Compiled, model_name: &str) {
    let stdout = std::io::stdout();
    let mut w = BufWriter::with_capacity(256 * 1024, stdout.lock());

    writeln!(w, "NAME {model_name}").unwrap();
    write_rows(&mut w, &compiled.rows);
    write_cols(&mut w, compiled.cols);
    write_rhs(&mut w, &compiled.rows);
    write_bounds(&mut w, compiled.bounds);
    writeln!(w, "ENDATA").unwrap();
    // BufWriter flushes on drop
}

fn write_rows(w: &mut impl Write, rows: &RowsMap) {
    writeln!(w, "ROWS").unwrap();
    for ((name, idx), (dir, _)) in rows {
        let name = resolve(*name);
        write!(w, " {dir}  {name}").unwrap();
        write_index_vals(w, idx);
        writeln!(w).unwrap();
    }
}

fn write_cols(w: &mut impl Write, cols: ColsMap) {
    writeln!(w, "COLUMNS").unwrap();
    for ((var_name, var_index), con_map) in cols {
        let var_name = resolve(var_name);
        for ((con_name, con_index), val) in con_map {
            if val != 0.0 {
                let con_name = resolve(con_name);
                write!(w, " {var_name}").unwrap();
                write_index_vals(w, &var_index);
                write!(w, " {con_name}").unwrap();
                write_index_vals(w, &con_index);
                writeln!(w, " {val}").unwrap();
            }
        }
    }
}

fn write_rhs(w: &mut impl Write, rows: &RowsMap) {
    writeln!(w, "RHS").unwrap();
    for ((name, idx), (row_type, val)) in rows {
        // Skip N-type rows (objective function) - they should never have RHS
        if *row_type == RowType::N {
            continue;
        }
        // MPS format assumes RHS is 0 if not provided
        // NB: -0 and +0 are different values
        if *val != 0.0 {
            let name = resolve(*name);
            write!(w, " RHS1 {name}").unwrap();
            write_index_vals(w, idx);
            writeln!(w, " {val}").unwrap();
        }
    }
}

fn write_bounds(w: &mut impl Write, bounds: BoundsMap) {
    writeln!(w, "BOUNDS").unwrap();

    for ((var_name, var_idx), bounds) in bounds {
        if bounds.op == BoundsOp::LO && bounds.val == Some(0.0) {
            // exclude vars with >= 0, as that is default in MPS
            continue;
        }

        let var_name = resolve(var_name);
        write!(w, " {} BND1 {var_name}", bounds.op).unwrap();
        write_index_vals(w, &var_idx);

        match bounds.val {
            Some(val) => writeln!(w, " {val}").unwrap(),
            None => writeln!(w).unwrap(),
        };
    }
}

/// Write index values directly to the buffer, avoiding String allocation
#[inline]
fn write_index_vals(w: &mut impl Write, v: &Index) {
    if !v.is_empty() {
        write!(w, "[").unwrap();
        let mut first = true;
        for item in v.iter() {
            if !first {
                write!(w, ",").unwrap();
            }
            first = false;
            write!(w, "{item}").unwrap();
        }
        write!(w, "]").unwrap();
    }
}
