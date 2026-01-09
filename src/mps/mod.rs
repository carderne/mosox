use std::collections::HashMap;

use crate::gmpl::atoms::RelOp;
use crate::gmpl::{DataParam, ParamDataBody};
use crate::{
    gmpl::{Constraint, Expr, atoms::MathOp},
    model::ModelWithData,
};

pub fn compile_mps(model: ModelWithData) {
    print_name();
    print_rows(&model);

    let var_map: HashMap<String, bool> = model
        .vars
        .clone()
        .into_iter()
        .map(|var| (var.name, true))
        .collect();

    let param_map: HashMap<String, f64> = model
        .params
        .clone()
        .into_iter()
        .map(|param| (param.decl.name, resolve_param(param.data.unwrap())))
        .collect();

    let _set_map: HashMap<String, Vec<String>> = model
        .sets
        .clone()
        .into_iter()
        .map(|set| (set.decl.name, vec![]))
        .collect();

    let mut constraints: Vec<MpsConstraint> = vec![];

    {
        let pairs = recurse(model.objective.expr.clone(), &var_map, &param_map);
        let pairs = match pairs {
            RecurseResult::Pairs(pairs) => pairs,
            _ => panic!("unhandled OBJ"),
        };

        constraints.push(MpsConstraint {
            name: model.objective.name.clone(),
            rhs: None,
            pairs,
        });
    }

    for constraint in &model.constraints {
        constraints.push(build_constraint(constraint, &var_map, &param_map));
    }

    let mut cols: HashMap<String, Vec<Pair>> =
        var_map.keys().map(|k| (k.clone(), Vec::new())).collect();

    for constraint in &constraints {
        for pair in &constraint.pairs {
            let var = pair.var.clone();
            let coeff = pair.coeff;
            cols.get_mut(&var).unwrap().push(Pair {
                var: constraint.name.clone(),
                coeff,
            });
        }
    }

    print_cols(cols);
    print_rhs(&constraints);
    print_bounds(&model);
    println!("ENDATA");
}

fn build_constraint(
    constraint: &Constraint,
    var_map: &HashMap<String, bool>,
    param_map: &HashMap<String, f64>,
) -> MpsConstraint {
    dbg!(&constraint);

    let lhs = recurse(constraint.constraint_expr.lhs.clone(), var_map, param_map);
    let rhs = recurse(constraint.constraint_expr.rhs.clone(), var_map, param_map);

    let pairs = match lhs {
        RecurseResult::Pairs(pairs) => pairs,
        _ => panic!("unhandled outer LHS"),
    };

    let rhs = match rhs {
        RecurseResult::Number(num) => num,
        _ => panic!("unhandled outer RHS"),
    };

    MpsConstraint {
        name: constraint.name.clone(),
        rhs: Some(rhs),
        pairs,
    }
}

fn recurse(
    expr: Expr,
    var_map: &HashMap<String, bool>,
    param_map: &HashMap<String, f64>,
) -> RecurseResult {
    match expr {
        Expr::Number(num) => RecurseResult::Number(num),
        Expr::VarSubscripted(var_or_param) => {
            let name = var_or_param.var;
            if var_map.get(&name).is_some() {
                return RecurseResult::Pair(Pair {
                    coeff: 1.0,
                    var: name,
                });
            }

            if let Some(param) = param_map.get(&name) {
                return RecurseResult::Number(*param);
            }

            panic!("symbol {} does not point to a valid var or param", &name);
        }
        Expr::FuncSum(_) => panic!("not implemented: FuncSum"),
        Expr::FuncMin(_) => panic!("not implemented: FuncMin"),
        Expr::FuncMax(_) => panic!("not implemented: FuncMax"),
        Expr::Conditional(_) => panic!("not implemented: Conditional"),
        Expr::UnaryNeg(_) => panic!("not implemented: UnaryNeg"),
        Expr::BinOp { lhs, op, rhs } => {
            let lhs = recurse(*lhs, var_map, param_map);
            let rhs = recurse(*rhs, var_map, param_map);

            let debug_msg = format!("unhandled case: lhs:{:?} rhs:{:?} op:{}", &lhs, &rhs, op);

            match (lhs, rhs, op) {
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Add) => {
                    RecurseResult::Number(l + r)
                }
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Sub) => {
                    RecurseResult::Number(l - r)
                }
                (RecurseResult::Number(l), RecurseResult::Pair(pair), MathOp::Mul) => {
                    let coeff = l * pair.coeff;
                    RecurseResult::Pair(Pair {
                        var: pair.var,
                        coeff,
                    })
                }
                (RecurseResult::Pair(l_pair), RecurseResult::Pair(r_pair), MathOp::Add) => {
                    RecurseResult::Pairs(vec![l_pair, r_pair])
                }
                _ => unreachable!("{}", debug_msg),
            }
        }
    }
}

// Structs
pub struct MpsConstraint {
    name: String,
    rhs: Option<f64>,
    pairs: Vec<Pair>,
}

#[derive(Clone, Debug)]
pub struct Pair {
    var: String,
    coeff: f64,
}

#[derive(Clone, Debug)]
pub enum RecurseResult {
    Number(f64),
    Pair(Pair),
    Pairs(Vec<Pair>),
}

// Utils
fn print_name() {
    println!("NAME          noname");
}

fn rel_op_to_row_type(op: &RelOp) -> String {
    match op {
        RelOp::Lt => panic!("Less than not supported"),
        RelOp::Le => "L".to_string(),
        RelOp::Eq => "E".to_string(),
        RelOp::EqEq => "E".to_string(),
        RelOp::Ne => panic!("Not equal not supported"),
        RelOp::Ne2 => panic!("Not equal not supported"),
        RelOp::Ge => "G".to_string(),
        RelOp::Gt => panic!("Greater than not supported"),
    }
}

fn rel_op_to_bounds(op: &RelOp) -> String {
    match op {
        RelOp::Lt => panic!("Less than not supported"),
        RelOp::Le => "UP".to_string(),
        RelOp::Eq => "FX".to_string(),
        RelOp::EqEq => "FX".to_string(),
        RelOp::Ne => panic!("Not equal not supported"),
        RelOp::Ne2 => panic!("Not equal not supported"),
        RelOp::Ge => "LO".to_string(),
        RelOp::Gt => panic!("Greater than not supported"),
    }
}

fn print_rows(model: &ModelWithData) {
    println!("ROWS");

    // Objective
    {
        let dir = "N";
        let name = model.objective.name.clone();
        println!(" {dir}  {name}");
    }

    for constraint in &model.constraints {
        let dir = rel_op_to_row_type(&constraint.constraint_expr.op);
        let name = constraint.name.clone();
        println!(" {dir}  {name}")
    }
}

fn print_cols(cols: HashMap<String, Vec<Pair>>) {
    println!("COLUMNS");
    for (var_name, pairs) in cols {
        for pair in pairs {
            let con_name = pair.var.clone();
            let coeff = pair.coeff;
            println!("    {}      {}         {}", var_name, con_name, coeff);
        }
    }
}

fn print_rhs(constraints: &Vec<MpsConstraint>) {
    println!("RHS");

    for constraint in constraints {
        if let Some(rhs) = constraint.rhs {
            let con_name = constraint.name.clone();
            println!("    RHS1      {}       {}", con_name, rhs);
        }
    }
}

fn resolve_param(param: DataParam) -> f64 {
    if let Some(body) = param.body {
        match body {
            ParamDataBody::Num(num) => num,
            ParamDataBody::List(_) => panic!("param data pairs not implemented"),
            ParamDataBody::Tables(_) => panic!("param data tables not implemented"),
        }
    } else if let Some(default) = param.default {
        default
    } else {
        panic!("param data body is incomplete");
    }
}

fn print_bounds(model: &ModelWithData) {
    println!("BOUNDS");

    for var in &model.vars {
        if let Some(bounds) = var.bounds.clone() {
            let dir = rel_op_to_bounds(&bounds.op);
            let name = var.name.clone();
            let val = bounds.value;
            println!(" {} BND1     {}         {}", dir, name, val);
        }
    }
}
