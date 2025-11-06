use std::fmt;

use crate::grammar::Rule;
use pest::iterators::Pair;

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
    pub param_in: Option<ExprOrLiteral>,
    pub default: Option<ExprOrLiteral>,
    pub assign: Option<ExprOrLiteral>,
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
                Rule::param_in => {
                    param_in = pair
                        .into_inner()
                        .next()
                        .map(|p| ExprOrLiteral::from_entry(p))
                }
                Rule::param_default => {
                    default = pair
                        .into_inner()
                        .next()
                        .map(|p| ExprOrLiteral::from_entry(p))
                }
                Rule::param_assign => {
                    assign = pair
                        .into_inner()
                        .next()
                        .map(|p| ExprOrLiteral::from_entry(p))
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
    pub constraint_exprs: Vec<ConstraintExpr>,
}

impl Constraint {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut domain = None;
        let mut constraint_exprs = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = pair.as_str().to_string(),
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::constraint_expr => constraint_exprs.push(ConstraintExpr::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            name,
            domain,
            constraint_exprs,
        }
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "constraint {}", self.name)?;
        if self.domain.is_some() {
            write!(f, " <domain>")?;
        }
        write!(f, ": <{} constraint expr(s)>", self.constraint_exprs.len())
    }
}

/// Data set values
#[derive(Clone, Debug)]
pub struct DataSet {
    pub name: String,
    pub values: Vec<Value>,
}

impl DataSet {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut values = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::name => name = pair.as_str().to_string(),
                Rule::value => values.push(Value::from_entry(pair)),
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
pub struct DataParam {
    pub name: String,
    pub default: Option<String>,
    pub tables: Vec<ParamDataTable>,
}

impl DataParam {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut name = String::new();
        let mut default = None;
        let mut tables = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::id => name = pair.as_str().to_string(),
                Rule::param_data_default => default = Some(pair.as_str().to_string()),
                Rule::param_data_table => tables.push(ParamDataTable::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            name,
            default,
            tables,
        }
    }
}

impl fmt::Display for DataParam {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "data: param {}", self.name)?;
        if self.default.is_some() {
            write!(f, " default <value>")?;
        }
        if !self.tables.is_empty() {
            write!(f, " := <{} table(s)>", self.tables.len())?;
        }
        Ok(())
    }
}

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

/// Logical expression
#[derive(Clone, Debug)]
pub struct LogicExpr {
    pub parts: Vec<LogicExprPart>,
    pub operators: Vec<BoolOp>,
}

impl LogicExpr {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut parts = Vec::new();
        let mut operators = Vec::new();

        let inner: Vec<_> = entry.into_inner().collect();
        for (i, pair) in inner.iter().enumerate() {
            match pair.as_rule() {
                Rule::logic_expr_compound => {
                    // Extract inner logic_expr from compound
                    let inner_logic = pair.clone().into_inner().next().unwrap();
                    parts.push(LogicExprPart::Compound(Box::new(LogicExpr::from_entry(
                        inner_logic,
                    ))));
                }
                Rule::expr => {
                    // This is the start of a comparison (expr rel_op expr)
                    if i + 2 < inner.len()
                        && let (Some(op_pair), Some(rhs_pair)) =
                            (inner.get(i + 1), inner.get(i + 2))
                        && op_pair.as_rule() == Rule::rel_op
                    {
                        let lhs = ExprOrLiteral::Expr(Expr::from_entry(pair.clone()));
                        let op = RelOp::from_entry(op_pair.clone());
                        let rhs = ExprOrLiteral::Expr(Expr::from_entry(rhs_pair.clone()));
                        parts.push(LogicExprPart::Comparison { lhs, op, rhs });
                    }
                }
                Rule::string_literal => {
                    // Part of a comparison with string literal
                    if i > 0
                        && i + 1 < inner.len()
                        && let Some(prev) = inner.get(i - 1)
                        && prev.as_rule() == Rule::rel_op
                    {
                        // Already handled in previous iteration
                        continue;
                    }
                }
                Rule::bool => operators.push(BoolOp::from_entry(pair.clone())),
                Rule::rel_op => {
                    // Skip, handled with expr
                }
                _ => {}
            }
        }

        Self { parts, operators }
    }
}

impl fmt::Display for LogicExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<logic expr>")
    }
}

/// Part of a logical expression
#[derive(Clone, Debug)]
pub enum LogicExprPart {
    Comparison {
        lhs: ExprOrLiteral,
        op: RelOp,
        rhs: ExprOrLiteral,
    },
    Compound(Box<LogicExpr>),
}

impl fmt::Display for LogicExprPart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<logic part>")
    }
}

/// Expression
#[derive(Clone, Debug)]
pub struct Expr {
    pub negated: bool,
    pub terms: Vec<ExprTerm>,
    pub operators: Vec<MathOp>,
}

impl Expr {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut negated = false;
        let mut terms = Vec::new();
        let mut operators = Vec::new();

        // Check if first term is negative before consuming entry
        let expr_str = entry.as_str().trim();
        if expr_str.starts_with('-') {
            negated = true;
        }

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::number => {
                    let val = pair.as_str().parse().unwrap_or(0.0);
                    terms.push(ExprTerm::Number(val));
                }
                Rule::var_subscripted => {
                    terms.push(ExprTerm::VarSubscripted(VarSubscripted::from_entry(pair)))
                }
                Rule::func_sum => terms.push(ExprTerm::FuncSum(FuncSum::from_entry(pair))),
                Rule::func_min => terms.push(ExprTerm::FuncMin(FuncMin::from_entry(pair))),
                Rule::func_max => terms.push(ExprTerm::FuncMax(FuncMax::from_entry(pair))),
                Rule::conditional => {
                    terms.push(ExprTerm::Conditional(Conditional::from_entry(pair)))
                }
                Rule::expr => terms.push(ExprTerm::Expr(Box::new(Expr::from_entry(pair)))),
                Rule::math_op => operators.push(MathOp::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            negated,
            terms,
            operators,
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<expr>")
    }
}

/// Expression term
#[derive(Clone, Debug)]
pub enum ExprTerm {
    Number(f64),
    VarSubscripted(VarSubscripted),
    FuncSum(FuncSum),
    FuncMin(FuncMin),
    FuncMax(FuncMax),
    Conditional(Conditional),
    Expr(Box<Expr>),
}

impl fmt::Display for ExprTerm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExprTerm::Number(n) => write!(f, "{}", n),
            ExprTerm::VarSubscripted(v) => write!(f, "{}", v),
            ExprTerm::FuncSum(_) => write!(f, "<sum>"),
            ExprTerm::FuncMin(_) => write!(f, "<min>"),
            ExprTerm::FuncMax(_) => write!(f, "<max>"),
            ExprTerm::Conditional(_) => write!(f, "<if-then-else>"),
            ExprTerm::Expr(_) => write!(f, "(<expr>)"),
        }
    }
}

/// Expression or string literal
#[derive(Clone, Debug)]
pub enum ExprOrLiteral {
    Expr(Expr),
    StringLiteral(String),
}

impl ExprOrLiteral {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        match entry.as_rule() {
            Rule::expr => ExprOrLiteral::Expr(Expr::from_entry(entry)),
            Rule::string_literal => ExprOrLiteral::StringLiteral(entry.as_str().to_string()),
            _ => {
                // For cases where we get the inner directly
                if let Some(inner) = entry.into_inner().next() {
                    ExprOrLiteral::from_entry(inner)
                } else {
                    ExprOrLiteral::StringLiteral(String::new())
                }
            }
        }
    }
}

impl fmt::Display for ExprOrLiteral {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExprOrLiteral::Expr(_) => write!(f, "<expr>"),
            ExprOrLiteral::StringLiteral(s) => write!(f, "{}", s),
        }
    }
}

/// Variable with optional subscript
#[derive(Clone, Debug)]
pub struct VarSubscripted {
    pub var: String,
    pub subscript: Option<Subscript>,
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

        Self { var, subscript }
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

/// Sum function
#[derive(Clone, Debug)]
pub struct FuncSum {
    pub domain: Domain,
    pub operand: FuncSumOperand,
}

impl FuncSum {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut domain = None;
        let mut operand = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::domain => domain = Some(Domain::from_entry(pair)),
                Rule::func_sum_operand => operand = Some(FuncSumOperand::from_entry(pair)),
                _ => {}
            }
        }

        Self {
            domain: domain.unwrap(),
            operand: operand.unwrap(),
        }
    }
}

impl fmt::Display for FuncSum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "sum <domain> <operand>")
    }
}

/// Sum function operand
#[derive(Clone, Debug)]
pub enum FuncSumOperand {
    VarSubscripted(VarSubscripted),
    Expr(Expr),
}

impl FuncSumOperand {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let inner = entry.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::var_subscripted => {
                FuncSumOperand::VarSubscripted(VarSubscripted::from_entry(inner))
            }
            Rule::expr => FuncSumOperand::Expr(Expr::from_entry(inner)),
            _ => FuncSumOperand::Expr(Expr::from_entry(inner)),
        }
    }
}

impl fmt::Display for FuncSumOperand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FuncSumOperand::VarSubscripted(v) => write!(f, "{}", v),
            FuncSumOperand::Expr(_) => write!(f, "(<expr>)"),
        }
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

        Self {
            domain: domain.unwrap(),
            var,
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

        Self {
            domain: domain.unwrap(),
            var,
        }
    }
}

impl fmt::Display for FuncMax {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "max <domain> max(<var>)")
    }
}

/// Conditional expression (if-then-else)
#[derive(Clone, Debug)]
pub struct Conditional {
    pub condition: LogicExpr,
    pub then_expr: Box<ExprOrLiteral>,
    pub else_expr: Option<Box<ExprOrLiteral>>,
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
                        then_expr = Some(Box::new(ExprOrLiteral::Expr(Expr::from_entry(
                            pair.clone(),
                        ))));
                    } else {
                        else_expr = Some(Box::new(ExprOrLiteral::Expr(Expr::from_entry(
                            pair.clone(),
                        ))));
                    }
                }
                Rule::string_literal => {
                    if then_expr.is_none() {
                        then_expr = Some(Box::new(ExprOrLiteral::StringLiteral(
                            pair.as_str().to_string(),
                        )));
                    } else {
                        else_expr = Some(Box::new(ExprOrLiteral::StringLiteral(
                            pair.as_str().to_string(),
                        )));
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
        write!(f, "if <condition> then <expr>")?;
        if self.else_expr.is_some() {
            write!(f, " else <expr>")?;
        }
        Ok(())
    }
}

// ==============================
// ENUMS AND OPERATORS
// ==============================

/// Relational operator
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
    pub value: ExprOrLiteral,
}

impl ParamCondition {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut op = RelOp::Ge;
        let mut value = None;

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::rel_op => op = RelOp::from_entry(pair),
                Rule::expr => value = Some(ExprOrLiteral::Expr(Expr::from_entry(pair))),
                Rule::string_literal => {
                    value = Some(ExprOrLiteral::StringLiteral(pair.as_str().to_string()))
                }
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

/// Value (identifier or number)
#[derive(Clone, Debug)]
pub enum Value {
    Id(String),
    Number(f64),
}

impl Value {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let inner = entry.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::id => Value::Id(inner.as_str().to_string()),
            Rule::number => Value::Number(inner.as_str().parse().unwrap_or(0.0)),
            _ => Value::Id(inner.as_str().to_string()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Id(s) => write!(f, "{}", s),
            Value::Number(n) => write!(f, "{}", n),
        }
    }
}

/// Parameter data table
#[derive(Clone, Debug)]
pub struct ParamDataTable {
    pub target: Option<Vec<ParamDataTarget>>,
    pub cols: Vec<String>,
    pub rows: Vec<ParamDataRow>,
}

impl ParamDataTable {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut target = None;
        let mut cols = Vec::new();
        let mut rows = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::param_data_target => {
                    let mut targets = Vec::new();
                    for inner in pair.into_inner() {
                        match inner.as_rule() {
                            Rule::index_var => {
                                targets.push(ParamDataTarget::IndexVar(inner.as_str().to_string()))
                            }
                            Rule::param_data_any => targets.push(ParamDataTarget::Any),
                            _ => {}
                        }
                    }
                    target = Some(targets);
                }
                Rule::param_data_cols => {
                    for inner in pair.into_inner() {
                        if inner.as_rule() == Rule::param_data_label {
                            cols.push(inner.as_str().to_string());
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
    IndexVar(String),
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
    pub label: String,
    pub values: Vec<f64>,
}

impl ParamDataRow {
    pub fn from_entry(entry: Pair<Rule>) -> Self {
        let mut label = String::new();
        let mut values = Vec::new();

        for pair in entry.into_inner() {
            match pair.as_rule() {
                Rule::param_data_label => {
                    if label.is_empty() {
                        label = pair.as_str().to_string();
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

        Self { label, values }
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
