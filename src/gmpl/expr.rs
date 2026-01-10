use std::fmt;
use std::sync::LazyLock;

use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::{Assoc::*, Op, PrattParser};

use crate::{
    gmpl::atoms::{BoolOp, Domain, FuncMax, FuncMin, FuncSum, MathOp, RelOp, VarSubscripted},
    grammar::Rule,
};

static PRATT_PARSER: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    PrattParser::new()
        // Precedence lowest to highest (per GMPL spec)
        .op(Op::infix(Rule::add, Left) | Op::infix(Rule::sub, Left))
        .op(Op::prefix(Rule::sum_prefix)) // iterated ops: between add/sub and mul/div
        .op(Op::infix(Rule::mul, Left) | Op::infix(Rule::div, Left))
        .op(Op::prefix(Rule::neg))
        .op(Op::infix(Rule::pow, Right))
});

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

static LOGIC_PRATT: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    PrattParser::new()
        // Precedence: and > or (standard convention)
        .op(Op::infix(Rule::bool_or, Left))
        .op(Op::infix(Rule::bool_and, Left))
});

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

        let inner: Vec<_> = entry.into_inner().collect();
        let mut i = 0;
        while i < inner.len() {
            let pair = &inner[i];
            match pair.as_rule() {
                Rule::logic_expr => condition = Some(LogicExpr::from_entry(pair.clone())),
                Rule::expr => {
                    if then_expr.is_none() {
                        then_expr = Some(Box::new(Expr::from_entry(pair.clone())));
                    } else {
                        else_expr = Some(Box::new(Expr::from_entry(pair.clone())));
                    }
                }
                _ => {}
            }
            i += 1;
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
