// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! SPDX licence expressions, as far as the packages use them: licence ids
//! joined by `AND` and `OR`, with parentheses and `WITH` exceptions. The
//! older form `MIT/Apache-2.0` that some crates still write is read as `OR`.

use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Expr {
    /// One licence, with its exception if any (`Apache-2.0 WITH LLVM-exception`).
    Id(String),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let tokens = tokens(text);
        let mut parser = Parser { tokens, at: 0 };
        let expr = parser.or()?;
        if parser.at != parser.tokens.len() {
            bail!("unexpected `{}` in `{text}`", parser.tokens[parser.at]);
        }
        Ok(expr)
    }

    /// Whether the expression can be satisfied with the allowed licences:
    /// one side of every `OR`, both sides of every `AND`.
    pub(crate) fn allowed(&self, allowed: &dyn Fn(&str) -> bool) -> bool {
        match self {
            Self::Id(id) => allowed(id),
            Self::And(a, b) => a.allowed(allowed) && b.allowed(allowed),
            Self::Or(a, b) => a.allowed(allowed) || b.allowed(allowed),
        }
    }

    /// The licence ids, without their exceptions, in order and without
    /// repeats.
    pub(crate) fn licences(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<String>) {
        match self {
            Self::Id(id) => {
                let base = id.split(" WITH ").next().unwrap_or(id).to_owned();
                if !out.contains(&base) {
                    out.push(base);
                }
            }
            Self::And(a, b) | Self::Or(a, b) => {
                a.collect(out);
                b.collect(out);
            }
        }
    }
}

fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_whitespace() || matches!(c, '(' | ')' | '/') {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            match c {
                '(' | ')' => out.push(c.to_string()),
                '/' => out.push("OR".to_owned()),
                _ => {}
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

struct Parser {
    tokens: Vec<String>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.at).map(String::as_str)
    }

    fn next(&mut self) -> Option<String> {
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        token
    }

    fn or(&mut self) -> Result<Expr> {
        let mut left = self.and()?;
        while self.peek() == Some("OR") {
            self.at += 1;
            let right = self.and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr> {
        let mut left = self.atom()?;
        while self.peek() == Some("AND") {
            self.at += 1;
            let right = self.atom()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn atom(&mut self) -> Result<Expr> {
        match self.next().as_deref() {
            Some("(") => {
                let inner = self.or()?;
                if self.next().as_deref() != Some(")") {
                    bail!("a `(` is not closed");
                }
                Ok(inner)
            }
            Some(op @ (")" | "AND" | "OR" | "WITH")) => {
                bail!("a licence id is missing before `{op}`")
            }
            Some(id) => {
                if self.peek() == Some("WITH") {
                    self.at += 1;
                    match self.next() {
                        Some(exception)
                            if !matches!(exception.as_str(), "(" | ")" | "AND" | "OR") =>
                        {
                            Ok(Expr::Id(format!("{id} WITH {exception}")))
                        }
                        _ => bail!("an exception is missing after `WITH`"),
                    }
                } else {
                    Ok(Expr::Id(id.to_owned()))
                }
            }
            None => bail!("the expression ends early"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Expr;

    fn allowed(text: &str, list: &[&str]) -> bool {
        Expr::parse(text).unwrap().allowed(&|id| list.contains(&id))
    }

    #[test]
    fn choices_need_one_allowed_licence_and_combinations_all() {
        let list = ["MIT", "Apache-2.0", "Unicode-3.0"];
        assert!(allowed("MIT OR Apache-2.0", &list));
        assert!(allowed("EPL-2.0 OR MIT", &list));
        assert!(!allowed("EPL-2.0 OR SSPL-1.0", &list));
        assert!(allowed("(MIT OR Apache-2.0) AND Unicode-3.0", &list));
        assert!(!allowed("(MIT OR Apache-2.0) AND Unicode-DFS-2016", &list));
        assert!(allowed("MIT/Apache-2.0", &list));
        assert!(allowed("Apache-2.0 / MIT", &list));
        assert!(!allowed("Apache-2.0 WITH LLVM-exception", &list));
        assert!(allowed(
            "Apache-2.0 WITH LLVM-exception",
            &["Apache-2.0 WITH LLVM-exception"]
        ));
    }

    #[test]
    fn and_binds_tighter_than_or() {
        // MIT OR (Apache-2.0 AND EPL-2.0)
        assert!(allowed("MIT OR Apache-2.0 AND EPL-2.0", &["MIT"]));
        assert!(!allowed("MIT OR Apache-2.0 AND EPL-2.0", &["Apache-2.0"]));
    }

    #[test]
    fn licences_are_listed_once_without_exceptions() {
        let expr = Expr::parse("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT").unwrap();
        assert_eq!(expr.licences(), ["Apache-2.0", "MIT"]);
    }

    #[test]
    fn broken_expressions_are_errors() {
        for text in [
            "",
            "MIT OR",
            "(MIT",
            "MIT)",
            "AND MIT",
            "Apache-2.0 WITH",
            "MIT MIT",
        ] {
            assert!(Expr::parse(text).is_err(), "{text}");
        }
    }
}
