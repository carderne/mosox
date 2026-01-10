use std::collections::HashMap;

use indexmap::IndexMap;

use crate::gmpl::atoms::{Domain, RelOp, VarSubscripted};
use crate::gmpl::{ParamDataBody, SetIndex};
use crate::model::ParamWithData;
use crate::{
    gmpl::{Constraint, Expr, atoms::MathOp},
    model::ModelWithData,
};
use itertools::Itertools;

type SetMap = IndexMap<String, Vec<SetIndex>>;
type SetValMap = HashMap<String, SetIndex>;
type SetArr = Vec<Vec<SetIndex>>;
type VarMap = HashMap<String, bool>;
type ParamMap = HashMap<String, ParamArr>;
enum ParamArr {
    Arr(HashMap<Vec<SetIndex>, f64>),
    Scalar(f64),
}
//                    var       var_index               con         con_index   val
type Cols = IndexMap<(String, Vec<SetIndex>), IndexMap<(String, Vec<SetIndex>), f64>>;
type RowMap = IndexMap<(String, Vec<SetIndex>), (String, Option<f64>)>;

// const SENTINEL: SetIndex = SetIndex::Int(u64::MAX);

pub fn compile_mps(model: ModelWithData) {
    let set_map: SetMap = model
        .sets
        .clone()
        .into_iter()
        .map(|set| (set.decl.name, set.data.unwrap().values))
        .collect();

    let var_map: VarMap = model
        .vars
        .clone()
        .into_iter()
        .map(|var| (var.name, true))
        .collect();

    let param_map: ParamMap = model
        .params
        .clone()
        .into_iter()
        .map(|param| (param.decl.name.clone(), resolve_param(param)))
        .collect();

    // Constraints
    let mut rows: RowMap = IndexMap::new();
    let mut cols: Cols = IndexMap::new();

    // First the objective alone
    {
        // Objective is always "singular"
        // it has no domain
        let index: SetValMap = HashMap::new();

        let pairs = recurse(
            model.objective.expr.clone(),
            &var_map,
            &param_map,
            &set_map,
            &index,
        );
        let pairs = match pairs {
            RecurseResult::Pairs(pairs) => pairs,
            _ => panic!("unhandled OBJ"),
        };
        for pair in &pairs {
            cols.entry((
                pair.var.clone(),
                pair.index.clone().unwrap_or_else(Vec::new),
            ))
            .or_default()
            .insert((model.objective.name.clone(), vec![]), pair.coeff);
        }
        rows.insert(
            (model.objective.name.clone(), vec![]),
            ("N".to_string(), None),
        );
    }

    // Then all the actual constraints
    for constraint in &model.constraints {
        let con_indexes = domain_to_indexes(&constraint.domain, &set_map);

        for con_index in con_indexes {
            let con_index_vals: SetValMap = constraint
                .clone()
                .domain
                .unwrap_or_else(|| Domain {
                    parts: vec![],
                    condition: None,
                })
                .parts
                .iter()
                .zip(con_index.iter())
                .map(|(part, idx)| (part.var.clone(), idx.clone()))
                .collect();

            let built =
                build_constraint(constraint, &var_map, &param_map, &set_map, &con_index_vals);

            let dir = rel_op_to_row_type(&constraint.constraint_expr.op);
            rows.insert(
                (constraint.name.clone(), con_index.clone()),
                (dir, built.rhs),
            );

            for pair in &built.pairs {
                cols.entry((
                    pair.var.clone(),
                    pair.index.clone().unwrap_or_else(Vec::new),
                ))
                .or_default()
                .insert((constraint.name.clone(), con_index.clone()), pair.coeff);
            }
        }
    }

    print_name();
    print_rows(&rows);
    print_cols(cols);
    print_rhs(&rows);
    print_bounds(&model, &set_map);
    println!("ENDATA");
}

fn resolve_param(param: ParamWithData) -> ParamArr {
    let data = param.data.unwrap();
    if let Some(body) = data.body {
        match body {
            ParamDataBody::Num(num) => ParamArr::Scalar(num),
            ParamDataBody::List(pairs) => {
                let mut arr: HashMap<Vec<SetIndex>, f64> = HashMap::new();
                for pair in pairs {
                    arr.insert(vec![pair.key], pair.value);
                }
                ParamArr::Arr(arr)
            }
            ParamDataBody::Tables(tables) => {
                let mut arr: HashMap<Vec<SetIndex>, f64> = HashMap::new();
                // TODO do something with paramdata.target
                for table in tables {
                    for row in table.rows {
                        for (col, value) in table.cols.iter().zip(row.values.iter()) {
                            arr.insert(vec![row.label.clone(), col.clone()], *value);
                        }
                    }
                }
                ParamArr::Arr(arr)
            }
        }
    } else if let Some(default) = data.default {
        ParamArr::Scalar(default)
    } else {
        panic!("param data body is incomplete");
    }
}

fn build_constraint(
    constraint: &Constraint,
    var_map: &VarMap,
    param_map: &ParamMap,
    set_map: &SetMap,
    index: &SetValMap,
) -> BuiltConstraint {
    let lhs = recurse(
        constraint.constraint_expr.lhs.clone(),
        var_map,
        param_map,
        set_map,
        index,
    );
    let rhs = recurse(
        constraint.constraint_expr.rhs.clone(),
        var_map,
        param_map,
        set_map,
        index,
    );

    let pairs = match lhs {
        RecurseResult::Pairs(pairs) => pairs,
        _ => panic!("unhandled outer LHS"),
    };

    let rhs = match rhs {
        RecurseResult::Number(num) => num,
        _ => panic!("unhandled outer RHS"),
    };

    BuiltConstraint {
        rhs: Some(rhs),
        pairs,
    }
}

fn recurse(
    expr: Expr,
    var_map: &VarMap,
    param_map: &ParamMap,
    set_map: &SetMap,
    index: &SetValMap,
) -> RecurseResult {
    match expr {
        Expr::Number(num) => RecurseResult::Number(num),
        Expr::VarSubscripted(var_or_param) => {
            let name = var_or_param.var;
            // let concrete: Option<Vec<SetIndex>> = var_or_param
            //     .concrete
            //     .map(|concrete| concrete.iter().map(|s| SetIndex::Str(s.clone())).collect());

            let concrete: Option<Vec<SetIndex>> = if let Some(c) = var_or_param.concrete {
                // Already resolved by sum expansion
                Some(c.iter().map(|s| SetIndex::Str(s.clone())).collect())
            } else {
                var_or_param.subscript.as_ref().map(|subscript| {
                    subscript
                        .indices
                        .iter()
                        .map(|i| {
                            index
                                .get(&i.var)
                                .unwrap_or_else(|| panic!("unbound variable: {}", i.var))
                                .clone()
                        })
                        .collect()
                })
            };

            if var_map.get(&name).is_some() {
                return RecurseResult::Pairs(vec![Pair {
                    coeff: 1.0,
                    index: concrete,
                    var: name,
                }]);
            }

            if let Some(param) = param_map.get(&name) {
                return match param {
                    ParamArr::Scalar(num) => RecurseResult::Number(*num),
                    ParamArr::Arr(arr) => {
                        RecurseResult::Number(*arr.get(&concrete.unwrap()).unwrap())
                    }
                };
            }

            panic!("symbol {} does not point to a valid var or param", &name);
        }
        Expr::FuncSum(func) => {
            let domain = func.domain;
            let operand = *func.operand;

            let new_expr = expand_sum(&operand, &domain, index, set_map);
            recurse(new_expr, var_map, param_map, set_map, index)
        }
        Expr::FuncMin(_) => panic!("not implemented: FuncMin"),
        Expr::FuncMax(_) => panic!("not implemented: FuncMax"),
        Expr::Conditional(_) => panic!("not implemented: Conditional"),
        Expr::UnaryNeg(_) => panic!("not implemented: UnaryNeg"),
        Expr::BinOp { lhs, op, rhs } => {
            let lhs = recurse(*lhs, var_map, param_map, set_map, index);
            let rhs = recurse(*rhs, var_map, param_map, set_map, index);

            let debug_msg = format!("unhandled case: lhs:{:?} rhs:{:?} op:{}", &lhs, &rhs, op);

            match (lhs, rhs, op) {
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Add) => {
                    RecurseResult::Number(l + r)
                }
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Sub) => {
                    RecurseResult::Number(l - r)
                }
                (RecurseResult::Number(l), RecurseResult::Pairs(pairs), MathOp::Mul) => {
                    let res: Vec<Pair> = pairs
                        .iter()
                        .map(|p| Pair {
                            coeff: l * p.coeff,
                            index: p.index.clone(),
                            var: p.var.clone(),
                        })
                        .collect();
                    RecurseResult::Pairs(res)
                }
                (RecurseResult::Pairs(l_pairs), RecurseResult::Pairs(r_pairs), MathOp::Add) => {
                    RecurseResult::Pairs([l_pairs, r_pairs].concat())
                }
                _ => unreachable!("{}", debug_msg),
            }
        }
    }
}

// Structs
pub struct BuiltConstraint {
    rhs: Option<f64>,
    pairs: Vec<Pair>,
}

#[derive(Clone, Debug)]
pub struct Pair {
    var: String,
    index: Option<Vec<SetIndex>>,
    coeff: f64,
}

#[derive(Clone, Debug)]
pub enum RecurseResult {
    Number(f64),
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

fn print_rows(rows: &RowMap) {
    println!("ROWS");
    for ((name, idx), (dir, _)) in rows {
        let idx = format_set_index(idx);
        println!(" {dir}  {name}{idx}")
    }
}

fn print_cols(cols: Cols) {
    println!("COLUMNS");
    for ((var_name, set_index), con_map) in cols {
        for ((con_name, con_index), val) in con_map {
            let var_idx = format_set_index(&set_index);
            let con_idx = format_set_index(&con_index);
            println!(
                "    {}{}      {}{}         {}",
                var_name, var_idx, con_name, con_idx, val
            );
        }
    }
}

fn print_rhs(rows: &RowMap) {
    println!("RHS");
    for ((name, idx), (_, val)) in rows {
        if let Some(num) = val {
            let idx = format_set_index(idx);
            println!("    RHS1      {name}{idx}       {num}");
        }
    }
}

fn print_bounds(model: &ModelWithData, set_map: &SetMap) {
    println!("BOUNDS");

    for var in &model.vars {
        if let Some(bounds) = var.bounds.clone() {
            let indexes = domain_to_indexes(&var.domain, set_map);
            let dir = rel_op_to_bounds(&bounds.op);
            let name = var.name.clone();
            let val = bounds.value;
            for index in indexes {
                let si = format_set_index(&index);
                println!(" {} BND1     {}{}         {}", dir, name, si, val);
            }
        }
    }
}

fn format_set_index(v: &[SetIndex]) -> String {
    if v.is_empty() {
        String::new()
    } else {
        let items: Vec<String> = v.iter().map(|s| s.to_string()).collect();
        format!("[{}]", items.join(","))
    }
}

fn domain_to_indexes(domain: &Option<Domain>, set_map: &SetMap) -> SetArr {
    match domain {
        None => vec![vec![]],
        Some(dom) => {
            let Domain { parts, condition } = dom;
            if condition.is_some() {
                panic!("domain conditions unhandled");
            }
            let sets: Vec<Vec<SetIndex>> = parts
                .iter()
                .map(|p| set_map.get(&p.set).unwrap().clone())
                .multi_cartesian_product()
                .collect();
            sets
        }
    }
}

fn expand_sum(
    operand: &Expr,
    sum_domain: &Domain,
    con_index_vals: &SetValMap, // bindings from constraint iteration
    set_map: &SetMap,
) -> Expr {
    let sum_indexes = domain_to_indexes(&Some(sum_domain.clone()), set_map);

    let substituted: Vec<Expr> = sum_indexes
        .iter()
        .map(|idx_combo| {
            let mut var_map = con_index_vals.clone();
            for (part, idx) in sum_domain.parts.iter().zip(idx_combo.iter()) {
                var_map.insert(part.var.clone(), idx.clone());
            }
            substitute_vars(operand, &var_map)
        })
        .collect();

    substituted
        .into_iter()
        .reduce(|acc, expr| Expr::BinOp {
            lhs: Box::new(acc),
            op: MathOp::Add,
            rhs: Box::new(expr),
        })
        .unwrap_or(Expr::Number(0.0))
}

fn substitute_vars(expr: &Expr, con_index_vals: &SetValMap) -> Expr {
    match expr {
        Expr::VarSubscripted(vs) => {
            if let Some(subscript) = &vs.subscript {
                let concrete: Vec<String> = subscript
                    .indices
                    .iter()
                    .map(|i| match con_index_vals.get(&i.var) {
                        Some(SetIndex::Str(s)) => s.clone(),
                        Some(SetIndex::Int(n)) => n.to_string(),
                        None => panic!("unbound variable: {}", i.var),
                    })
                    .collect();

                return Expr::VarSubscripted(VarSubscripted {
                    var: vs.var.clone(),
                    subscript: None,
                    concrete: Some(concrete),
                });
            }
            Expr::VarSubscripted(vs.clone())
        }
        Expr::BinOp { lhs, op, rhs } => Expr::BinOp {
            lhs: Box::new(substitute_vars(lhs, con_index_vals)),
            op: op.clone(),
            rhs: Box::new(substitute_vars(rhs, con_index_vals)),
        },
        Expr::Number(n) => Expr::Number(*n),
        Expr::UnaryNeg(inner) => Expr::UnaryNeg(Box::new(substitute_vars(inner, con_index_vals))),
        _ => todo!("handle other variants"),
    }
}
