use std::fmt::Write as _;
use std::io::BufWriter;
use std::io::Write;

use crate::highs::format::Format;
use crate::ir::interner::intern_resolve;

use super::SolutionData;

pub fn write_solution(data: SolutionData, format: Format, output: &std::path::Path) {
    let file = std::fs::File::create(output).expect("cannot create output file");
    let mut w = BufWriter::with_capacity(256 * 1024, file);
    match format {
        Format::Txt => write_txt(&mut w, data),
        Format::Csv => write_csv(&mut w, data),
    };
}

fn write_txt(w: &mut impl Write, data: SolutionData) {
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

fn write_csv(w: &mut impl Write, data: SolutionData) {
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

pub fn format_name(spur: lasso::Spur, idx: &crate::ir::Index) -> String {
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
