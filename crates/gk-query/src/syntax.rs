// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The syntax tree of a query. Every node keeps the byte span of its text,
//! for highlighting, error positions and editing one condition in place.

use std::fmt;

/// A byte range of the query text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// The first byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl Span {
    /// The span from `start` to `end`.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// The smallest span covering both.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// The text of this span in `src`.
    #[must_use]
    pub fn of(self, src: &str) -> &str {
        src.get(self.start..self.end).unwrap_or("")
    }
}

/// A comparison in a condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    /// `:` is one of, contains, on or within, equals.
    Is,
    /// `=` exactly.
    Eq,
    /// `!=` is not; includes tasks where the field is empty.
    Ne,
    /// `<` before or less than.
    Lt,
    /// `<=`.
    Le,
    /// `>` after or more than.
    Gt,
    /// `>=`.
    Ge,
}

impl Op {
    /// Every operator, longest spelling first (the order the parser tries).
    pub const ALL: [Self; 7] = [
        Self::Ne,
        Self::Le,
        Self::Ge,
        Self::Is,
        Self::Eq,
        Self::Lt,
        Self::Gt,
    ];

    /// How the operator is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Is => ":",
            Self::Eq => "=",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One value of a condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// What the value is.
    pub kind: ValueKind,
    /// Where it is.
    pub span: Span,
}

/// A value: a word, quoted text, or a range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKind {
    /// A word, or text in quotes (`quoted`), with escapes removed.
    Word {
        /// The text.
        text: String,
        /// Whether it was written in quotes.
        quoted: bool,
    },
    /// `low..high`.
    Range(String, String),
}

impl Value {
    /// The value's text; a range's two ends joined by `..`.
    #[must_use]
    pub fn text(&self) -> String {
        match &self.kind {
            ValueKind::Word { text, .. } => text.clone(),
            ValueKind::Range(a, b) => format!("{a}..{b}"),
        }
    }
}

/// A condition on a field: `label:bug,crash`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicate {
    /// The field as written (an alias stays an alias).
    pub field: String,
    /// Where the field name is.
    pub field_span: Span,
    /// The comparison.
    pub op: Op,
    /// Where the operator is.
    pub op_span: Span,
    /// One or more values; several mean "any of them".
    pub values: Vec<Value>,
    /// The whole condition.
    pub span: Span,
}

/// Free text to search for: a word or a phrase in quotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextTerm {
    /// The text, escapes removed.
    pub text: String,
    /// Whether it was written in quotes (a phrase; a key in quotes is text).
    pub quoted: bool,
    /// Where it is.
    pub span: Span,
}

/// A bare task key, `TM-41`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTerm {
    /// The project key.
    pub project: String,
    /// The task's number.
    pub number: u32,
    /// Where it is.
    pub span: Span,
}

/// How a negation was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Negation {
    /// A leading `-`.
    Minus,
    /// The keyword `NOT`.
    Not,
}

/// A filter expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// All of these (a space or `AND`).
    And(Vec<Expr>),
    /// Any of these (`OR`).
    Or(Vec<Expr>),
    /// Not this. The span covers the negation and the expression.
    Not(Box<Expr>, Negation, Span),
    /// An expression in parentheses.
    Group(Box<Expr>, Span),
    /// A condition on a field.
    Pred(Predicate),
    /// Free text.
    Text(TextTerm),
    /// A task key.
    Key(KeyTerm),
}

impl Expr {
    /// The span of the whole expression.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::And(items) | Self::Or(items) => items
                .iter()
                .map(Self::span)
                .reduce(Span::join)
                .unwrap_or_default(),
            Self::Not(_, _, span) | Self::Group(_, span) => *span,
            Self::Pred(p) => p.span,
            Self::Text(t) => t.span,
            Self::Key(k) => k.span,
        }
    }

    /// The expression's structure without spans, as an S-expression; two
    /// queries with the same shape mean the same.
    #[must_use]
    pub fn shape(&self) -> String {
        match self {
            Self::And(items) => format!(
                "(and {})",
                items.iter().map(Self::shape).collect::<Vec<_>>().join(" ")
            ),
            Self::Or(items) => format!(
                "(or {})",
                items.iter().map(Self::shape).collect::<Vec<_>>().join(" ")
            ),
            Self::Not(inner, _, _) => format!("(not {})", inner.shape()),
            Self::Group(inner, _) => format!("(group {})", inner.shape()),
            Self::Pred(p) => format!(
                "(pred {:?} {} [{}])",
                p.field,
                p.op,
                p.values
                    .iter()
                    .map(|v| match &v.kind {
                        ValueKind::Word { text, .. } => format!("{text:?}"),
                        ValueKind::Range(a, b) => format!("{a:?}..{b:?}"),
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Self::Text(t) => format!("(text {:?} {})", t.text, t.quoted),
            Self::Key(k) => format!("(key {}-{})", k.project, k.number),
        }
    }
}

/// One sort key: `due` or `-priority`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    /// The field as written.
    pub field: String,
    /// Descending.
    pub descending: bool,
    /// Where it is.
    pub span: Span,
}

/// The `sort:` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortClause {
    /// The keys, most important first.
    pub keys: Vec<SortKey>,
    /// The whole clause.
    pub span: Span,
}

/// A parsed query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    /// The filter; `None` matches every task.
    pub filter: Option<Expr>,
    /// The sort clause, if the query has one.
    pub sort: Option<SortClause>,
}

impl Query {
    /// The query's structure without spans.
    #[must_use]
    pub fn shape(&self) -> String {
        let filter = self
            .filter
            .as_ref()
            .map_or_else(|| "()".to_owned(), Expr::shape);
        match &self.sort {
            Some(s) => format!(
                "{filter} (sort {})",
                s.keys
                    .iter()
                    .map(|k| format!("{}{}", if k.descending { "-" } else { "" }, k.field))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            None => filter,
        }
    }
}

/// What a piece of the query text is, for highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// A field name.
    Field,
    /// An operator.
    Operator,
    /// A value.
    Value,
    /// A comma between values.
    Comma,
    /// `AND`, `OR` or `NOT`.
    Keyword,
    /// A leading `-`.
    Negation,
    /// `(` or `)`.
    Paren,
    /// Free text.
    Text,
    /// A task key.
    Key,
    /// `sort:` and its keys.
    Sort,
    /// Text the parser could not read.
    Error,
}

impl TokenKind {
    /// The name used in outputs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Operator => "operator",
            Self::Value => "value",
            Self::Comma => "comma",
            Self::Keyword => "keyword",
            Self::Negation => "negation",
            Self::Paren => "paren",
            Self::Text => "text",
            Self::Key => "key",
            Self::Sort => "sort",
            Self::Error => "error",
        }
    }
}

/// A highlighted piece of the query text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// What it is.
    pub kind: TokenKind,
    /// Where it is.
    pub span: Span,
}
