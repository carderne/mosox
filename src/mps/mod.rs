use std::collections::HashMap;

use indexmap::IndexMap;

use crate::gmpl::atoms::{BoolOp, Domain, RelOp, VarSubscripted};
use crate::gmpl::{LogicExpr, ParamDataBody, ParamDataTarget, SetIndex};
use crate::model::ParamWithData;
use crate::{
    gmpl::{Constraint, Expr, atoms::MathOp},
    model::ModelWithData,
};
use itertools::Itertools;

type SetMap = IndexMap<String, Vec<SetIndex>>;
type IdxValMap = HashMap<String, SetIndex>;
type SetArr = Vec<Vec<SetIndex>>;
type VarMap = HashMap<String, bool>;
type ParamMap = HashMap<String, ParamCont>;
struct ParamCont {
    data: ParamArr,
    default: Option<Expr>,
}
enum ParamArr {
    Arr(HashMap<Vec<SetIndex>, f64>),
    Scalar(f64),
    Expr(Expr),
    None,
}
//                    var       var_index               con         con_index   val
type Cols = IndexMap<(String, Vec<SetIndex>), IndexMap<(String, Vec<SetIndex>), f64>>;
type RowMap = IndexMap<(String, Vec<SetIndex>), (String, Option<f64>)>;

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
        let idx_val_map: IdxValMap = HashMap::new();

        let pairs = recurse(
            model.objective.expr.clone(),
            &var_map,
            &param_map,
            &set_map,
            &idx_val_map,
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
        let con_indexes = domain_to_indexes(
            &constraint.domain,
            &var_map,
            &param_map,
            &set_map,
            &HashMap::new(),
        );

        for con_index in con_indexes {
            // There are three concepts in domain indexing.
            // SET (generically, dimension)
            // type: String
            // Upper-case name for a dimension
            // Eg: YEAR
            // (The set directive supplies all the values)
            //
            // INDEX (set_index, con_index etc)
            // type: String
            // A single index is a single lower-case letter
            // That a var/param/constraint uses to index a given set/dimension
            // Eg: y
            //
            // VALUE
            // type: SetIndex
            // represents a single actual value in this set/dimension
            // Eg: 2014
            //
            // con_index_vals stores the current LOCATION
            // as a dict like:
            // { y => 2014, r: "Africa" }
            //
            // This should be improved so that it also knows which set/dimension
            // each entry comes from...
            let idx_val_map: IdxValMap = constraint
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

            let built = build_constraint(constraint, &var_map, &param_map, &set_map, &idx_val_map);

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
    print_bounds(&model, &var_map, &param_map, &set_map);
    println!("ENDATA");
}

fn resolve_param_default(param: &ParamWithData) -> Option<Expr> {
    if let Some(data) = &param.data {
        if let Some(default) = data.default {
            return Some(Expr::Number(default));
        };
    } else if let Some(default) = &param.decl.default {
        return Some(default.clone());
    };

    None
}

fn resolve_param(param: ParamWithData) -> ParamCont {
    let default = resolve_param_default(&param);
    if let Some(data) = param.data
        && let Some(body) = data.body
    {
        match body {
            ParamDataBody::Num(num) => ParamCont {
                data: ParamArr::Scalar(num),
                default,
            },
            ParamDataBody::List(pairs) => {
                let mut arr: HashMap<Vec<SetIndex>, f64> = HashMap::new();
                for pair in pairs {
                    arr.insert(vec![pair.key], pair.value);
                }
                ParamCont {
                    data: ParamArr::Arr(arr),
                    default,
                }
            }
            ParamDataBody::Tables(tables) => {
                let mut arr: HashMap<Vec<SetIndex>, f64> = HashMap::new();
                for table in tables {
                    // Expressions like:
                    // [Atlantis_00A,NGCC,NOx,*,*]:
                    // Become prefixes for the indexes down below
                    // NOTE: Current implementation ONLY supports having exactly two * (Any)
                    // targets, and they must be the last two
                    let target_idxs: Vec<SetIndex> = match table.target {
                        Some(targets) => targets
                            .iter()
                            .filter_map(|t| match t {
                                ParamDataTarget::IndexVar(idx) => Some(idx.clone()),
                                ParamDataTarget::Any => None,
                            })
                            .collect(),
                        None => vec![],
                    };
                    for row in table.rows {
                        for (col, value) in table.cols.iter().zip(row.values.iter()) {
                            arr.insert(
                                [target_idxs.clone(), vec![row.label.clone(), col.clone()]]
                                    .concat(),
                                *value,
                            );
                        }
                    }
                }
                ParamCont {
                    data: ParamArr::Arr(arr),
                    default,
                }
            }
        }
    } else if let Some(expr) = param.decl.assign {
        ParamCont {
            data: ParamArr::Expr(expr),
            default,
        }
    } else {
        ParamCont {
            data: ParamArr::None,
            default,
        }
    }
}

fn build_constraint(
    constraint: &Constraint,
    var_map: &VarMap,
    param_map: &ParamMap,
    set_map: &SetMap,
    idx_val_map: &IdxValMap,
) -> BuiltConstraint {
    let lhs = recurse(
        constraint.constraint_expr.lhs.clone(),
        var_map,
        param_map,
        set_map,
        idx_val_map,
    );
    let rhs = recurse(
        constraint.constraint_expr.rhs.clone(),
        var_map,
        param_map,
        set_map,
        idx_val_map,
    );

    match (lhs, rhs) {
        (RecurseResult::Pairs(pairs), RecurseResult::Number(num)) => BuiltConstraint {
            pairs,
            rhs: Some(num),
        },
        (RecurseResult::Number(num), RecurseResult::Pairs(pairs)) => BuiltConstraint {
            pairs,
            rhs: Some(num),
        },
        (RecurseResult::Number(_), RecurseResult::Number(_)) => {
            panic!("constraint has f64 on both sides: {}", &constraint.name)
        }
        (RecurseResult::Pairs(lhs), RecurseResult::Pairs(rhs)) => {
            let rhs_neg: Vec<Pair> = rhs
                .iter()
                .map(|pair| Pair {
                    var: pair.var.clone(),
                    index: pair.index.clone(),
                    coeff: -pair.coeff,
                })
                .collect();

            let pairs = [lhs, rhs_neg].concat();
            BuiltConstraint {
                rhs: Some(0.0),
                pairs,
            }
        }
    }
}

fn recurse(
    expr: Expr,
    var_map: &VarMap,
    param_map: &ParamMap,
    set_map: &SetMap,
    idx_val_map: &IdxValMap,
) -> RecurseResult {
    match expr.clone() {
        Expr::Number(num) => RecurseResult::Number(num),
        Expr::VarSubscripted(var_or_param) => {
            let name = var_or_param.var.clone();

            let concrete: Option<Vec<SetIndex>> = if let Some(c) = var_or_param.concrete {
                // Already resolved by sum expansion
                Some(c.iter().map(|s| SetIndex::Str(s.clone())).collect())
            } else {
                var_or_param.subscript.as_ref().map(|subscript| {
                    subscript
                        .indices
                        .iter()
                        .map(|i| idx_val_map.get(&i.var).unwrap().clone())
                        .collect()
                })
            };

            if var_map.get(&name).is_some() {
                RecurseResult::Pairs(vec![Pair {
                    coeff: 1.0,
                    index: concrete,
                    var: name,
                }])
            } else if let Some(param) = param_map.get(&name) {
                match &param.data {
                    ParamArr::Scalar(num) => RecurseResult::Number(*num),
                    ParamArr::Arr(arr) => {
                        let arr_idx = concrete.expect("concrete is none");
                        if let Some(arr_val) = arr.get(&arr_idx) {
                            RecurseResult::Number(*arr_val)
                        } else {
                            match &param.default {
                                Some(expr) => {
                                    recurse(expr.clone(), var_map, param_map, set_map, idx_val_map)
                                }
                                None => panic!("tried to get uninitialized param: {}", &name),
                            }
                        }
                    }
                    ParamArr::Expr(expr) => {
                        recurse(expr.clone(), var_map, param_map, set_map, idx_val_map)
                    }
                    ParamArr::None => match &param.default {
                        Some(expr) => {
                            recurse(expr.clone(), var_map, param_map, set_map, idx_val_map)
                        }
                        None => panic!("tried to get uninitialized param: {}", &name),
                    },
                }
            } else if let Some(index_val) = idx_val_map.get(&name) {
                // Use the current index value (eg y=>2014) as an actual value
                // Mostly (only?) used in domain condition expressions
                match index_val {
                    SetIndex::Str(_) => panic!("cannot use a string SetIndex here"),
                    SetIndex::Int(num) => RecurseResult::Number(*num as f64),
                }
            } else {
                panic!(
                    "symbol does not point to a valid var or param. symbol: {} // constraint: {}",
                    &name, &expr,
                );
            }
        }
        Expr::FuncSum(func) => {
            let domain = func.domain;
            let operand = *func.operand;

            let new_expr = expand_sum(&operand, &domain, var_map, param_map, set_map, idx_val_map);
            recurse(new_expr, var_map, param_map, set_map, idx_val_map)
        }
        Expr::FuncMin(_) => panic!("not implemented: FuncMin"),
        Expr::FuncMax(_) => panic!("not implemented: FuncMax"),
        Expr::Conditional(_) => panic!("not implemented: Conditional"),
        Expr::UnaryNeg(_) => panic!("not implemented: UnaryNeg"),
        Expr::BinOp { lhs, op, rhs } => {
            let lhs = recurse(*lhs, var_map, param_map, set_map, idx_val_map);
            let rhs = recurse(*rhs, var_map, param_map, set_map, idx_val_map);

            let debug_msg = format!("unhandled case: lhs:{:?} rhs:{:?} op:{}", &lhs, &rhs, op);

            match (lhs, rhs, op) {
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Add) => {
                    RecurseResult::Number(l + r)
                }
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Sub) => {
                    RecurseResult::Number(l - r)
                }
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Mul) => {
                    RecurseResult::Number(l * r)
                }
                (RecurseResult::Number(l), RecurseResult::Number(r), MathOp::Div) => {
                    RecurseResult::Number(l / r)
                }
                (RecurseResult::Number(num), RecurseResult::Pairs(pairs), MathOp::Mul) => {
                    let res: Vec<Pair> = pairs
                        .iter()
                        .map(|p| Pair {
                            coeff: num * p.coeff,
                            index: p.index.clone(),
                            var: p.var.clone(),
                        })
                        .collect();
                    RecurseResult::Pairs(res)
                }
                (RecurseResult::Pairs(pairs), RecurseResult::Number(num), MathOp::Add) => {
                    if num == 0.0 {
                        RecurseResult::Pairs(pairs)
                    } else {
                        panic!("must handle adding const to pairs: {}", debug_msg);
                    }
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

fn print_bounds(model: &ModelWithData, var_map: &VarMap, param_map: &ParamMap, set_map: &SetMap) {
    println!("BOUNDS");

    for var in &model.vars {
        if let Some(bounds) = var.bounds.clone() {
            let indexes =
                domain_to_indexes(&var.domain, var_map, param_map, set_map, &HashMap::new());
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

fn domain_to_indexes(
    domain: &Option<Domain>,
    var_map: &VarMap,
    param_map: &ParamMap,
    set_map: &SetMap,
    idx_val_map: &IdxValMap,
) -> SetArr {
    match domain {
        None => vec![vec![]],
        Some(dom) => {
            let Domain { parts, condition } = dom;

            parts
                .iter()
                .map(|p| set_map.get(&p.set).unwrap().clone())
                .multi_cartesian_product()
                .filter_map(|idx| match condition {
                    None => Some(idx),
                    Some(logic) => {
                        let local_idx_map: IdxValMap = parts
                            .iter()
                            .zip(idx.iter())
                            .map(|(part, idx)| (part.var.clone(), idx.clone()))
                            .collect();
                        let merged_idx_map = {
                            let mut m = idx_val_map.clone();
                            m.extend(local_idx_map);
                            m
                        };

                        if check_domain_condition(
                            logic,
                            var_map,
                            param_map,
                            set_map,
                            &merged_idx_map,
                        ) {
                            Some(idx)
                        } else {
                            None
                        }
                    }
                })
                .collect()
        }
    }
}

fn check_domain_condition(
    logic: &LogicExpr,
    var_map: &VarMap,
    param_map: &ParamMap,
    set_map: &SetMap,
    idx_val_map: &IdxValMap,
) -> bool {
    match logic {
        LogicExpr::Comparison { lhs, op, rhs } => {
            let lhs = recurse(lhs.clone(), var_map, param_map, set_map, idx_val_map);
            let rhs = recurse(rhs.clone(), var_map, param_map, set_map, idx_val_map);

            match (lhs, rhs, op) {
                (RecurseResult::Number(lhs), RecurseResult::Number(rhs), RelOp::Eq) => lhs == rhs,
                (RecurseResult::Number(lhs), RecurseResult::Number(rhs), RelOp::Ne) => lhs != rhs,
                (RecurseResult::Number(lhs), RecurseResult::Number(rhs), RelOp::Gt) => lhs >= rhs,
                (RecurseResult::Number(lhs), RecurseResult::Number(rhs), RelOp::Ge) => lhs >= rhs,
                (RecurseResult::Number(lhs), RecurseResult::Number(rhs), RelOp::Lt) => lhs <= rhs,
                (RecurseResult::Number(lhs), RecurseResult::Number(rhs), RelOp::Le) => lhs <= rhs,
                _ => panic!("unhandled logic expr: {}", logic),
            }
        }
        LogicExpr::BoolOp { lhs, op, rhs } => {
            let lhs = check_domain_condition(lhs, var_map, param_map, set_map, idx_val_map);
            let rhs = check_domain_condition(rhs, var_map, param_map, set_map, idx_val_map);
            match op {
                BoolOp::And => lhs && rhs,
                BoolOp::Or => lhs || rhs,
            }
        }
    }
}

fn expand_sum(
    operand: &Expr,
    sum_domain: &Domain,
    var_map: &VarMap,
    param_map: &ParamMap,
    set_map: &SetMap,
    idx_val_map: &IdxValMap,
) -> Expr {
    let sum_indexes = domain_to_indexes(
        &Some(sum_domain.clone()),
        var_map,
        param_map,
        set_map,
        idx_val_map,
    );

    let substituted: Vec<Expr> = sum_indexes
        .iter()
        .map(|idx_combo| {
            let mut var_map = idx_val_map.clone();
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

fn substitute_vars(expr: &Expr, con_index_vals: &IdxValMap) -> Expr {
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
