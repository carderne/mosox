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

static SET_PRATT: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    PrattParser::new()
        // inter and union at the same precedence level (no ordering defined)
        .op(Op::infix(Rule::set_infix_op, Left))
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

#[derive(Clone, Debug)]
pub enum SetExpr {
    /// A bare set atom: domain, setof, arith range, or set ref
    Atom(SetAtom),
    /// Binary inter/union: `lhs inter rhs`, `lhs union rhs`
    InfixOp {
        lhs: Box<SetExpr>,
        op: SetInfixOp,
        rhs: Box<SetExpr>,
    },
}

impl SetExpr {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        parse_set_expr(entry.into_inner())
    }
}

fn parse_set_expr(pairs: Pairs<Rule>) -> Result<SetExpr> {
    SET_PRATT
        .map_primary(|primary| match primary.as_rule() {
            Rule::set_atom => {
                let inner = primary.into_inner().next().context("empty set_atom")?;
                Ok(SetExpr::Atom(match inner.as_rule() {
                    Rule::domain => SetAtom::Domain(Domain::from_entry(inner)?),
                    Rule::set_setof => SetAtom::SetOf(SetOf::from_entry(inner)?),
                    Rule::set_arith => SetAtom::Arith(SetArith::from_entry(inner)?),
                    Rule::set_ref => SetAtom::Ref(SetRef::from_entry(inner)?),
                    Rule::set_expr => return parse_set_expr(inner.into_inner()),
                    rule => bail!("Unexpected rule in set_atom: {:?}", rule),
                }))
            }
            rule => bail!("Expected set_atom primary, found {:?}", rule),
        })
        .map_infix(|lhs, op, rhs| {
            let op = match op.as_str() {
                "inter" => SetInfixOp::Inter,
                "union" => SetInfixOp::Union,
                s => bail!("Unexpected set_infix_op: {}", s),
            };
            Ok(SetExpr::InfixOp {
                lhs: Box::new(lhs?),
                op,
                rhs: Box::new(rhs?),
            })
        })
        .parse(pairs)
}

/// A leaf node in a set expression
#[derive(Clone, Debug)]
pub enum SetAtom {
    Domain(Domain),
    SetOf(SetOf),
    Arith(SetArith),
    Ref(SetRef),
}

/// Inter or union operator
#[derive(Clone, Copy, Debug)]
pub enum SetInfixOp {
    Inter,
    Union,
}

#[derive(Clone, Debug)]
pub struct SetRef {
    pub spur: Spur,
    pub subscript: Subscript,
}

impl SetRef {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut spur = None;
        let mut subscript = Subscript::default();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => spur = Some(intern(pair.as_str())),
                Rule::subscript => subscript = Subscript::from_entry(pair)?,
                _ => {}
            }
        }

        Ok(Self {
            spur: spur.context("missing set ref id")?,
            subscript,
        })
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

/// Arithmetic range set: `lo..hi`
#[derive(Clone, Debug)]
pub struct SetArith {
    pub lo: SetArithEnd,
    pub hi: SetArithEnd,
}

impl SetArith {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut lo: Option<SetArithEnd> = None;
        let mut hi: Option<SetArithEnd> = None;

        for pair in entry.into_inner() {
            if pair.as_rule() == Rule::set_arith_end {
                let end = SetArithEnd::from_entry(pair)?;
                if lo.is_none() {
                    lo = Some(end);
                } else {
                    hi = Some(end);
                }
            }
        }

        Ok(Self {
            lo: lo.context("missing set_arith lower bound")?,
            hi: hi.context("missing set_arith upper bound")?,
        })
    }
}

/// Endpoint of a set arithmetic range: either a bare integer or a named set with optional subscript and shift
#[derive(Clone, Debug)]
pub enum SetArithEnd {
    Int(u32),
    Named {
        name: Spur,
        subscript: Subscript,
        shift: Option<SubscriptShift>,
    },
}

impl SetArithEnd {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut int_val: Option<u32> = None;
        let mut name: Option<Spur> = None;
        let mut subscript = Subscript::default();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::int => int_val = Some(pair.as_str().parse()?),
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::subscript => subscript = Subscript::from_entry(pair)?,
                Rule::set_arith_fancy => {
                    return Self::parse_fancy(pair);
                }
                _ => {}
            }
        }

        if let Some(n) = int_val {
            Ok(SetArithEnd::Int(n))
        } else {
            Ok(SetArithEnd::Named {
                name: name.context("missing set_arith_end name")?,
                subscript,
                shift: None,
            })
        }
    }

    fn parse_fancy(entry: Pair<Rule>) -> Result<Self> {
        let mut name: Option<Spur> = None;
        let mut subscript = Subscript::default();
        let mut is_add = true;
        let mut offset: Option<u32> = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = Some(intern(pair.as_str())),
                Rule::subscript => subscript = Subscript::from_entry(pair)?,
                Rule::add => is_add = true,
                Rule::sub => is_add = false,
                Rule::int => offset = Some(pair.as_str().parse()?),
                _ => {}
            }
        }

        let offset = offset.context("missing set_arith_fancy offset")?;
        let shift = if is_add {
            SubscriptShift::Plus(offset)
        } else {
            SubscriptShift::Minus(offset)
        };

        Ok(SetArithEnd::Named {
            name: name.context("missing set_arith_fancy name")?,
            subscript,
            shift: Some(shift),
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

/// Data parameter values
#[derive(Clone, Debug)]
pub struct ParamDataPair {
    pub key: SetVal,
    pub value: ParamVal,
}

impl ParamDataPair {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut tokens = entry.into_inner();
        let key = SetVal::from_entry(tokens.next().context("missing param data key")?)?;
        let value = ParamVal::from_entry(tokens.next().context("missing param data value")?)?;
        Ok(Self { key, value })
    }
}

#[derive(Clone, Debug)]
pub enum ParamDataPlainValue {
    Scalar(ParamVal),
    Pairs(Vec<ParamDataPair>),
}

#[derive(Clone, Debug)]
pub struct ParamDataPlain {
    pub target: Option<Vec<ParamDataTarget>>,
    pub value: ParamDataPlainValue,
}

impl ParamDataPlain {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut target = None;
        let mut pairs = Vec::new();
        let mut scalar = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::param_data_target => target = Some(parse_param_data_target(pair)?),
                Rule::param_data_pair => pairs.push(ParamDataPair::from_entry(pair)?),
                Rule::param_data_val => scalar = Some(ParamVal::from_entry(pair)?),
                _ => {}
            }
        }

        let value = if !pairs.is_empty() {
            ParamDataPlainValue::Pairs(pairs)
        } else {
            ParamDataPlainValue::Scalar(scalar.context("missing param_data_plain value")?)
        };

        Ok(Self { target, value })
    }
}

#[derive(Clone, Debug)]
pub enum ParamDataBody {
    Tabular(Vec<ParamDataTable>),
    Plain(Vec<ParamDataPlain>),
}

#[derive(Clone, Debug)]
pub struct ParamData {
    pub name: Spur,
    pub default: Option<ParamVal>,
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
                Rule::param_data_default => default = Some(ParamVal::Num(pair.as_str().parse()?)),
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
                    target = Some(parse_param_data_target(pair)?);
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

/// Parameter data target
#[derive(Clone, Debug)]
pub enum ParamDataTarget {
    IndexVar(SetVal),
    Any,
}

/// Parameter data row
#[derive(Clone, Debug)]
pub struct ParamDataRow {
    pub label: SetVal,
    pub values: Vec<ParamVal>,
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
                            values.push(ParamVal::from_entry(inner)?);
                        }
                    }
                }
                Rule::param_data_val => values.push(ParamVal::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self {
            label: label.context("missing param data row label")?,
            values,
        })
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

/// Expression - recursive tree structure with proper operator precedence
#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    VarSubscripted(VarSubscripted),
    FuncSum(Box<FuncSum>),
    FuncMin(Box<FuncMin>),
    FuncMax(Box<FuncMax>),
    FuncCard(Box<FuncCard>),
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
            Rule::func_card => Ok(Expr::FuncCard(Box::new(FuncCard::from_entry(primary)?))),
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

// TODO remove these display
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

/// Param val
/// Usually number, but can be symbolic (string)
/// When symbolic, should only be used for eg indexing and obviously cannot end up in matrix
/// What a pain.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ParamVal {
    Str(Spur),
    Num(f64),
}

impl ParamVal {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let inner = entry.into_inner().next().context("empty param_data_val")?;
        Ok(match inner.as_rule() {
            Rule::id => ParamVal::Str(intern(inner.as_str())),
            Rule::number => ParamVal::Num(inner.as_str().parse()?),
            _ => bail!("unexpected rule in param_data_val: {:?}", inner.as_rule()),
        })
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

/// Single domain part (e.g., "r in REGION")
#[derive(Clone, Debug)]
pub struct DomainPart {
    pub var: DomainPartVar,
    pub expr: SetExpr,
}

impl DomainPart {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut var = DomainPartVar::None;
        let mut expr = None;

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
                Rule::set_expr => expr = Some(SetExpr::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self {
            var,
            expr: expr.context("missing domain_part expr")?,
        })
    }
}

#[derive(Clone, Debug)]
pub enum DomainPartVar {
    None,
    Single(Spur),
    Tuple(Vec<Spur>),
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
                Rule::id => var = Some(intern(pair.as_str())),
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

/// Parse param_data_target into Vec<ParamDataTarget>
fn parse_param_data_target(pair: Pair<Rule>) -> Result<Vec<ParamDataTarget>> {
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
    Ok(targets)
}

/// Parse param_data_body into ParamDataBody (reused by Param and ParamData)
fn parse_param_data_body(pair: Pair<Rule>) -> Result<ParamDataBody> {
    let mut inner_pairs = pair.into_inner();
    let first = inner_pairs.next().context("empty param_data_body")?;

    Ok(match first.as_rule() {
        Rule::param_data_matrix => {
            let mut tables = vec![ParamDataTable::from_entry(first)?];
            let rest: Vec<_> = inner_pairs
                .map(ParamDataTable::from_entry)
                .collect::<Result<_>>()?;
            tables.extend(rest);
            ParamDataBody::Tabular(tables)
        }
        Rule::param_data_symbolic => {
            let s = first.as_str();
            let unquoted = s[1..s.len() - 1].to_string();
            let plain = ParamDataPlain {
                target: None,
                value: ParamDataPlainValue::Scalar(ParamVal::Str(intern(&unquoted))),
            };
            ParamDataBody::Plain(vec![plain])
        }
        Rule::param_data_entry => {
            let mut entries = vec![ParamDataPlain::from_entry(first)?];
            let rest: Vec<_> = inner_pairs
                .map(ParamDataPlain::from_entry)
                .collect::<Result<_>>()?;
            entries.extend(rest);
            ParamDataBody::Plain(entries)
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

#[derive(Clone, Debug)]
pub struct SubscriptPart {
    pub var: SubscriptPartVar,
    pub shift: Option<SubscriptShift>,
}

impl SubscriptPart {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut var = None;
        let mut shift = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::var_subscripted => {
                    var = Some(SubscriptPartVar::Var(VarSubscripted::from_entry(pair)?));
                }
                Rule::int => var = Some(SubscriptPartVar::ValInt(pair.as_str().parse().unwrap())),
                Rule::string_literal => {
                    let s = pair.as_str();
                    let s = &s[1..s.len() - 1]; // strip quotes
                    var = Some(SubscriptPartVar::ValStr(intern(s)));
                }
                Rule::subscript_shift => shift = Some(SubscriptShift::from_entry(pair)?),
                _ => {}
            }
        }

        Ok(Self {
            var: var.context("missing subscript_part var")?,
            shift,
        })
    }
}

/// SubscriptPartVar (value or reference)
#[derive(Clone, Debug)]
pub enum SubscriptPartVar {
    ValStr(Spur),        // a value from a set, eg `gas`
    ValInt(u32),         // a value from a set, eg `4`
    Var(VarSubscripted), // a var/param, possibly subscripted, eg `i` or `foo[i]`
}

/// Subscript shift (+n or -n)
#[derive(Clone, Copy, Debug)]
pub enum SubscriptShift {
    Plus(u32),
    Minus(u32),
}

impl SubscriptShift {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let mut is_add = true;
        let mut val: u32 = 1;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::add => is_add = true,
                Rule::sub => is_add = false,
                Rule::int => val = pair.as_str().parse().unwrap_or(1),
                _ => {}
            }
        }

        Ok(if is_add {
            SubscriptShift::Plus(val)
        } else {
            SubscriptShift::Minus(val)
        })
    }
}

/// Sum function (iterated sum over a domain)
#[derive(Clone, Debug)]
pub struct FuncSum {
    pub domain: Domain,
    pub operand: Box<Expr>,
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

/// Card function (cardinality of a set)
#[derive(Clone, Debug)]
pub struct FuncCard {
    pub expr: SetExpr,
}

impl FuncCard {
    pub fn from_entry(entry: Pair<Rule>) -> Result<Self> {
        let inner = entry
            .into_inner()
            .next()
            .context("missing func_card set_expr")?;
        Ok(Self {
            expr: SetExpr::from_entry(inner)?,
        })
    }
}
