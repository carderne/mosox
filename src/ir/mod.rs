pub(crate) mod interner;
pub(crate) mod model;
pub(crate) mod op;

use std::fmt;
use std::ops::Deref;
use std::sync::LazyLock;

use lasso::Spur;
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::pratt_parser::{Assoc::*, Op, PrattParser};
use smallvec::{SmallVec, smallvec};

use crate::gmpl::grammar::Rule;
use crate::ir::interner::{intern, intern_resolve};

static PRATT_PARSER: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    PrattParser::new()
        // Precedence lowest to highest (per GMPL spec)
        .op(Op::infix(Rule::add, Left) | Op::infix(Rule::sub, Left))
        .op(Op::prefix(Rule::sum_prefix)) // iterated ops: between add/sub and mul/div
        .op(Op::infix(Rule::mul, Left) | Op::infix(Rule::div, Left))
        .op(Op::prefix(Rule::neg))
        .op(Op::infix(Rule::pow, Right))
});

static LOGIC_PRATT: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    PrattParser::new()
        // Precedence: and > or (standard convention)
        .op(Op::infix(Rule::bool_or, Left))
        .op(Op::infix(Rule::bool_and, Left))
});

// ==============================
// ROOT RULES
// ==============================

/// Variable declaration
#[derive(Clone, Debug)]
pub struct Var {
    pub name: Spur,
    pub domain: Option<Domain>,
    pub bounds: Option<VarBounds>,
    pub param_type: Option<ParamType>,
}

impl Var {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name: Option<Spur> = None;
        let mut domain = None;
        let mut bounds = None;
        let mut param_type = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                // simple_domain is parsed but not used - var access uses index values from the set
                Rule::simple_domain => {}
                Rule::var_bounds => bounds = Some(VarBounds::from_entry(pair)),
                Rule::param_type => param_type = Some(ParamType::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            name: name.unwrap(),
            domain,
            bounds,
            param_type,
        }
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "var {}", intern_resolve(self.name))?;
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

/// Parameter assignment (expression or inline data)
#[derive(Clone, Debug)]
pub enum ParamAssign {
    Expr(Expr),
    Data(ParamDataBody),
}

/// Parameter declaration
#[derive(Clone, Debug)]
pub struct Param {
    pub name: Spur,
    pub domain: Option<Domain>,
    pub param_type: Option<ParamType>,
    pub conditions: Vec<ParamCondition>,
    pub param_in: Option<Expr>,
    pub default: Option<Expr>,
    pub assign: Option<ParamAssign>,
}

impl Param {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name: Option<Spur> = None;
        let mut domain = None;
        let mut param_type = None;
        let mut conditions = Vec::new();
        let mut param_in = None;
        let mut default = None;
        let mut assign = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                // simple_domain is parsed but not used - param access uses index values from the set
                Rule::simple_domain => {}
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
                    let inner = pair.into_inner().next().unwrap();
                    assign = Some(match inner.as_rule() {
                        Rule::expr => ParamAssign::Expr(Expr::from_entry(inner)),
                        Rule::param_data_body => ParamAssign::Data(parse_param_data_body(inner)),
                        _ => unreachable!("Unexpected rule in param_assign: {:?}", inner.as_rule()),
                    });
                }
                _ => {}
            }
        }

        Self {
            name: name.unwrap(),
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
        write!(f, "param {}", intern_resolve(self.name))?;
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
        match &self.assign {
            Some(ParamAssign::Expr(_)) => write!(f, " := <expr>")?,
            Some(ParamAssign::Data(_)) => write!(f, " := <data>")?,
            None => {}
        }
        Ok(())
    }
}

/// Set value (expression or inline data)
#[derive(Clone, Debug)]
pub enum SetValue {
    Expr(SetExpr),
    Vals(SetVals),
}

/// Set declaration
#[derive(Clone, Debug)]
pub struct Set {
    pub name: Spur,
    pub dims: Vec<SetDomainPart>,
    pub dimen: Option<u32>,
    pub within: Option<String>,
    pub cross: Option<String>,
    pub expr: Option<SetExpr>,
    pub inline_data: Option<SetVals>,
    pub default: Option<SetValue>,
}

impl Set {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name: Option<Spur> = None;
        let mut dims = Vec::new();
        let mut dimen = None;
        let mut within = None;
        let mut cross = None;
        let mut expr = None;
        let mut inline_data = None;
        let mut default = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::simple_domain => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::simple_domain_part {
                            dims.push(SetDomainPart::from_simple_domain_part(inner));
                        }
                    }
                }
                Rule::set_attrib => {
                    let inner = pair.into_inner().next().unwrap();
                    match inner.as_rule() {
                        Rule::set_dimen => {
                            let int_pair = inner.into_inner().next().unwrap();
                            dimen = Some(int_pair.as_str().parse().unwrap());
                        }
                        Rule::set_within => {
                            for p in inner.into_inner() {
                                match p.as_rule() {
                                    Rule::within_set => within = Some(p.as_str().to_string()),
                                    Rule::cross_set => cross = Some(p.as_str().to_string()),
                                    _ => {}
                                }
                            }
                        }
                        Rule::set_assign => {
                            let assign_inner = inner.into_inner().next().unwrap();
                            match assign_inner.as_rule() {
                                Rule::set_expr => expr = Some(SetExpr::from_entry(assign_inner)),
                                Rule::set_vals | Rule::set_tuples => {
                                    inline_data = Some(parse_set_vals_or_tuples(assign_inner));
                                }
                                _ => {}
                            }
                        }
                        Rule::set_default => {
                            let default_inner = inner.into_inner().next().unwrap();
                            default = Some(match default_inner.as_rule() {
                                Rule::set_expr => {
                                    SetValue::Expr(SetExpr::from_entry(default_inner))
                                }
                                _ => SetValue::Vals(parse_set_vals_or_tuples(default_inner)),
                            });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Self {
            name: name.unwrap(),
            dims,
            dimen,
            within,
            cross,
            expr,
            inline_data,
            default,
        }
    }
}

impl fmt::Display for Set {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "set {}", intern_resolve(self.name))
    }
}

#[derive(Clone, Debug)]
pub struct SetDomainPart {
    pub id: Option<Spur>,
    pub set: Spur,
}

impl SetDomainPart {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut id: Option<Spur> = None;
        let mut set: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => id = Some(intern(pair.as_str())),
                Rule::domain_set => set = Some(intern(pair.as_str())),
                _ => {}
            }
        }

        Self {
            id,
            set: set.unwrap(),
        }
    }

    /// Parse from simple_domain_part (used in SET declarations)
    /// Grammar: simple_domain_part = { (id ~ "in")? ~ domain_set }
    pub fn from_simple_domain_part(entry: Pair<Rule>) -> Self {
        let mut id: Option<Spur> = None;
        let mut set: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => {
                    // In simple_domain_part, id comes before "in" keyword
                    // If we already have an id, this shouldn't happen
                    id = Some(intern(pair.as_str()));
                }
                Rule::domain_set => set = Some(intern(pair.as_str())),
                _ => {}
            }
        }

        Self {
            id,
            set: set.unwrap(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SetExpr {
    Domain(Domain),
    SetMath(SetMath),
    SetOf(SetOf),
}

impl SetExpr {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let inner = entry.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::domain => SetExpr::Domain(Domain::from_entry(inner)),
            Rule::set_inter => SetExpr::SetMath(SetMath::from_entry(inner)),
            Rule::set_setof => SetExpr::SetOf(SetOf::from_entry(inner)),
            _ => unreachable!("Unexpected rule in set_expr: {:?}", inner.as_rule()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SetMath {
    pub intersection: Vec<VarSubscripted>,
}

impl SetMath {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let intersection = entry
            .into_inner()
            .filter(|p| p.as_rule() == Rule::var_subscripted)
            .map(VarSubscripted::from_entry)
            .collect();
        Self { intersection }
    }
}

#[derive(Clone, Debug)]
pub struct SetOf {
    pub domain: Domain,
    pub integrand: DomainPartVar,
}

impl SetOf {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut domain = None;
        let mut integrand = DomainPartVar::Single(intern(""));

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::domain_var => {
                    let inner = pair.into_inner().next().unwrap();
                    integrand = match inner.as_rule() {
                        Rule::domain_var_single => DomainPartVar::Single(intern(inner.as_str())),
                        Rule::domain_var_tuple => {
                            let ids: Vec<Spur> = inner
                                .into_inner()
                                .filter(|p| p.as_rule() == Rule::id)
                                .map(|p| intern(p.as_str()))
                                .collect();
                            DomainPartVar::Tuple(ids)
                        }
                        _ => unreachable!(),
                    };
                }
                _ => {}
            }
        }

        Self {
            domain: domain.unwrap(),
            integrand,
        }
    }
}

/// Objective function
#[derive(Clone, Debug)]
pub struct Objective {
    pub sense: ObjSense,
    pub name: Spur,
    pub expr: Expr,
}

impl Objective {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut sense = ObjSense::Minimize;
        let mut name: Option<Spur> = None;
        let mut expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::obj_sense => sense = ObjSense::from_entry(pair),
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::expr => expr = Some(Expr::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            sense,
            name: name.unwrap(),
            expr: expr.unwrap(),
        }
    }
}

impl fmt::Display for Objective {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}: <expr>", self.sense, intern_resolve(self.name))
    }
}

/// Constraint
#[derive(Clone, Debug)]
pub struct Constraint {
    pub name: Spur,
    pub domain: Option<Domain>,
    pub expr: ConstraintExpr,
}

impl Constraint {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name: Option<Spur> = None;
        let mut domain = None;
        let mut constraint_expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::constraint_expr => constraint_expr = Some(ConstraintExpr::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            name: name.unwrap(),
            domain,
            expr: constraint_expr.unwrap(),
        }
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "constraint {}", intern_resolve(self.name))?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        write!(f, ": {}", self.expr)
    }
}

/// Data set values
#[derive(Clone, Debug)]
pub struct SetData {
    pub name: Spur,
    pub index: Index,
    pub values: SetVals,
}

impl SetData {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name: Option<Spur> = None;
        let mut index = smallvec![];
        let mut values = SetVals::default();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::index => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::set_val {
                            index.push(SetVal::from_entry(inner));
                        }
                    }
                }
                Rule::set_data_assign => {
                    values = parse_set_assign(pair);
                }
                _ => {}
            }
        }

        Self {
            name: name.unwrap(),
            index,
            values,
        }
    }
}

impl fmt::Display for SetData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "data: set {} := <{} values>",
            intern_resolve(self.name),
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
            .parse::<u32>()
            .map(SetVal::Int)
            .unwrap_or_else(|_| SetVal::Str(intern(key)));
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
pub struct ParamData {
    pub name: Spur,
    pub default: Option<f64>,
    pub body: Option<ParamDataBody>,
}

impl ParamData {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name: Option<Spur> = None;
        let mut default = None;
        let mut body = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::param_data_default => default = Some(pair.as_str().parse().unwrap()),
                Rule::param_data_body => {
                    body = Some(parse_param_data_body(pair));
                }
                _ => {}
            }
        }

        Self {
            name: name.unwrap(),
            default,
            body,
        }
    }
}

impl fmt::Display for ParamData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "data: param {}", intern_resolve(self.name))?;
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
                            Rule::set_val => {
                                targets.push(ParamDataTarget::IndexVar(SetVal::from_entry(inner)))
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
                                .parse::<u32>()
                                .map(SetVal::Int)
                                .unwrap_or_else(|_| SetVal::Str(intern(raw)));
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
                            raw.parse::<u32>()
                                .map(SetVal::Int)
                                .unwrap_or_else(|_| SetVal::Str(intern(raw))),
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
    DataSet(SetData),
    DataParam(ParamData),
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

/// Expression - recursive tree structure with proper operator precedence
#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    VarSubscripted(VarSubscripted),
    FuncSum(Box<FuncSum>),
    FuncMin(Box<FuncMin>),
    FuncMax(Box<FuncMax>),
    Conditional(Box<Conditional>),
    UnaryNeg(Box<Expr>),
    BinOp {
        lhs: Box<Expr>,
        op: MathOp,
        rhs: Box<Expr>,
    },
}

impl Expr {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        parse_expr(entry.into_inner())
    }
}

/// Parse expression using Pratt parser for correct precedence
pub fn parse_expr(pairs: Pairs<Rule>) -> Expr {
    PRATT_PARSER
        .map_primary(|primary| match primary.as_rule() {
            Rule::number => Expr::Number(primary.as_str().parse().unwrap_or(0.0)),
            Rule::var_subscripted => Expr::VarSubscripted(VarSubscripted::from_entry(primary)),
            Rule::func_min => Expr::FuncMin(Box::new(FuncMin::from_entry(primary))),
            Rule::func_max => Expr::FuncMax(Box::new(FuncMax::from_entry(primary))),
            Rule::conditional => Expr::Conditional(Box::new(Conditional::from_entry(primary))),
            Rule::expr => parse_expr(primary.into_inner()),
            rule => unreachable!("Expected primary, found {:?}", rule),
        })
        .map_prefix(|op, rhs| match op.as_rule() {
            Rule::neg => Expr::UnaryNeg(Box::new(rhs)),
            Rule::sum_prefix => {
                // Extract domain from sum_prefix
                let domain = op
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::domain)
                    .map(Domain::from_entry)
                    .expect("sum_prefix must have domain");
                Expr::FuncSum(Box::new(FuncSum {
                    domain,
                    operand: Box::new(rhs),
                }))
            }
            rule => unreachable!("Expected prefix op, found {:?}", rule),
        })
        .map_infix(|lhs, op, rhs| {
            let op = match op.as_rule() {
                Rule::add => MathOp::Add,
                Rule::sub => MathOp::Sub,
                Rule::mul => MathOp::Mul,
                Rule::div => MathOp::Div,
                Rule::pow => MathOp::Pow,
                rule => unreachable!("Expected infix op, found {:?}", rule),
            };
            Expr::BinOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            }
        })
        .parse(pairs)
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expr::Number(n) => write!(f, "{}", n),
            Expr::VarSubscripted(v) => write!(f, "{}", v),
            Expr::FuncSum(func) => write!(f, "{}", **func),
            Expr::FuncMin(func) => write!(f, "{}", **func),
            Expr::FuncMax(func) => write!(f, "{}", **func),
            Expr::Conditional(cond) => write!(f, "{}", **cond),
            Expr::UnaryNeg(e) => write!(f, "-{}", **e),
            Expr::BinOp { lhs, op, rhs } => write!(f, "({} {} {})", **lhs, op, **rhs),
        }
    }
}

/// Logical expression - recursive tree structure with proper operator precedence
#[derive(Clone, Debug)]
pub enum LogicExpr {
    Comparison {
        lhs: Expr,
        op: RelOp,
        rhs: Expr,
    },
    BoolOp {
        lhs: Box<LogicExpr>,
        op: BoolOp,
        rhs: Box<LogicExpr>,
    },
}

impl LogicExpr {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        parse_logic_expr(entry.into_inner())
    }
}

/// Parse logical expression using Pratt parser for correct precedence
fn parse_logic_expr(pairs: Pairs<Rule>) -> LogicExpr {
    LOGIC_PRATT
        .map_primary(|primary| match primary.as_rule() {
            Rule::comparison => {
                let mut inner = primary.into_inner();
                let lhs = Expr::from_entry(inner.next().unwrap());
                let op = RelOp::from_entry(inner.next().unwrap());
                let rhs = Expr::from_entry(inner.next().unwrap());
                LogicExpr::Comparison { lhs, op, rhs }
            }
            Rule::logic_compound => {
                let inner = primary.into_inner().next().unwrap();
                parse_logic_expr(inner.into_inner())
            }
            Rule::logic_expr => parse_logic_expr(primary.into_inner()),
            rule => unreachable!("Expected logic primary, found {:?}", rule),
        })
        .map_infix(|lhs, op, rhs| {
            let op = match op.as_rule() {
                Rule::bool_and => BoolOp::And,
                Rule::bool_or => BoolOp::Or,
                rule => unreachable!("Expected bool op, found {:?}", rule),
            };
            LogicExpr::BoolOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            }
        })
        .parse(pairs)
}

impl fmt::Display for LogicExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LogicExpr::Comparison { lhs, op, rhs } => write!(f, "({} {} {})", lhs, op, rhs),
            LogicExpr::BoolOp { lhs, op, rhs } => write!(f, "({} {} {})", lhs, op, rhs),
        }
    }
}

/// Conditional expression (if-then-else)
#[derive(Clone, Debug)]
pub struct Conditional {
    pub condition: LogicExpr,
    pub then_expr: Box<Expr>,
    pub else_expr: Option<Box<Expr>>,
}

impl Conditional {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut condition = None;
        let mut then_expr = None;
        let mut else_expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::logic_expr => condition = Some(LogicExpr::from_entry(pair)),
                Rule::expr => {
                    if then_expr.is_none() {
                        then_expr = Some(Box::new(Expr::from_entry(pair)));
                    } else {
                        else_expr = Some(Box::new(Expr::from_entry(pair)));
                    }
                }
                _ => {}
            }
        }

        Self {
            condition: condition.unwrap(),
            then_expr: then_expr.unwrap(),
            else_expr,
        }
    }
}

impl fmt::Display for Conditional {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "if {} then {}", self.condition, self.then_expr)?;
        if let Some(else_expr) = &self.else_expr {
            write!(f, " else {}", else_expr)?;
        }
        Ok(())
    }
}

// ==============================
// CHILD STRUCTS
// ==============================

/// Set val (identifier or positive integer)
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub enum SetVal {
    Str(Spur),
    Int(u32),
    // This will panic if there's a tuple with more than two elements
    Tuple([SetValTerminal; 2]),
}

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub enum SetValTerminal {
    Str(Spur),
    Int(u32),
}

impl SetVal {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let inner = entry.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::id => SetVal::Str(intern(inner.as_str())),
            Rule::int => SetVal::Int(inner.as_str().parse().unwrap_or(0)),
            _ => SetVal::Str(intern(inner.as_str())),
        }
    }
}

impl fmt::Display for SetVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SetVal::Str(s) => write!(f, "{}", intern_resolve(*s)),
            SetVal::Int(n) => write!(f, "{}", n),
            SetVal::Tuple([a, b]) => write!(f, "{},{}", a, b),
        }
    }
}

impl fmt::Display for SetValTerminal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SetValTerminal::Str(s) => write!(f, "{}", intern_resolve(*s)),
            SetValTerminal::Int(n) => write!(f, "{}", n),
        }
    }
}

/// Domain specification
#[derive(Clone, Debug)]
pub struct Domain {
    pub parts: Vec<DomainPart>,
    pub condition: Option<LogicExpr>,
}

impl Domain {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut parts = Vec::new();
        let mut condition = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain_part => parts.push(DomainPart::from_entry(pair)),
                Rule::logic_expr => condition = Some(LogicExpr::from_entry(pair)),
                _ => {}
            }
        }

        Self { parts, condition }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{")?;
        let vars: Vec<_> = self.parts.iter().map(|p| p.var.to_string()).collect();
        write!(f, "{}", vars.join(", "))?;
        if self.condition.is_some() {
            write!(f, ": <condition>")?;
        }
        write!(f, "}}")
    }
}

/// Single domain part (e.g., "r in REGION")
#[derive(Clone, Debug)]
pub struct DomainPart {
    pub var: DomainPartVar,
    pub set: Spur,
    pub subscript: Subscript,
}

impl DomainPart {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut var = DomainPartVar::Single(intern(""));
        let mut subscript = Subscript::default();
        let mut set: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain_var => {
                    let inner = pair.into_inner().next().unwrap();
                    var = match inner.as_rule() {
                        Rule::domain_var_single => DomainPartVar::Single(intern(inner.as_str())),
                        Rule::domain_var_tuple => {
                            let ids: Vec<Spur> = inner
                                .into_inner()
                                .filter(|p| p.as_rule() == Rule::id)
                                .map(|p| intern(p.as_str()))
                                .collect();
                            DomainPartVar::Tuple(ids)
                        }
                        _ => unreachable!(),
                    };
                }
                Rule::subscript => {
                    subscript = Subscript::from_entry(pair);
                }
                Rule::domain_set => set = Some(intern(pair.as_str())),
                _ => {}
            }
        }

        Self {
            var,
            subscript,
            set: set.unwrap(),
        }
    }
}

impl fmt::Display for DomainPart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} in {}", self.var, intern_resolve(self.set))
    }
}

#[derive(Clone, Debug)]
pub enum DomainPartVar {
    Single(Spur),
    Tuple(Vec<Spur>),
}

impl fmt::Display for DomainPartVar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DomainPartVar::Single(s) => write!(f, "{}", intern_resolve(*s)),
            DomainPartVar::Tuple(v) => {
                let strs: Vec<&str> = v.iter().map(|s| intern_resolve(*s)).collect();
                write!(f, "({})", strs.join(", "))
            }
        }
    }
}

// ==============================
// ENUMS AND OPERATORS
// ==============================

/// Relational operator
#[derive(Clone, Copy, Debug)]
pub enum RelOp {
    Lt,
    Le,
    Eq,
    EqEq,
    Ne,
    Ne2,
    Ge,
    Gt,
}

impl RelOp {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        match entry.as_str() {
            "<" => RelOp::Lt,
            "<=" => RelOp::Le,
            "=" => RelOp::Eq,
            "==" => RelOp::EqEq,
            "<>" => RelOp::Ne,
            "!=" => RelOp::Ne2,
            ">=" => RelOp::Ge,
            ">" => RelOp::Gt,
            _ => RelOp::Eq,
        }
    }
}

impl fmt::Display for RelOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RelOp::Lt => write!(f, "<"),
            RelOp::Le => write!(f, "<="),
            RelOp::Eq => write!(f, "="),
            RelOp::EqEq => write!(f, "=="),
            RelOp::Ne => write!(f, "<>"),
            RelOp::Ne2 => write!(f, "!="),
            RelOp::Ge => write!(f, ">="),
            RelOp::Gt => write!(f, ">"),
        }
    }
}

/// Mathematical operator
#[derive(Clone, Copy, Debug)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

impl MathOp {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        match entry.as_str() {
            "+" => MathOp::Add,
            "-" => MathOp::Sub,
            "*" => MathOp::Mul,
            "/" => MathOp::Div,
            "^" => MathOp::Pow,
            _ => MathOp::Add,
        }
    }
}

impl fmt::Display for MathOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MathOp::Add => write!(f, "+"),
            MathOp::Sub => write!(f, "-"),
            MathOp::Mul => write!(f, "*"),
            MathOp::Div => write!(f, "/"),
            MathOp::Pow => write!(f, "^"),
        }
    }
}

/// Boolean operator
#[derive(Clone, Debug)]
pub enum BoolOp {
    And,
    Or,
}

impl BoolOp {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        match entry.as_str() {
            "and" | "&&" => BoolOp::And,
            "or" | "||" => BoolOp::Or,
            _ => BoolOp::And,
        }
    }
}

impl fmt::Display for BoolOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BoolOp::And => write!(f, "and"),
            BoolOp::Or => write!(f, "or"),
        }
    }
}

/// Variable with optional subscript
#[derive(Clone, Debug)]
pub struct VarSubscripted {
    pub var: Spur,
    pub subscript: Subscript,
}

impl VarSubscripted {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut var: Option<Spur> = None;
        let mut subscript = Subscript::default();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::var_ref => var = Some(intern(pair.as_str())),
                Rule::subscript => subscript = Subscript::from_entry(pair),
                _ => {}
            }
        }

        Self {
            var: var.unwrap(),
            subscript,
        }
    }
}

impl fmt::Display for VarSubscripted {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", intern_resolve(self.var))?;
        if !self.subscript.is_empty() {
            write!(f, "[...]")?;
        }
        Ok(())
    }
}

/// SetVals
#[derive(Clone, Debug, Eq, PartialEq, Hash, Default)]
#[repr(transparent)]
pub struct SetVals(pub Vec<SetVal>);

impl Deref for SetVals {
    type Target = Vec<SetVal>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<SetVal>> for SetVals {
    fn from(inner: Vec<SetVal>) -> Self {
        SetVals(inner)
    }
}

/// Index
pub type Index = SmallVec<[SetVal; 6]>;

/// Parse set_vals or set_tuples directly into SetVals
fn parse_set_vals_or_tuples(pair: Pair<Rule>) -> SetVals {
    let mut values = Vec::new();

    match pair.as_rule() {
        Rule::set_vals => {
            for val in pair.into_inner() {
                if val.as_rule() == Rule::set_val {
                    values.push(SetVal::from_entry(val));
                }
            }
        }
        Rule::set_tuples => {
            for tuple in pair.into_inner() {
                if tuple.as_rule() == Rule::set_tuple {
                    let tuple_vals: Vec<SetValTerminal> = tuple
                        .into_inner()
                        .filter(|p| p.as_rule() == Rule::set_val)
                        .map(|p| {
                            let inner = p.into_inner().next().unwrap();
                            match inner.as_rule() {
                                Rule::id => SetValTerminal::Str(intern(inner.as_str())),
                                Rule::int => {
                                    SetValTerminal::Int(inner.as_str().parse().unwrap_or(0))
                                }
                                _ => SetValTerminal::Str(intern(inner.as_str())),
                            }
                        })
                        .collect();
                    assert!(
                        tuple_vals.len() == 2,
                        "Only 2-element tuples supported, got {}",
                        tuple_vals.len()
                    );
                    values.push(SetVal::Tuple([tuple_vals[0], tuple_vals[1]]));
                }
            }
        }
        _ => {}
    }

    SetVals(values)
}

/// Parse set_assign (`:= ...`) into SetVals (used by SetData in data section)
fn parse_set_assign(pair: Pair<Rule>) -> SetVals {
    let inner = pair.into_inner().next().unwrap();
    parse_set_vals_or_tuples(inner)
}

/// Parse param_data_body into ParamDataBody (reused by Param and ParamData)
fn parse_param_data_body(pair: Pair<Rule>) -> ParamDataBody {
    let mut inner_pairs = pair.into_inner();
    let first = inner_pairs.next().unwrap();

    match first.as_rule() {
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
    }
}

/// Subscript (array indexing with optional shifts)
#[derive(Clone, Debug, Default)]
pub struct Subscript(pub Vec<SubscriptPart>);

impl Subscript {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let parts = entry
            .into_inner()
            .filter(|p| p.as_rule() == Rule::subscript_part)
            .map(SubscriptPart::from_entry)
            .collect();
        Self(parts)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &SubscriptPart> {
        self.0.iter()
    }
}

#[derive(Clone, Debug)]
pub struct SubscriptPart {
    pub var: Spur,
    pub shift: Option<SubscriptShift>,
}

impl SubscriptPart {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut var = intern("");
        let mut shift = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id | Rule::int => var = intern(pair.as_str()),
                Rule::subscript_shift => shift = Some(SubscriptShift::from_entry(pair)),
                _ => {}
            }
        }

        Self { var, shift }
    }
}

impl fmt::Display for SubscriptPart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", intern_resolve(self.var))?;
        if let Some(shift) = &self.shift {
            write!(f, "{}", shift)?;
        }
        Ok(())
    }
}

/// Subscript shift (+1 or -1)
#[derive(Clone, Debug)]
pub enum SubscriptShift {
    Plus,
    Minus,
}

impl SubscriptShift {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let s = entry.as_str();
        if s.starts_with('+') {
            SubscriptShift::Plus
        } else {
            SubscriptShift::Minus
        }
    }
}

impl fmt::Display for SubscriptShift {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SubscriptShift::Plus => write!(f, "+1"),
            SubscriptShift::Minus => write!(f, "-1"),
        }
    }
}

/// Sum function (iterated sum over a domain)
#[derive(Clone, Debug)]
pub struct FuncSum {
    pub domain: Domain,
    pub operand: Box<Expr>,
}

impl fmt::Display for FuncSum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "sum {} {}", self.domain, self.operand)
    }
}

/// Min function
#[derive(Clone, Debug)]
pub struct FuncMin {
    pub domain: Domain,
    pub var: Spur,
}

impl FuncMin {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut domain = None;
        let mut var: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::func_var => var = Some(intern(pair.as_str())),
                _ => {}
            }
        }
        let domain = domain.unwrap();

        Self {
            domain,
            var: var.unwrap(),
        }
    }
}

impl fmt::Display for FuncMin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "min <domain> min(<var>)")
    }
}

/// Max function
#[derive(Clone, Debug)]
pub struct FuncMax {
    pub domain: Domain,
    pub var: Spur,
}

impl FuncMax {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut domain = None;
        let mut var: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::func_var => var = Some(intern(pair.as_str())),
                _ => {}
            }
        }
        let domain = domain.unwrap();

        Self {
            domain,
            var: var.unwrap(),
        }
    }
}

impl fmt::Display for FuncMax {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "max <domain> max(<var>)")
    }
}
