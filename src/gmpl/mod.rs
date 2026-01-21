pub mod atoms;
mod expr;

use std::fmt;

use pest::iterators::Pair;

use crate::grammar::Rule;
pub use atoms::{Domain, RelOp};
pub use expr::{Expr, LogicExpr};

// ==============================
// ROOT RULES
// ==============================

/// Variable declaration
#[derive(Clone, Debug)]
pub struct Var {
    pub name: String,
    pub domain: Option<Domain>,
    pub bounds: Option<VarBounds>,
    pub param_type: Option<ParamType>,
}

impl Var {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut domain = None;
        let mut bounds = None;
        let mut param_type = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = pair.as_str().to_string(),
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::var_bounds => bounds = Some(VarBounds::from_entry(pair)),
                Rule::param_type => param_type = Some(ParamType::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            name,
            domain,
            bounds,
            param_type,
        }
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "var {}", self.name)?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        if let Some(bounds) = &self.bounds {
            write!(f, " {}", bounds)?;
        }
        if let Some(ptype) = &self.param_type {
            write!(f, " {}", ptype)?;
        }
        Ok(())
    }
}

/// Parameter declaration
#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub domain: Option<Domain>,
    pub param_type: Option<ParamType>,
    pub conditions: Vec<ParamCondition>,
    pub param_in: Option<Expr>,
    pub default: Option<Expr>,
    pub assign: Option<Expr>,
}

impl Param {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut domain = None;
        let mut param_type = None;
        let mut conditions = Vec::new();
        let mut param_in = None;
        let mut default = None;
        let mut assign = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = pair.as_str().to_string(),
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::param_type => param_type = Some(ParamType::from_entry(pair)),
                Rule::param_condition => conditions.push(ParamCondition::from_entry(pair)),
                Rule::param_in => param_in = pair.into_inner().next().map(|p| Expr::from_entry(p)),
                Rule::param_default => {
                    if let Some(p) = pair
                        .into_inner()
                        .next()
                        // ignore symbolic string_literal -> only used for file path
                        .filter(|p| p.as_rule() == Rule::expr)
                    {
                        default = Some(Expr::from_entry(p));
                    }
                }
                Rule::param_assign => {
                    assign = pair.into_inner().next().map(|p| Expr::from_entry(p))
                }
                _ => {}
            }
        }

        Self {
            name,
            domain,
            param_type,
            conditions,
            param_in,
            default,
            assign,
        }
    }
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "param {}", self.name)?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        if let Some(ptype) = &self.param_type {
            write!(f, " {}", ptype)?;
        }
        if !self.conditions.is_empty() {
            write!(f, " <conditions>")?;
        }
        if self.param_in.is_some() {
            write!(f, " in <expr>")?;
        }
        if self.default.is_some() {
            write!(f, " default <expr>")?;
        }
        if self.assign.is_some() {
            write!(f, " := <expr>")?;
        }
        Ok(())
    }
}

/// Set declaration
#[derive(Clone, Debug)]
pub struct Set {
    pub name: String,
}

impl Set {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut pairs = entry.into_inner();
        let name = pairs.next().unwrap().as_str().to_string();
        Self { name }
    }
}

impl fmt::Display for Set {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "set {}", self.name)
    }
}

/// Objective function
#[derive(Clone, Debug)]
pub struct Objective {
    pub sense: ObjSense,
    pub name: String,
    pub expr: Expr,
}

impl Objective {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut sense = ObjSense::Minimize;
        let mut name = String::new();
        let mut expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::obj_sense => sense = ObjSense::from_entry(pair),
                Rule::id => name = pair.as_str().to_string(),
                Rule::expr => expr = Some(Expr::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            sense,
            name,
            expr: expr.unwrap(),
        }
    }
}

impl fmt::Display for Objective {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}: <expr>", self.sense, self.name)
    }
}

/// Constraint
#[derive(Clone, Debug)]
pub struct Constraint {
    pub name: String,
    pub domain: Option<Domain>,
    pub expr: ConstraintExpr,
}

impl Constraint {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut domain = None;
        let mut constraint_expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = pair.as_str().to_string(),
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::constraint_expr => constraint_expr = Some(ConstraintExpr::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            name,
            domain,
            expr: constraint_expr.unwrap(),
        }
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "constraint {}", self.name)?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        write!(f, ": {}", self.expr)
    }
}

/// Data set values
#[derive(Clone, Debug)]
pub struct DataSet {
    pub name: String,
    pub values: Vec<SetVal>,
}

impl DataSet {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut values = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = pair.as_str().to_string(),
                Rule::set_val => values.push(SetVal::from_entry(pair)),
                _ => {}
            }
        }

        Self { name, values }
    }
}

impl fmt::Display for DataSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "data: set {} := <{} values>",
            self.name,
            self.values.len()
        )
    }
}

/// Data parameter values
#[derive(Clone, Debug)]
pub struct ParamDataPair {
    pub key: SetVal,
    pub value: f64,
}

impl ParamDataPair {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut tokens = entry.into_inner();
        let key = tokens.next().unwrap().as_str();
        let key = key
            .parse::<u64>()
            .map(SetVal::Int)
            .unwrap_or_else(|_| SetVal::Str(key.to_string()));
        let value = tokens.next().unwrap().as_str().parse().unwrap();
        Self { key, value }
    }
}

#[derive(Clone, Debug)]
pub enum ParamDataBody {
    Tables(Vec<ParamDataTable>),
    List(Vec<ParamDataPair>),
    Num(f64),
}

#[derive(Clone, Debug)]
pub struct DataParam {
    pub name: String,
    pub default: Option<f64>,
    pub body: Option<ParamDataBody>,
}

impl DataParam {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut default = None;
        let mut body = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = pair.as_str().to_string(),
                Rule::param_data_default => default = Some(pair.as_str().parse().unwrap()),
                Rule::param_data_body => {
                    let mut inner_pairs = pair.into_inner();
                    let first = inner_pairs.next().unwrap();

                    body = Some(match first.as_rule() {
                        Rule::param_data_list => {
                            let pairs = first.into_inner().map(ParamDataPair::from_entry).collect();
                            ParamDataBody::List(pairs)
                        }
                        Rule::param_data_matrix => {
                            let mut tables = vec![ParamDataTable::from_entry(first)];
                            tables.extend(inner_pairs.map(ParamDataTable::from_entry));
                            ParamDataBody::Tables(tables)
                        }
                        Rule::param_data_scalar => {
                            let num: f64 = first.as_str().parse().unwrap();
                            ParamDataBody::Num(num)
                        }
                        _ => unreachable!(),
                    });
                }
                _ => {}
            }
        }

        Self {
            name,
            default,
            body,
        }
    }
}

impl fmt::Display for DataParam {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "data: param {}", self.name)?;
        if self.default.is_some() {
            write!(f, " default <value>")?;
        }
        match &self.body {
            Some(ParamDataBody::Tables(tables)) => {
                write!(f, " := <{} table(s)>", tables.len())?;
            }
            Some(ParamDataBody::List(pairs)) => {
                write!(f, " := <{} pair(s)>", pairs.len())?;
            }
            Some(ParamDataBody::Num(num)) => {
                write!(f, " := {}", num)?;
            }
            None => {}
        }
        Ok(())
    }
}

/// Constraint expression (e.g., "expr <= expr")
#[derive(Clone, Debug)]
pub struct ConstraintExpr {
    pub lhs: Expr,
    pub op: RelOp,
    pub rhs: Expr,
}

impl ConstraintExpr {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut pairs = entry.into_inner();
        let lhs = Expr::from_entry(pairs.next().unwrap());
        let op = RelOp::from_entry(pairs.next().unwrap());
        let rhs = Expr::from_entry(pairs.next().unwrap());

        Self { lhs, op, rhs }
    }
}

impl fmt::Display for ConstraintExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<expr> {} <expr>", self.op)
    }
}

/// Parameter type
#[derive(Clone, Debug)]
pub enum ParamType {
    Integer,
    Binary,
    Symbolic,
}

impl ParamType {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        match entry.as_str() {
            "integer" => ParamType::Integer,
            "binary" => ParamType::Binary,
            "symbolic" => ParamType::Symbolic,
            _ => ParamType::Integer,
        }
    }
}

impl fmt::Display for ParamType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParamType::Integer => write!(f, "integer"),
            ParamType::Binary => write!(f, "binary"),
            ParamType::Symbolic => write!(f, "symbolic"),
        }
    }
}

/// Objective sense
#[derive(Clone, Debug)]
pub enum ObjSense {
    Minimize,
    Maximize,
}

impl ObjSense {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        match entry.as_str() {
            "minimize" => ObjSense::Minimize,
            "maximize" => ObjSense::Maximize,
            _ => ObjSense::Minimize,
        }
    }
}

impl fmt::Display for ObjSense {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ObjSense::Minimize => write!(f, "minimize"),
            ObjSense::Maximize => write!(f, "maximize"),
        }
    }
}

/// Variable bounds
#[derive(Clone, Copy, Debug)]
pub struct VarBounds {
    pub op: RelOp,
    pub value: f64,
}

impl VarBounds {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut op = RelOp::Ge;
        let mut value = 0.0;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::rel_op => op = RelOp::from_entry(pair),
                Rule::number => value = pair.as_str().parse().unwrap_or(0.0),
                _ => {}
            }
        }

        Self { op, value }
    }
}

impl fmt::Display for VarBounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.op, self.value)
    }
}

/// Parameter condition
#[derive(Clone, Debug)]
pub struct ParamCondition {
    pub op: RelOp,
    pub value: Expr,
}

impl ParamCondition {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut op = RelOp::Ge;
        let mut value = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::rel_op => op = RelOp::from_entry(pair),
                Rule::expr => value = Some(Expr::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            op,
            value: value.unwrap(),
        }
    }
}

impl fmt::Display for ParamCondition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} <value>", self.op)
    }
}

/// Set val (identifier or positive integer)
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum SetVal {
    Str(String),
    Int(u64),
}

impl SetVal {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let inner = entry.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::id => SetVal::Str(inner.as_str().to_string()),
            Rule::int => SetVal::Int(inner.as_str().parse().unwrap_or(0)),
            _ => SetVal::Str(inner.as_str().to_string()),
        }
    }
}

impl Ord for SetVal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (SetVal::Int(a), SetVal::Int(b)) => a.cmp(b),
            (SetVal::Str(a), SetVal::Str(b)) => a.cmp(b),
            (SetVal::Int(_), SetVal::Str(_)) => std::cmp::Ordering::Less,
            (SetVal::Str(_), SetVal::Int(_)) => std::cmp::Ordering::Greater,
        }
    }
}

impl PartialOrd for SetVal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SetVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SetVal::Str(s) => write!(f, "{}", s),
            SetVal::Int(n) => write!(f, "{}", n),
        }
    }
}

/// Parameter data table
#[derive(Clone, Debug)]
pub struct ParamDataTable {
    pub target: Option<Vec<ParamDataTarget>>,
    pub cols: Vec<SetVal>,
    pub rows: Vec<ParamDataRow>,
}

impl ParamDataTable {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut target = None;
        let mut cols: Vec<SetVal> = Vec::new();
        let mut rows = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::param_data_target => {
                    let mut targets = Vec::new();
                    for inner in pair.into_inner() {
                        match inner.as_rule() {
                            Rule::index_var => {
                                let raw = inner.as_str();
                                let idx = raw
                                    .parse::<u64>()
                                    .map(SetVal::Int)
                                    .unwrap_or_else(|_| SetVal::Str(raw.to_string()));

                                targets.push(ParamDataTarget::IndexVar(idx))
                            }
                            Rule::param_data_any => targets.push(ParamDataTarget::Any),
                            _ => {}
                        }
                    }
                    target = Some(targets);
                }
                Rule::param_data_cols => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::set_val {
                            let raw = inner.as_str();
                            let col = raw
                                .parse::<u64>()
                                .map(SetVal::Int)
                                .unwrap_or_else(|_| SetVal::Str(raw.to_string()));
                            cols.push(col);
                        }
                    }
                }
                Rule::param_data_row => rows.push(ParamDataRow::from_entry(pair)),
                _ => {}
            }
        }

        Self { target, cols, rows }
    }
}

impl fmt::Display for ParamDataTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.target.is_some() {
            write!(f, " [<target>]")?;
        }
        write!(f, " {} cols, {} rows", self.cols.len(), self.rows.len())
    }
}

/// Parameter data target
#[derive(Clone, Debug)]
pub enum ParamDataTarget {
    IndexVar(SetVal),
    Any,
}

impl fmt::Display for ParamDataTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParamDataTarget::IndexVar(s) => write!(f, "{}", s),
            ParamDataTarget::Any => write!(f, "*"),
        }
    }
}

/// Parameter data row
#[derive(Clone, Debug)]
pub struct ParamDataRow {
    pub label: SetVal,
    pub values: Vec<f64>,
}

impl ParamDataRow {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut label: Option<SetVal> = None;
        let mut values = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::set_val => {
                    if label.is_none() {
                        let raw = pair.as_str();
                        label = Some(
                            raw.parse::<u64>()
                                .map(SetVal::Int)
                                .unwrap_or_else(|_| SetVal::Str(raw.to_string())),
                        );
                    }
                }
                Rule::param_data_row_vals => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::param_data_val {
                            values.push(inner.as_str().parse().unwrap_or(0.0));
                        }
                    }
                }
                Rule::param_data_val => values.push(pair.as_str().parse().unwrap_or(0.0)),
                _ => {}
            }
        }

        Self {
            label: label.unwrap(),
            values,
        }
    }
}

impl fmt::Display for ParamDataRow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} <{} values>", self.label, self.values.len())
    }
}

// ==============================
// ROOT ENTRY ENUM
// ==============================

/// Root entry type
#[derive(Clone, Debug)]
pub enum Entry {
    Var(Var),
    Param(Param),
    Set(Set),
    Objective(Objective),
    Constraint(Constraint),
    DataSet(DataSet),
    DataParam(DataParam),
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Entry::Var(v) => write!(f, "{}", v),
            Entry::Param(p) => write!(f, "{}", p),
            Entry::Set(s) => write!(f, "{}", s),
            Entry::Objective(o) => write!(f, "{}", o),
            Entry::Constraint(c) => write!(f, "{}", c),
            Entry::DataSet(ds) => write!(f, "{}", ds),
            Entry::DataParam(dp) => write!(f, "{}", dp),
        }
    }
}
