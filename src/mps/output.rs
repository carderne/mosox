use crate::{
    gmpl::IndexVal,
    mps::{Bounds, Cols, Compiled, Rows, bounds::BoundsOp},
};

pub fn print_mps(compiled: Compiled) {
    println!("NAME          noname");
    print_rows(&compiled.rows);
    print_cols(compiled.cols);
    print_rhs(&compiled.rows);
    print_bounds(compiled.bounds);
    println!("ENDATA");
}

fn print_rows(rows: &Rows) {
    println!("ROWS");
    for ((name, idx), (dir, _)) in rows {
        let idx = format_index_vals(idx);
        println!(" {dir}  {name}{idx}")
    }
}

fn print_cols(cols: Cols) {
    println!("COLUMNS");
    for ((var_name, var_index), con_map) in cols {
        for ((con_name, con_index), val) in con_map {
            let var_idx = format_index_vals(&var_index);
            let con_idx = format_index_vals(&con_index);
            println!(
                "    {}{}      {}{}         {}",
                var_name, var_idx, con_name, con_idx, val
            );
        }
    }
}

fn print_rhs(rows: &Rows) {
    println!("RHS");
    for ((name, idx), (_, val)) in rows {
        if let Some(num) = val {
            let idx = format_index_vals(idx);
            println!("    RHS1      {name}{idx}       {num}");
        }
    }
}

fn print_bounds(bounds: Bounds) {
    println!("BOUNDS");

    for ((var_name, var_idx), bounds) in bounds {
        if bounds.op == BoundsOp::LO && bounds.val == Some(0.0) {
            // exclude vars with >= 0, as that is default in MPS
            continue;
        }

        let val = match bounds.val {
            Some(val) => val.to_string(),
            None => String::new(),
        };

        println!(
            " {} BND1     {}{}         {}",
            bounds.op,
            var_name,
            format_index_vals(&var_idx),
            val
        );
    }
}

fn format_index_vals(v: &[IndexVal]) -> String {
    if v.is_empty() {
        String::new()
    } else {
        let items: Vec<String> = v.iter().map(|s| s.to_string()).collect();
        format!("[{}]", items.join(","))
    }
}
