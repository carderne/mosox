use std::fmt;

use pest::iterators::Pair;

use crate::{
    gmpl::{
        IndexVal,
        expr::{Expr, LogicExpr},
    },
    grammar::Rule,
};

// ==============================
// CHILD STRUCTS
// ==============================

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
        let vars: Vec<_> = self.parts.iter().map(|p| p.var.as_str()).collect();
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
    pub var: String,
    pub set: String,
}

impl DomainPart {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut var = String::new();
        let mut set = String::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain_var => var = pair.as_str().to_string(),
                Rule::domain_set => set = pair.as_str().to_string(),
                _ => {}
            }
        }

        Self { var, set }
    }
}

impl fmt::Display for DomainPart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} in {}", self.var, self.set)
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
    pub var: String,
    pub subscript: Option<Subscript>,
    pub concrete: Option<Vec<IndexVal>>,
}

impl VarSubscripted {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut var = String::new();
        let mut subscript = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::var_ref => var = pair.as_str().to_string(),
                Rule::subscript => subscript = Some(Subscript::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            var,
            subscript,
            concrete: None,
        }
    }
}

impl fmt::Display for VarSubscripted {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.var)?;
        if self.subscript.is_some() {
            write!(f, "[...]")?;
        }
        Ok(())
    }
}

/// Subscript (array indexing)
#[derive(Clone, Debug)]
pub struct Subscript {
    pub indices: Vec<Index>,
}

impl Subscript {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut indices = Vec::new();

        for pair in entry.into_inner() {
            if pair.as_rule() == Rule::index {
                indices.push(Index::from_entry(pair));
            }
        }

        Self { indices }
    }
}

impl fmt::Display for Subscript {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[")?;
        for (i, idx) in self.indices.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", idx)?;
        }
        write!(f, "]")
    }
}

/// Index with optional shift
#[derive(Clone, Debug)]
pub struct Index {
    pub var: String,
    pub shift: Option<IndexShift>,
}

impl Index {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut var = String::new();
        let mut shift = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::index_var => var = pair.as_str().to_string(),
                Rule::index_shift => shift = Some(IndexShift::from_entry(pair)),
                _ => {}
            }
        }

        Self { var, shift }
    }
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.var)?;
        if let Some(shift) = &self.shift {
            write!(f, "{}", shift)?;
        }
        Ok(())
    }
}

/// Index shift (+1 or -1)
#[derive(Clone, Debug)]
pub enum IndexShift {
    Plus,
    Minus,
}

impl IndexShift {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let s = entry.as_str();
        if s.starts_with('+') {
            IndexShift::Plus
        } else {
            IndexShift::Minus
        }
    }
}

impl fmt::Display for IndexShift {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IndexShift::Plus => write!(f, "+1"),
            IndexShift::Minus => write!(f, "-1"),
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
    pub var: String,
}

impl FuncMin {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut domain = None;
        let mut var = String::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::func_var => var = pair.as_str().to_string(),
                _ => {}
            }
        }
        let domain = domain.unwrap();

        Self { domain, var }
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
    pub var: String,
}

impl FuncMax {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut domain = None;
        let mut var = String::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::func_var => var = pair.as_str().to_string(),
                _ => {}
            }
        }
        let domain = domain.unwrap();

        Self { domain, var }
    }
}

impl fmt::Display for FuncMax {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "max <domain> max(<var>)")
    }
}
