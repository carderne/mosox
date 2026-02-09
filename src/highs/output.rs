use std::io::Write;

use super::SolutionData;

pub fn write_txt(w: &mut impl Write, data: &SolutionData) {
    let _ = writeln!(w, "OBJECTIVE VALUE\n{:.3}", data.objective_value);

    let _ = writeln!(w, "\nCONSTRAINTS");
    for row in &data.constraints {
        let _ = writeln!(
            w,
            "{} = {:.3}; marginal = {:.3}",
            row.name, row.value, row.marginal
        );
    }

    let _ = writeln!(w, "\nVARS");
    for row in &data.variables {
        let _ = writeln!(
            w,
            "{} = {:.3}; marginal = {:.3}",
            row.name, row.value, row.marginal
        );
    }
}

pub fn write_csv(w: &mut impl Write, data: &SolutionData) {
    let _ = writeln!(w, "type,name,value,marginal");
    let _ = writeln!(
        w,
        "objective,{},{:.3},{:.3}",
        data.objective_name, data.objective_value, 0.0
    );
    for row in &data.constraints {
        let _ = writeln!(
            w,
            "constraint,{},{:.3},{:.3}",
            row.name, row.value, row.marginal
        );
    }
    for row in &data.variables {
        let _ = writeln!(w, "var,{},{:.3},{:.3}", row.name, row.value, row.marginal);
    }
}
