// Copyright 2018 sqlparser-rs contributors. All rights reserved.
// Copyright Materialize, Inc. All rights reserved.
//
// This file is derived from the sqlparser-rs project, available at
// https://github.com/andygrove/sqlparser-rs. It was incorporated
// directly into Materialize on December 21, 2019.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE file at the
// root of this repository, or online at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// An SQL expression of any type.
///
/// The parser does not distinguish between expressions of different types
/// (e.g. boolean vs string), so the caller must handle expressions of
/// inappropriate type, like `WHERE 1` or `SELECT 1=1`, as necessary.
use crate::ast::display::{self, AstDisplay, AstFormatter};
use crate::ast::{
    BinaryOperator, DataType, Ident, ObjectName, OrderByExpr, Query, UnaryOperator, Value,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    /// Identifier e.g. table name or column name
    Identifier(Vec<Ident>),
    /// Qualified wildcard, e.g. `alias.*` or `schema.table.*`.
    /// (Same caveats apply to `QualifiedWildcard` as to `Wildcard`.)
    QualifiedWildcard(Vec<Ident>),
    /// A positional parameter, e.g., `$1` or `$42`
    Parameter(usize),
    /// `IS NULL` expression
    IsNull(Box<Expr>),
    /// `IS NOT NULL` expression
    IsNotNull(Box<Expr>),
    /// `[ NOT ] IN (val1, val2, ...)`
    InList {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
    /// `[ NOT ] IN (SELECT ...)`
    InSubquery {
        expr: Box<Expr>,
        subquery: Box<Query>,
        negated: bool,
    },
    /// `<expr> [ NOT ] BETWEEN <low> AND <high>`
    Between {
        expr: Box<Expr>,
        negated: bool,
        low: Box<Expr>,
        high: Box<Expr>,
    },
    /// Binary operation e.g. `1 + 1` or `foo > bar`
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    /// Unary operation e.g. `NOT foo`
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expr>,
    },
    /// CAST an expression to a different data type e.g. `CAST(foo AS VARCHAR(123))`
    Cast {
        expr: Box<Expr>,
        data_type: DataType,
    },
    Extract {
        field: String,
        expr: Box<Expr>,
    },
    Trim {
        side: TrimSide,
        exprs: Vec<Expr>,
    },
    /// `expr COLLATE collation`
    Collate {
        expr: Box<Expr>,
        collation: ObjectName,
    },
    /// COALESCE(<expr>, ...)
    ///
    /// While COALESCE has the same syntax as a function call, its semantics are
    /// extremely unusual, and are better captured with a dedicated AST node.
    Coalesce {
        exprs: Vec<Expr>,
    },
    /// Nested expression e.g. `(foo > bar)` or `(1)`
    Nested(Box<Expr>),
    /// A row constructor like `ROW(<expr>...)` or `(<expr>, <expr>...)`.
    Row {
        exprs: Vec<Expr>,
    },
    /// A literal value, such as string, number, date or NULL
    Value(Value),
    /// Scalar function call e.g. `LEFT(foo, 5)`
    Function(Function),
    /// `CASE [<operand>] WHEN <condition> THEN <result> ... [ELSE <result>] END`
    ///
    /// Note we only recognize a complete single expression as `<condition>`,
    /// not `< 0` nor `1, 2, 3` as allowed in a `<simple when clause>` per
    /// <https://jakewheat.github.io/sql-overview/sql-2011-foundation-grammar.html#simple-when-clause>
    Case {
        operand: Option<Box<Expr>>,
        conditions: Vec<Expr>,
        results: Vec<Expr>,
        else_result: Option<Box<Expr>>,
    },
    /// An exists expression `EXISTS(SELECT ...)`, used in expressions like
    /// `WHERE EXISTS (SELECT ...)`.
    Exists(Box<Query>),
    /// A parenthesized subquery `(SELECT ...)`, used in expression like
    /// `SELECT (subquery) AS x` or `WHERE (subquery) = x`
    Subquery(Box<Query>),
    /// `<expr> <op> ANY/SOME (<query>)`
    Any {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Query>,
        some: bool, // just tracks which syntax was used
    },
    /// `<expr> <op> ALL (<query>)`
    All {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Query>,
    },
    /// `LIST[<expr>*]`
    List(Vec<Expr>),
}

impl AstDisplay for Expr {
    fn fmt(&self, f: &mut AstFormatter) {
        match self {
            Expr::Identifier(s) => f.write_node(&display::separated(s, ".")),
            Expr::QualifiedWildcard(q) => {
                f.write_node(&display::separated(q, "."));
                f.write_str(".*");
            }
            Expr::Parameter(n) => f.write_str(&format!("${}", n)),
            Expr::IsNull(ast) => {
                f.write_node(&ast);
                f.write_str(" IS NULL");
            }
            Expr::IsNotNull(ast) => {
                f.write_node(&ast);
                f.write_str(" IS NOT NULL");
            }
            Expr::InList {
                expr,
                list,
                negated,
            } => {
                f.write_node(&expr);
                f.write_str(" ");
                if *negated {
                    f.write_str("NOT ");
                }
                f.write_str("IN (");
                f.write_node(&display::comma_separated(list));
                f.write_str(")");
            }
            Expr::InSubquery {
                expr,
                subquery,
                negated,
            } => {
                f.write_node(&expr);
                f.write_str(" ");
                if *negated {
                    f.write_str("NOT ");
                }
                f.write_str("IN (");
                f.write_node(&subquery);
                f.write_str(")");
            }
            Expr::Between {
                expr,
                negated,
                low,
                high,
            } => {
                f.write_node(&expr);
                if *negated {
                    f.write_str(" NOT");
                }
                f.write_str(" BETWEEN ");
                f.write_node(&low);
                f.write_str(" AND ");
                f.write_node(&high);
            }
            Expr::BinaryOp { left, op, right } => {
                f.write_node(&left);
                f.write_str(" ");
                f.write_str(op);
                f.write_str(" ");
                f.write_node(&right);
            }
            Expr::UnaryOp { op, expr } => {
                f.write_str(op);
                f.write_str(" ");
                f.write_node(&expr);
            }
            Expr::Cast { expr, data_type } => {
                // We are potentially rewriting an expression like
                //     CAST(<expr> OP <expr> AS <type>)
                // to
                //     <expr> OP <expr>::<type>
                // which could incorrectly change the meaning of the expression
                // as the `::` binds tightly. To be safe, we wrap the inner
                // expression in parentheses
                //    (<expr> OP <expr>)::<type>
                // unless the inner expression is of a type that we know is
                // safe to follow with a `::` to without wrapping.
                let needs_wrap = match **expr {
                    Expr::Nested(_)
                    | Expr::Value(_)
                    | Expr::Cast { .. }
                    | Expr::Function { .. }
                    | Expr::Identifier { .. }
                    | Expr::Extract { .. }
                    | Expr::Trim { .. }
                    | Expr::Collate { .. }
                    | Expr::Coalesce { .. } => false,
                    _ => true,
                };
                if needs_wrap {
                    f.write_str('(');
                }
                f.write_node(&expr);
                if needs_wrap {
                    f.write_str(')');
                }
                f.write_str("::");
                f.write_node(data_type);
            }
            Expr::Extract { field, expr } => {
                f.write_str("EXTRACT(");
                f.write_node(&display::escape_single_quote_string(field));
                f.write_str(" FROM ");
                f.write_node(&expr);
                f.write_str(")");
            }
            Expr::Trim { side, exprs } => {
                f.write_node(side);
                f.write_str("(");
                f.write_node(&exprs[0]);
                if exprs.len() == 2 {
                    f.write_str(", ");
                    f.write_node(&exprs[1]);
                }
                f.write_str(")");
            }
            Expr::Collate { expr, collation } => {
                f.write_node(&expr);
                f.write_str(" COLLATE ");
                f.write_node(&collation);
            }
            Expr::Coalesce { exprs } => {
                f.write_str("COALESCE(");
                f.write_node(&display::comma_separated(&exprs));
                f.write_str(")");
            }
            Expr::Nested(ast) => {
                f.write_str("(");
                f.write_node(&ast);
                f.write_str(")");
            }
            Expr::Row { exprs } => {
                f.write_str("ROW(");
                f.write_node(&display::comma_separated(&exprs));
                f.write_str(")");
            }
            Expr::Value(v) => {
                f.write_node(v);
            }
            Expr::Function(fun) => {
                f.write_node(fun);
            }
            Expr::Case {
                operand,
                conditions,
                results,
                else_result,
            } => {
                f.write_str("CASE");
                if let Some(operand) = operand {
                    f.write_str(" ");
                    f.write_node(&operand);
                }
                for (c, r) in conditions.iter().zip(results) {
                    f.write_str(" WHEN ");
                    f.write_node(c);
                    f.write_str(" THEN ");
                    f.write_node(r);
                }

                if let Some(else_result) = else_result {
                    f.write_str(" ELSE ");
                    f.write_node(&else_result);
                }
                f.write_str(" END")
            }
            Expr::Exists(s) => {
                f.write_str("EXISTS (");
                f.write_node(&s);
                f.write_str(")");
            }
            Expr::Subquery(s) => {
                f.write_str("(");
                f.write_node(&s);
                f.write_str(")");
            }
            Expr::Any {
                left,
                op,
                right,
                some,
            } => {
                f.write_node(&left);
                f.write_str(" ");
                f.write_str(op);
                if *some {
                    f.write_str(" SOME ");
                } else {
                    f.write_str(" ANY ");
                }
                f.write_str("(");
                f.write_node(&right);
                f.write_str(")");
            }
            Expr::All { left, op, right } => {
                f.write_node(&left);
                f.write_str(" ");
                f.write_str(op);
                f.write_str(" ALL (");
                f.write_node(&right);
                f.write_str(")");
            }
            Expr::List(exprs) => {
                let mut exprs = exprs.iter().peekable();
                f.write_str("LIST[");
                while let Some(expr) = exprs.next() {
                    f.write_node(expr);
                    if exprs.peek().is_some() {
                        f.write_str(", ");
                    }
                }
                f.write_str("]");
            }
        }
    }
}
impl_display!(Expr);

impl Expr {
    pub fn is_string_literal(&self) -> bool {
        if let Expr::Value(Value::String(_)) = self {
            true
        } else {
            false
        }
    }
}

/// A window specification (i.e. `OVER (PARTITION BY .. ORDER BY .. etc.)`)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowSpec {
    pub partition_by: Vec<Expr>,
    pub order_by: Vec<OrderByExpr>,
    pub window_frame: Option<WindowFrame>,
}

impl AstDisplay for WindowSpec {
    fn fmt(&self, f: &mut AstFormatter) {
        let mut delim = "";
        if !self.partition_by.is_empty() {
            delim = " ";
            f.write_str("PARTITION BY ");
            f.write_node(&display::comma_separated(&self.partition_by));
        }
        if !self.order_by.is_empty() {
            f.write_str(delim);
            delim = " ";
            f.write_str("ORDER BY ");
            f.write_node(&display::comma_separated(&self.order_by));
        }
        if let Some(window_frame) = &self.window_frame {
            if let Some(end_bound) = &window_frame.end_bound {
                f.write_str(delim);
                f.write_node(&window_frame.units);
                f.write_str(" BETWEEN ");
                f.write_node(&window_frame.start_bound);
                f.write_str(" AND ");
                f.write_node(&*end_bound);
            } else {
                f.write_str(delim);
                f.write_node(&window_frame.units);
                f.write_str(" ");
                f.write_node(&window_frame.start_bound);
            }
        }
    }
}
impl_display!(WindowSpec);

/// Specifies the data processed by a window function, e.g.
/// `RANGE UNBOUNDED PRECEDING` or `ROWS BETWEEN 5 PRECEDING AND CURRENT ROW`.
///
/// Note: The parser does not validate the specified bounds; the caller should
/// reject invalid bounds like `ROWS UNBOUNDED FOLLOWING` before execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowFrame {
    pub units: WindowFrameUnits,
    pub start_bound: WindowFrameBound,
    /// The right bound of the `BETWEEN .. AND` clause. The end bound of `None`
    /// indicates the shorthand form (e.g. `ROWS 1 PRECEDING`), which must
    /// behave the same as `end_bound = WindowFrameBound::CurrentRow`.
    pub end_bound: Option<WindowFrameBound>,
    // TBD: EXCLUDE
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowFrameUnits {
    Rows,
    Range,
    Groups,
}

impl AstDisplay for WindowFrameUnits {
    fn fmt(&self, f: &mut AstFormatter) {
        f.write_str(match self {
            WindowFrameUnits::Rows => "ROWS",
            WindowFrameUnits::Range => "RANGE",
            WindowFrameUnits::Groups => "GROUPS",
        })
    }
}
impl_display!(WindowFrameUnits);

/// Specifies [WindowFrame]'s `start_bound` and `end_bound`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowFrameBound {
    /// `CURRENT ROW`
    CurrentRow,
    /// `<N> PRECEDING` or `UNBOUNDED PRECEDING`
    Preceding(Option<u64>),
    /// `<N> FOLLOWING` or `UNBOUNDED FOLLOWING`.
    Following(Option<u64>),
}

impl AstDisplay for WindowFrameBound {
    fn fmt(&self, f: &mut AstFormatter) {
        match self {
            WindowFrameBound::CurrentRow => f.write_str("CURRENT ROW"),
            WindowFrameBound::Preceding(None) => f.write_str("UNBOUNDED PRECEDING"),
            WindowFrameBound::Following(None) => f.write_str("UNBOUNDED FOLLOWING"),
            WindowFrameBound::Preceding(Some(n)) => {
                f.write_str(n);
                f.write_str(" PRECEDING");
            }
            WindowFrameBound::Following(Some(n)) => {
                f.write_str(n);
                f.write_str(" FOLLOWING");
            }
        }
    }
}
impl_display!(WindowFrameBound);

/// A function call
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Function {
    pub name: ObjectName,
    pub args: FunctionArgs,
    // aggregate functions may specify e.g. `COUNT(DISTINCT X) FILTER (WHERE ...)`
    pub filter: Option<Box<Expr>>,
    pub over: Option<WindowSpec>,
    // aggregate functions may specify eg `COUNT(DISTINCT x)`
    pub distinct: bool,
}

impl AstDisplay for Function {
    fn fmt(&self, f: &mut AstFormatter) {
        f.write_node(&self.name);
        f.write_str("(");
        if self.distinct {
            f.write_str("DISTINCT ")
        }
        f.write_node(&self.args);
        f.write_str(")");
        if let Some(filter) = &self.filter {
            f.write_str(" FILTER (WHERE ");
            f.write_node(&filter);
            f.write_str(")");
        }
        if let Some(o) = &self.over {
            f.write_str(" OVER (");
            f.write_node(o);
            f.write_str(")");
        }
    }
}
impl_display!(Function);

/// Arguments for a function call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FunctionArgs {
    /// The special star argument, as in `count(*)`.
    Star,
    /// A normal list of arguments.
    Args(Vec<Expr>),
}

impl AstDisplay for FunctionArgs {
    fn fmt(&self, f: &mut AstFormatter) {
        match self {
            FunctionArgs::Star => f.write_str("*"),
            FunctionArgs::Args(args) => f.write_node(&display::comma_separated(&args)),
        }
    }
}
impl_display!(FunctionArgs);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// Expresses which side you want to trim characters from in `trim` function
/// calls.
pub enum TrimSide {
    /// Equivalent to `trim`
    Both,
    /// Equivalent to `ltrim`
    Leading,
    /// Equivalent to `rtrim`
    Trailing,
}

impl AstDisplay for TrimSide {
    fn fmt(&self, f: &mut AstFormatter) {
        match self {
            TrimSide::Both => f.write_str("btrim"),
            TrimSide::Leading => f.write_str("ltrim"),
            TrimSide::Trailing => f.write_str("rtrim"),
        }
    }
}
impl_display!(TrimSide);
