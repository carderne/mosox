pub(crate) mod interner;
pub(crate) mod model;
pub(crate) mod op;

use std::fmt;
use std::ops::Deref;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail, ensure};
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
    pub bounds: Vec<VarBounds>,
    pub var_type: VarType,
}

impl Var {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut name: Option<Spur> = None;
        let mut domain = None;
        let mut bounds = Vec::new();
        let mut var_type = VarType::default();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::domain => domain = Some(Domain::from_entry(pair)?),
                Rule::var_attrib => {
                    for inner in pair.into_inner() {
                        match inner.as_rule() {
                            Rule::var_bounds => bounds.push(VarBounds::from_entry(inner)?),
                            Rule::var_type => var_type = VarType::from_entry(inner)?,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            name: name.context("missing var name")?,
            domain,
            bounds,
            var_type,
        })
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "var {}", intern_resolve(self.name))?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        for bounds in &self.bounds {
            write!(f, " {}", bounds)?;
        }
        if !matches!(self.var_type, VarType::Float) {
            write!(f, " {}", self.var_type)?;
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
    pub param_type: ParamType,
    pub conditions: Vec<ParamCondition>,
    pub param_in: Option<Expr>,
    pub default: Option<Expr>,
    pub assign: Option<ParamAssign>,
}

impl Param {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut name: Option<Spur> = None;
        let mut domain = None;
        let mut param_type = ParamType::default();
        let mut conditions = Vec::new();
        let mut param_in = None;
        let mut default = None;
        let mut assign = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::domain => domain = Some(Domain::from_entry(pair)?),
                Rule::param_type => param_type = ParamType::from_entry(pair)?,
                Rule::param_condition => conditions.push(ParamCondition::from_entry(pair)?),
                Rule::param_in => {
                    param_in = pair.into_inner().next().map(Expr::from_entry).transpose()?;
                }
                Rule::param_default => {
                    if let Some(p) = pair
                        .into_inner()
                        .next()
                        // ignore symbolic string_literal -> only used for file path
                        .filter(|p| p.as_rule() == Rule::expr)
                    {
                        default = Some(Expr::from_entry(p)?);
                    }
                }
                Rule::param_assign => {
                    let inner = pair.into_inner().next().context("empty param_assign")?;
                    assign = Some(match inner.as_rule() {
                        Rule::expr => ParamAssign::Expr(Expr::from_entry(inner)?),
                        Rule::param_data_body => ParamAssign::Data(parse_param_data_body(inner)?),
                        _ => bail!("Unexpected rule in param_assign: {:?}", inner.as_rule()),
                    });
                }
                _ => {}
            }
        }

        Ok(Self {
            name: name.context("missing param name")?,
            domain,
            param_type,
            conditions,
            param_in,
            default,
            assign,
        })
    }
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "param {}", intern_resolve(self.name))?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        if !matches!(self.param_type, ParamType::Float) {
            write!(f, " {}", self.param_type)?;
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
    pub domain: Domain,
    pub dimen: Option<u32>,
    pub within: Option<String>,
    pub cross: Option<String>,
    pub expr: Option<SetExpr>,
    pub inline_data: Option<SetVals>,
    pub default: Option<SetValue>,
}

impl Set {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut name: Option<Spur> = None;
        let mut domain = Domain::default();
        let mut dimen = None;
        let mut within = None;
        let mut cross = None;
        let mut expr = None;
        let mut inline_data = None;
        let mut default = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::domain => domain = Domain::from_entry(pair)?,
                Rule::set_attrib => {
                    let inner = pair.into_inner().next().context("empty set_attrib")?;
                    match inner.as_rule() {
                        Rule::set_dimen => {
                            let int_pair = inner.into_inner().next().context("empty set_dimen")?;
                            dimen = Some(int_pair.as_str().parse()?);
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
                            let assign_inner =
                                inner.into_inner().next().context("empty set_assign")?;
                            match assign_inner.as_rule() {
                                Rule::set_expr => expr = Some(SetExpr::from_entry(assign_inner)?),
                                Rule::set_vals | Rule::set_tuples => {
                                    inline_data = Some(parse_set_vals_or_tuples(assign_inner)?);
                                }
                                _ => {}
                            }
                        }
                        Rule::set_default => {
                            let default_inner =
                                inner.into_inner().next().context("empty set_default")?;
                            default = Some(match default_inner.as_rule() {
                                Rule::set_expr => {
                                    SetValue::Expr(SetExpr::from_entry(default_inner)?)
                                }
                                _ => SetValue::Vals(parse_set_vals_or_tuples(default_inner)?),
                            });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            name: name.context("missing set name")?,
            domain,
            dimen,
            within,
            cross,
            expr,
            inline_data,
            default,
        })
    }
}

impl fmt::Display for Set {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "set {}", intern_resolve(self.name))
    }
}

#[derive(Clone, Debug)]
pub enum SetExpr {
    Domain(Domain),
    SetMath(SetMath),
    SetOf(SetOf),
    Ref(SetRef),
}

impl SetExpr {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let inner = entry.into_inner().next().context("empty set_expr")?;
        Ok(match inner.as_rule() {
            Rule::domain => SetExpr::Domain(Domain::from_entry(inner)?),
            Rule::set_inter => SetExpr::SetMath(SetMath::from_entry(inner)?),
            Rule::set_setof => SetExpr::SetOf(SetOf::from_entry(inner)?),
            Rule::set_ref => SetExpr::Ref(SetRef::from_entry(inner)?),
            _ => bail!("Unexpected rule in set_expr: {:?}", inner.as_rule()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct SetRef {
    pub spur: Spur,
    pub index: Index,
}

impl SetRef {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut spur = None;
        let mut index = smallvec![];

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => spur = Some(intern(pair.as_str())),
                Rule::index => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::set_val {
                            index.push(SetVal::from_entry(inner)?);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            spur: spur.context("missing set ref id")?,
            index,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SetMath {
    pub intersection: Vec<VarSubscripted>,
}

impl SetMath {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let intersection = entry
            .into_inner()
            .filter(|p| p.as_rule() == Rule::var_subscripted)
            .map(VarSubscripted::from_entry)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { intersection })
    }
}

#[derive(Clone, Debug)]
pub struct SetOf {
    pub domain: Domain,
    pub integrand: DomainPartVar,
}

impl SetOf {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut domain = None;
        let mut integrand = DomainPartVar::Single(intern(""));

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)?),
                Rule::domain_var => {
                    let inner = pair.into_inner().next().context("empty domain_var")?;
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
                        _ => bail!("unexpected domain_var variant: {:?}", inner.as_rule()),
                    };
                }
                _ => {}
            }
        }

        Ok(Self {
            domain: domain.context("missing setof domain")?,
            integrand,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut sense = ObjSense::Minimize;
        let mut name: Option<Spur> = None;
        let mut expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::obj_sense => sense = ObjSense::from_entry(pair)?,
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::expr => expr = Some(Expr::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self {
            sense,
            name: name.context("missing objective name")?,
            expr: expr.context("missing objective expr")?,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut name: Option<Spur> = None;
        let mut domain = None;
        let mut constraint_expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = Some(intern(pair.as_str())),
                Rule::domain => domain = Some(Domain::from_entry(pair)?),
                Rule::constraint_expr => constraint_expr = Some(ConstraintExpr::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self {
            name: name.context("missing constraint name")?,
            domain,
            expr: constraint_expr.context("missing constraint expr")?,
        })
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

/// Check statement
#[derive(Clone, Debug)]
pub struct Check {
    pub line_no: i32,
    pub domain: Option<Domain>,
    pub expr: LogicExpr,
}

impl Check {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let (line_no, _) = entry.line_col();
        let mut domain = None;
        let mut expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)?),
                Rule::logic_expr => expr = Some(LogicExpr::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self {
            line_no: line_no as i32,
            domain,
            expr: expr.context("missing check expr")?,
        })
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "check")?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        write!(f, " {}", self.expr)
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut name: Option<Spur> = None;
        let mut index = smallvec![];
        let mut values = SetVals::default();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::index_data => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::set_val_data {
                            index.push(SetVal::from_entry(inner)?);
                        }
                    }
                }
                Rule::set_assign_data => {
                    values = parse_set_assign(pair)?;
                }
                _ => {}
            }
        }

        Ok(Self {
            name: name.context("missing set data name")?,
            index,
            values,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut tokens = entry.into_inner();
        let key = tokens.next().context("missing param data key")?.as_str();
        let key = key
            .parse::<u32>()
            .map(SetVal::Int)
            .unwrap_or_else(|_| SetVal::Str(intern(key)));
        let value: f64 = tokens
            .next()
            .context("missing param data value")?
            .as_str()
            .parse()?;
        Ok(Self { key, value })
    }
}

#[derive(Clone, Debug)]
pub enum ParamDataBody {
    Tables(Vec<ParamDataTable>),
    List(Vec<ParamDataPair>),
    Num(f64),
    Symbolic(String),
}

#[derive(Clone, Debug)]
pub struct ParamData {
    pub name: Spur,
    pub default: Option<f64>,
    pub body: Option<ParamDataBody>,
}

impl ParamData {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut name: Option<Spur> = None;
        let mut default = None;
        let mut body = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::param_data_default => default = Some(pair.as_str().parse()?),
                Rule::param_data_body => {
                    body = Some(parse_param_data_body(pair)?);
                }
                _ => {}
            }
        }

        Ok(Self {
            name: name.context("missing param data name")?,
            default,
            body,
        })
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
            Some(ParamDataBody::Symbolic(s)) => {
                write!(f, " := \"{}\"", s)?;
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut pairs = entry.into_inner();
        let lhs = Expr::from_entry(pairs.next().context("missing constraint lhs")?)?;
        let op = RelOp::from_entry(pairs.next().context("missing constraint op")?)?;
        let rhs = Expr::from_entry(pairs.next().context("missing constraint rhs")?)?;

        Ok(Self { lhs, op, rhs })
    }
}

impl fmt::Display for ConstraintExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<expr> {} <expr>", self.op)
    }
}

/// Variable type
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum VarType {
    #[default]
    Float,
    Integer,
    Binary,
}

impl VarType {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "integer" => VarType::Integer,
            "binary" => VarType::Binary,
            _ => VarType::Float,
        })
    }
}

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VarType::Float => write!(f, "float"),
            VarType::Integer => write!(f, "integer"),
            VarType::Binary => write!(f, "binary"),
        }
    }
}

/// Parameter type
#[derive(Clone, Debug, Default)]
pub enum ParamType {
    #[default]
    Float,
    Integer,
    Binary,
    Symbolic,
}

impl ParamType {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "integer" => ParamType::Integer,
            "binary" => ParamType::Binary,
            "symbolic" => ParamType::Symbolic,
            _ => ParamType::Float,
        })
    }
}

impl fmt::Display for ParamType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParamType::Float => write!(f, "float"),
            ParamType::Integer => write!(f, "integer"),
            ParamType::Binary => write!(f, "binary"),
            ParamType::Symbolic => write!(f, "symbolic"),
        }
    }
}

/// Objective sense
#[derive(Clone, Copy, Debug)]
pub enum ObjSense {
    Minimize,
    Maximize,
}

impl ObjSense {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "minimize" => ObjSense::Minimize,
            "maximize" => ObjSense::Maximize,
            _ => ObjSense::Minimize,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut op = RelOp::Ge;
        let mut value = 0.0;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::rel_op => op = RelOp::from_entry(pair)?,
                Rule::number => value = pair.as_str().parse().unwrap_or(0.0),
                _ => {}
            }
        }

        Ok(Self { op, value })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut op = RelOp::Ge;
        let mut value = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::rel_op => op = RelOp::from_entry(pair)?,
                Rule::expr => value = Some(Expr::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self {
            op,
            value: value.context("missing param condition value")?,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut target = None;
        let mut cols: Vec<SetVal> = Vec::new();
        let mut rows = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::param_data_target => {
                    let mut targets = Vec::new();
                    for inner in pair.into_inner() {
                        match inner.as_rule() {
                            Rule::set_val_data => {
                                targets.push(ParamDataTarget::IndexVar(SetVal::from_entry(inner)?))
                            }
                            Rule::param_data_any => targets.push(ParamDataTarget::Any),
                            _ => {}
                        }
                    }
                    target = Some(targets);
                }
                Rule::param_data_cols => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::set_val_data {
                            let raw = inner.as_str();
                            let col = raw
                                .parse::<u32>()
                                .map(SetVal::Int)
                                .unwrap_or_else(|_| SetVal::Str(intern(raw)));
                            cols.push(col);
                        }
                    }
                }
                Rule::param_data_row => rows.push(ParamDataRow::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self { target, cols, rows })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut label: Option<SetVal> = None;
        let mut values = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::set_val_data => {
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

        Ok(Self {
            label: label.context("missing param data row label")?,
            values,
        })
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
    Set(Box<Set>),
    Objective(Objective),
    Constraint(Constraint),
    Check(Check),
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
            Entry::Check(c) => write!(f, "{}", c),
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        parse_expr(entry.into_inner())
    }
}

/// Parse expression using Pratt parser for correct precedence
pub fn parse_expr(pairs: Pairs<Rule>) -> Result<Expr> {
    PRATT_PARSER
        .map_primary(|primary| match primary.as_rule() {
            Rule::number => Ok(Expr::Number(primary.as_str().parse().unwrap_or(0.0))),
            Rule::var_subscripted => Ok(Expr::VarSubscripted(VarSubscripted::from_entry(primary)?)),
            Rule::func_min => Ok(Expr::FuncMin(Box::new(FuncMin::from_entry(primary)?))),
            Rule::func_max => Ok(Expr::FuncMax(Box::new(FuncMax::from_entry(primary)?))),
            Rule::conditional => Ok(Expr::Conditional(Box::new(Conditional::from_entry(
                primary,
            )?))),
            Rule::expr => parse_expr(primary.into_inner()),
            rule => bail!("Expected primary, found {:?}", rule),
        })
        .map_prefix(|op, rhs| {
            let rhs = rhs?;
            match op.as_rule() {
                Rule::neg => Ok(Expr::UnaryNeg(Box::new(rhs))),
                Rule::sum_prefix => {
                    // Extract domain from sum_prefix
                    let domain = op
                        .into_inner()
                        .find(|p| p.as_rule() == Rule::domain)
                        .map(Domain::from_entry)
                        .transpose()?
                        .context("sum_prefix must have domain")?;
                    Ok(Expr::FuncSum(Box::new(FuncSum {
                        domain,
                        operand: Box::new(rhs),
                    })))
                }
                rule => bail!("Expected prefix op, found {:?}", rule),
            }
        })
        .map_infix(|lhs, op, rhs| {
            let lhs = lhs?;
            let rhs = rhs?;
            let op = match op.as_rule() {
                Rule::add => MathOp::Add,
                Rule::sub => MathOp::Sub,
                Rule::mul => MathOp::Mul,
                Rule::div => MathOp::Div,
                Rule::pow => MathOp::Pow,
                rule => bail!("Expected infix op, found {:?}", rule),
            };
            Ok(Expr::BinOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            })
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
    Membership {
        lhs: SetVals,
        op: MemberOp,
        rhs: Box<SetExpr>,
    },
    Subset {
        lhs: Box<SetExpr>,
        op: SubsetOp,
        rhs: Box<SetExpr>,
    },
    BoolOp {
        lhs: Box<LogicExpr>,
        op: BoolOp,
        rhs: Box<LogicExpr>,
    },
}

impl LogicExpr {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        parse_logic_expr(entry.into_inner())
    }
}

/// Parse logical expression using Pratt parser for correct precedence
fn parse_logic_expr(pairs: Pairs<Rule>) -> Result<LogicExpr> {
    LOGIC_PRATT
        .map_primary(|primary| match primary.as_rule() {
            Rule::logic_compare => {
                let mut inner = primary.into_inner();
                let lhs = Expr::from_entry(inner.next().context("missing logic compare lhs")?)?;
                let op = RelOp::from_entry(inner.next().context("missing logic compare op")?)?;
                let rhs = Expr::from_entry(inner.next().context("missing logic compare rhs")?)?;
                Ok(LogicExpr::Comparison { lhs, op, rhs })
            }
            Rule::logic_member => {
                let mut inner = primary.into_inner();
                let lhs =
                    parse_set_vals_or_tuples(inner.next().context("missing logic member lhs")?)?;
                let op = MemberOp::from_entry(inner.next().context("missing logic member op")?)?;
                let rhs = Box::new(SetExpr::from_entry(
                    inner.next().context("missing logic member rhs")?,
                )?);
                Ok(LogicExpr::Membership { lhs, op, rhs })
            }
            Rule::logic_subset => {
                let mut inner = primary.into_inner();
                let lhs = Box::new(SetExpr::from_entry(
                    inner.next().context("missing logic subset lhs")?,
                )?);
                let op = SubsetOp::from_entry(inner.next().context("missing logic subset op")?)?;
                let rhs = Box::new(SetExpr::from_entry(
                    inner.next().context("missing logic subset rhs")?,
                )?);
                Ok(LogicExpr::Subset { lhs, op, rhs })
            }
            Rule::logic_compound => {
                let inner = primary
                    .into_inner()
                    .next()
                    .context("empty logic_compound")?;
                parse_logic_expr(inner.into_inner())
            }
            Rule::logic_expr => parse_logic_expr(primary.into_inner()),
            rule => bail!("Expected logic primary, found {:?}", rule),
        })
        .map_infix(|lhs, op, rhs| {
            let lhs = lhs?;
            let rhs = rhs?;
            let op = match op.as_rule() {
                Rule::bool_and => BoolOp::And,
                Rule::bool_or => BoolOp::Or,
                rule => bail!("Expected bool op, found {:?}", rule),
            };
            Ok(LogicExpr::BoolOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            })
        })
        .parse(pairs)
}

impl fmt::Display for LogicExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LogicExpr::Comparison { lhs, op, rhs } => write!(f, "({} {} {})", lhs, op, rhs),
            LogicExpr::Membership { op, .. } => write!(f, "(<tuple> {} <set>)", op),
            LogicExpr::Subset { op, .. } => write!(f, "(<set> {} <set>)", op),
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut condition = None;
        let mut then_expr = None;
        let mut else_expr = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::logic_expr => condition = Some(LogicExpr::from_entry(pair)?),
                Rule::expr => {
                    if then_expr.is_none() {
                        then_expr = Some(Box::new(Expr::from_entry(pair)?));
                    } else {
                        else_expr = Some(Box::new(Expr::from_entry(pair)?));
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            condition: condition.context("missing conditional condition")?,
            then_expr: then_expr.context("missing conditional then_expr")?,
            else_expr,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let inner = entry.into_inner().next().context("empty set_val")?;
        Ok(match inner.as_rule() {
            Rule::id => SetVal::Str(intern(inner.as_str())),
            Rule::int => SetVal::Int(inner.as_str().parse().unwrap_or(0)),
            _ => SetVal::Str(intern(inner.as_str())),
        })
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
#[derive(Clone, Debug, Default)]
pub struct Domain {
    pub parts: Vec<DomainPart>,
    pub condition: Option<LogicExpr>,
}

impl Domain {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut parts = Vec::new();
        let mut condition = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain_part => parts.push(DomainPart::from_entry(pair)?),
                Rule::logic_expr => condition = Some(LogicExpr::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self { parts, condition })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut var = DomainPartVar::Single(intern(""));
        let mut subscript = Subscript::default();
        let mut set: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain_var => {
                    let inner = pair.into_inner().next().context("empty domain_var")?;
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
                        _ => bail!("unexpected domain_var variant: {:?}", inner.as_rule()),
                    };
                }
                Rule::subscript => {
                    subscript = Subscript::from_entry(pair)?;
                }
                Rule::domain_set => set = Some(intern(pair.as_str())),
                _ => {}
            }
        }

        Ok(Self {
            var,
            subscript,
            set: set.context("missing domain set")?,
        })
    }
}

impl fmt::Display for DomainPart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} in {}", self.var, intern_resolve(self.set))
    }
}

#[derive(Clone, Debug)]
pub enum DomainPartVar {
    None,
    Single(Spur),
    Tuple(Vec<Spur>),
}

impl fmt::Display for DomainPartVar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DomainPartVar::None => write!(f, ""),
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "<" => RelOp::Lt,
            "<=" => RelOp::Le,
            "=" => RelOp::Eq,
            "==" => RelOp::EqEq,
            "<>" => RelOp::Ne,
            "!=" => RelOp::Ne2,
            ">=" => RelOp::Ge,
            ">" => RelOp::Gt,
            _ => RelOp::Eq,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "+" => MathOp::Add,
            "-" => MathOp::Sub,
            "*" => MathOp::Mul,
            "/" => MathOp::Div,
            "^" => MathOp::Pow,
            _ => MathOp::Add,
        })
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

/// Membership operator (tuple in set)
#[derive(Clone, Copy, Debug)]
pub enum MemberOp {
    In,
    NotIn,
}

impl MemberOp {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "in" => MemberOp::In,
            "not in" | "!in" => MemberOp::NotIn,
            _ => bail!("Unexpected member_op: {}", entry.as_str()),
        })
    }
}

impl fmt::Display for MemberOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MemberOp::In => write!(f, "in"),
            MemberOp::NotIn => write!(f, "not in"),
        }
    }
}

/// Subset operator (set within set)
#[derive(Clone, Copy, Debug)]
pub enum SubsetOp {
    Within,
    NotWithin,
}

impl SubsetOp {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "within" => SubsetOp::Within,
            "not within" | "!within" => SubsetOp::NotWithin,
            _ => bail!("Unexpected subset_op: {}", entry.as_str()),
        })
    }
}

impl fmt::Display for SubsetOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SubsetOp::Within => write!(f, "within"),
            SubsetOp::NotWithin => write!(f, "not within"),
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        Ok(match entry.as_str() {
            "and" | "&&" => BoolOp::And,
            "or" | "||" => BoolOp::Or,
            _ => BoolOp::And,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut var: Option<Spur> = None;
        let mut subscript = Subscript::default();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::var_ref => var = Some(intern(pair.as_str())),
                Rule::subscript => subscript = Subscript::from_entry(pair)?,
                _ => {}
            }
        }

        Ok(Self {
            var: var.context("missing var ref")?,
            subscript,
        })
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
fn parse_set_vals_or_tuples(pair: Pair<Rule>) -> Result<SetVals> {
    let mut values = Vec::new();

    match pair.as_rule() {
        Rule::set_vals | Rule::set_vals_data => {
            for val in pair.into_inner() {
                if matches!(val.as_rule(), Rule::set_val | Rule::set_val_data) {
                    values.push(SetVal::from_entry(val)?);
                }
            }
        }
        Rule::set_tuples | Rule::set_tuples_data => {
            for tuple in pair.into_inner() {
                if matches!(tuple.as_rule(), Rule::set_tuple | Rule::set_tuple_data) {
                    let tuple_vals: Vec<SetValTerminal> = tuple
                        .into_inner()
                        .filter(|p| matches!(p.as_rule(), Rule::set_val | Rule::set_val_data))
                        .map(|p| -> Result<SetValTerminal> {
                            let inner = p.into_inner().next().context("empty set_val")?;
                            Ok(match inner.as_rule() {
                                Rule::id | Rule::id_data => {
                                    SetValTerminal::Str(intern(inner.as_str()))
                                }
                                Rule::int => {
                                    SetValTerminal::Int(inner.as_str().parse().unwrap_or(0))
                                }
                                _ => SetValTerminal::Str(intern(inner.as_str())),
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    ensure!(
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

    Ok(SetVals(values))
}

/// Parse set_assign (`:= ...`) into SetVals (used by SetData in data section)
fn parse_set_assign(pair: Pair<Rule>) -> Result<SetVals> {
    match pair.into_inner().next() {
        Some(inner) => parse_set_vals_or_tuples(inner),
        None => Ok(SetVals(vec![])),
    }
}

/// Parse param_data_body into ParamDataBody (reused by Param and ParamData)
fn parse_param_data_body(pair: Pair<Rule>) -> Result<ParamDataBody> {
    let mut inner_pairs = pair.into_inner();
    let first = inner_pairs.next().context("empty param_data_body")?;

    Ok(match first.as_rule() {
        Rule::param_data_list => {
            let pairs: Vec<_> = first
                .into_inner()
                .map(ParamDataPair::from_entry)
                .collect::<Result<_>>()?;
            ParamDataBody::List(pairs)
        }
        Rule::param_data_matrix => {
            let mut tables = vec![ParamDataTable::from_entry(first)?];
            let rest: Vec<_> = inner_pairs
                .map(ParamDataTable::from_entry)
                .collect::<Result<_>>()?;
            tables.extend(rest);
            ParamDataBody::Tables(tables)
        }
        Rule::param_data_scalar => {
            let num: f64 = first.as_str().parse()?;
            ParamDataBody::Num(num)
        }
        Rule::param_data_symbolic => {
            let s = first.as_str();
            let unquoted = s[1..s.len() - 1].to_string();
            ParamDataBody::Symbolic(unquoted)
        }
        _ => bail!("unexpected rule in param_data_body: {:?}", first.as_rule()),
    })
}

/// Subscript (array indexing with optional shifts)
#[derive(Clone, Debug, Default)]
pub struct Subscript(pub Vec<SubscriptPart>);

impl Subscript {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let parts = entry
            .into_inner()
            .filter(|p| p.as_rule() == Rule::subscript_part)
            .map(SubscriptPart::from_entry)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self(parts))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &SubscriptPart> {
        self.0.iter()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SubscriptPart {
    pub var: SubscriptPartVar,
    pub shift: Option<SubscriptShift>,
}

impl SubscriptPart {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut var = SubscriptPartVar::Var(intern(""));
        let mut shift = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => var = SubscriptPartVar::Var(intern(pair.as_str())),
                Rule::int => var = SubscriptPartVar::ValInt(pair.as_str().parse().unwrap()),
                Rule::string_literal => {
                    let s = pair.as_str();
                    let s = &s[1..s.len() - 1]; // strip quotes
                    var = SubscriptPartVar::ValStr(intern(s));
                }
                Rule::subscript_shift => shift = Some(SubscriptShift::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self { var, shift })
    }
}

impl fmt::Display for SubscriptPart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.var {
            SubscriptPartVar::Var(s) | SubscriptPartVar::ValStr(s) => {
                write!(f, "{}", intern_resolve(s))?;
            }
            SubscriptPartVar::ValInt(n) => {
                write!(f, "{}", n)?;
            }
        }
        if let Some(shift) = &self.shift {
            write!(f, "{}", shift)?;
        }
        Ok(())
    }
}

/// SubscriptPartVar (value or reference)
#[derive(Clone, Copy, Debug)]
pub enum SubscriptPartVar {
    Var(Spur),    // a variable from a domain, like i, j
    ValStr(Spur), // a value from a set, eg `gas`
    ValInt(u32),  // a value from a set, eg `4`
}

/// Subscript shift (+1 or -1)
#[derive(Clone, Copy, Debug)]
pub enum SubscriptShift {
    Plus,
    Minus,
}

impl SubscriptShift {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let s = entry.as_str();
        Ok(if s.starts_with('+') {
            SubscriptShift::Plus
        } else {
            SubscriptShift::Minus
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut domain = None;
        let mut var: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)?),
                Rule::func_var => var = Some(intern(pair.as_str())),
                _ => {}
            }
        }
        let domain = domain.context("missing func_min domain")?;

        Ok(Self {
            domain,
            var: var.context("missing func_min var")?,
        })
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
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut domain = None;
        let mut var: Option<Spur> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)?),
                Rule::func_var => var = Some(intern(pair.as_str())),
                _ => {}
            }
        }
        let domain = domain.context("missing func_max domain")?;

        Ok(Self {
            domain,
            var: var.context("missing func_max var")?,
        })
    }
}

impl fmt::Display for FuncMax {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "max <domain> max(<var>)")
    }
}
