// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The parser: query text to a syntax tree with spans, plus the tokens for
//! highlighting. It reads the grammar only; whether a field exists or a
//! value fits is checked afterwards ([`crate::check`]).
//!
//! The text is first split into pieces at spaces and parentheses (quoted
//! text stays whole, spaces and all); each piece is then a keyword, a
//! condition (`field` `op` `values`), `sort:`, a task key or free text.
//! Precedence: `NOT` and `-` bind tightest, then `AND` (or a plain space),
//! then `OR`.

use crate::error::QueryError;
use crate::syntax::{
    Expr, KeyTerm, Negation, Op, Predicate, Query, SortClause, SortKey, Span, TextTerm, Token,
    TokenKind, Value, ValueKind,
};

/// The longest query read, in bytes.
pub const MAX_QUERY_BYTES: usize = 8192;
/// The deepest nesting of parentheses and negations.
pub const MAX_DEPTH: usize = 32;

/// Parses `src` into a query.
///
/// # Errors
/// The first mistake in the text.
pub fn parse(src: &str) -> Result<Query, QueryError> {
    parse_with_tokens(src).0
}

/// Parses `src`, also returning the tokens read up to the first mistake.
pub fn parse_with_tokens(src: &str) -> (Result<Query, QueryError>, Vec<Token>) {
    if src.len() > MAX_QUERY_BYTES {
        let error = QueryError::new("query-too-long", Span::new(MAX_QUERY_BYTES, src.len()))
            .arg("max", MAX_QUERY_BYTES.to_string());
        return (Err(error), Vec::new());
    }
    let lexemes = match lex(src) {
        Ok(l) => l,
        Err(e) => {
            return (
                Err(e.clone()),
                vec![Token {
                    kind: TokenKind::Error,
                    span: e.span,
                }],
            );
        }
    };
    let mut p = Parser {
        src,
        lexemes,
        pos: 0,
        tokens: Vec::new(),
        sort: None,
    };
    let result = p.query();
    let mut tokens = p.tokens;
    if let Err(e) = &result {
        tokens.retain(|t| t.span.end <= e.span.start || t.span.start >= e.span.end);
        tokens.push(Token {
            kind: TokenKind::Error,
            span: e.span,
        });
    }
    tokens.sort_by_key(|t| (t.span.start, t.span.end));
    (result, tokens)
}

/// A piece of a chunk: plain text, or text in quotes.
#[derive(Debug, Clone)]
enum Part {
    Bare(String, Span),
    Quoted(String, Span),
}

impl Part {
    fn span(&self) -> Span {
        match self {
            Self::Bare(_, s) | Self::Quoted(_, s) => *s,
        }
    }
}

/// Text between spaces and parentheses.
#[derive(Debug, Clone)]
struct Chunk {
    parts: Vec<Part>,
    span: Span,
}

impl Chunk {
    /// The chunk's text when it is one plain word.
    fn bare(&self) -> Option<&str> {
        match self.parts.as_slice() {
            [Part::Bare(t, _)] => Some(t),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
enum Lexeme {
    Open(Span),
    Close(Span),
    Chunk(Chunk),
}

impl Lexeme {
    fn span(&self) -> Span {
        match self {
            Self::Open(s) | Self::Close(s) => *s,
            Self::Chunk(c) => c.span,
        }
    }
}

fn lex(src: &str) -> Result<Vec<Lexeme>, QueryError> {
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    let mut parts: Vec<Part> = Vec::new();
    let mut bare = String::new();
    let mut bare_start = 0;
    let flush_bare = |parts: &mut Vec<Part>, bare: &mut String, start: usize, end: usize| {
        if !bare.is_empty() {
            parts.push(Part::Bare(std::mem::take(bare), Span::new(start, end)));
        }
    };
    let flush_chunk = |out: &mut Vec<Lexeme>, parts: &mut Vec<Part>| {
        if let (Some(first), Some(last)) = (parts.first(), parts.last()) {
            let span = first.span().join(last.span());
            out.push(Lexeme::Chunk(Chunk {
                parts: std::mem::take(parts),
                span,
            }));
        }
    };
    while let Some((i, c)) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                flush_bare(&mut parts, &mut bare, bare_start, i);
                flush_chunk(&mut out, &mut parts);
            }
            '(' | ')' => {
                flush_bare(&mut parts, &mut bare, bare_start, i);
                flush_chunk(&mut out, &mut parts);
                let span = Span::new(i, i + 1);
                out.push(if c == '(' {
                    Lexeme::Open(span)
                } else {
                    Lexeme::Close(span)
                });
            }
            '"' | '\'' => {
                flush_bare(&mut parts, &mut bare, bare_start, i);
                let mut text = String::new();
                let mut end = None;
                while let Some((j, d)) = chars.next() {
                    if d == '\\' {
                        match chars.peek() {
                            Some(&(_, e)) if e == c || e == '\\' => {
                                text.push(e);
                                chars.next();
                            }
                            _ => text.push(d),
                        }
                    } else if d == c {
                        end = Some(j + 1);
                        break;
                    } else {
                        text.push(d);
                    }
                }
                let Some(end) = end else {
                    return Err(QueryError::new(
                        "query-unclosed-quote",
                        Span::new(i, src.len()),
                    ));
                };
                parts.push(Part::Quoted(text, Span::new(i, end)));
            }
            c => {
                if bare.is_empty() {
                    bare_start = i;
                }
                bare.push(c);
            }
        }
    }
    flush_bare(&mut parts, &mut bare, bare_start, src.len());
    flush_chunk(&mut out, &mut parts);
    Ok(out)
}

struct Parser<'a> {
    src: &'a str,
    lexemes: Vec<Lexeme>,
    pos: usize,
    tokens: Vec<Token>,
    sort: Option<SortClause>,
}

impl Parser<'_> {
    fn token(&mut self, kind: TokenKind, span: Span) {
        self.tokens.push(Token { kind, span });
    }

    fn peek(&self) -> Option<&Lexeme> {
        self.lexemes.get(self.pos)
    }

    fn peek_keyword(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Lexeme::Chunk(c)) if c.bare() == Some(word))
    }

    /// Where the text ends, for errors at the end.
    fn end(&self) -> Span {
        Span::new(self.src.len(), self.src.len())
    }

    fn query(&mut self) -> Result<Query, QueryError> {
        if self.lexemes.is_empty() {
            return Ok(Query::default());
        }
        let (filter, branches) = self.or(0)?;
        if let Some(l) = self.peek() {
            return Err(match l {
                Lexeme::Close(s) => QueryError::new("query-unexpected-paren", *s),
                other => QueryError::new("query-condition-before", other.span())
                    .arg("keyword", other.span().of(self.src)),
            });
        }
        if let Some(sort) = &self.sort
            && branches > 1
        {
            return Err(QueryError::new("query-sort-placement", sort.span));
        }
        Ok(Query {
            filter,
            sort: self.sort.take(),
        })
    }

    /// `and { OR and }`; also returns the number of branches.
    fn or(&mut self, depth: usize) -> Result<(Option<Expr>, usize), QueryError> {
        let mut branches = vec![self.and(depth, None)?];
        while self.peek_keyword("OR") {
            let keyword = self.lexemes[self.pos].span();
            self.token(TokenKind::Keyword, keyword);
            self.pos += 1;
            branches.push(self.and(depth, Some(keyword))?);
        }
        let count = branches.len();
        // A branch that was only `sort:` is `None`; the caller reports it.
        let mut present: Vec<Expr> = branches.into_iter().flatten().collect();
        let expr = if count == 1 || present.is_empty() {
            present.pop()
        } else {
            Some(Expr::Or(present))
        };
        Ok((expr, count))
    }

    /// `unary { [AND] unary }`. `after` is the `OR` before it, if any.
    fn and(&mut self, depth: usize, after: Option<Span>) -> Result<Option<Expr>, QueryError> {
        let mut items = Vec::new();
        let mut saw_sort = false;
        loop {
            match self.peek() {
                None | Some(Lexeme::Close(_)) => break,
                Some(Lexeme::Chunk(c)) if c.bare() == Some("OR") => break,
                Some(Lexeme::Chunk(c)) if c.bare() == Some("AND") => {
                    let span = c.span;
                    if items.is_empty() && !saw_sort {
                        return Err(
                            QueryError::new("query-condition-before", span).arg("keyword", "AND")
                        );
                    }
                    self.token(TokenKind::Keyword, span);
                    self.pos += 1;
                    match self.peek() {
                        None | Some(Lexeme::Close(_)) => {
                            return Err(QueryError::new("query-condition-after", span)
                                .arg("keyword", "AND"));
                        }
                        Some(Lexeme::Chunk(c)) if matches!(c.bare(), Some("OR" | "AND")) => {
                            return Err(QueryError::new("query-condition-after", span)
                                .arg("keyword", "AND"));
                        }
                        _ => {}
                    }
                }
                Some(_) => match self.unary(depth)? {
                    Some(e) => items.push(e),
                    None => saw_sort = true,
                },
            }
        }
        if items.is_empty() && !saw_sort {
            return Err(match after {
                Some(or) => QueryError::new("query-condition-after", or).arg("keyword", "OR"),
                None => match self.peek() {
                    Some(Lexeme::Chunk(c)) => {
                        QueryError::new("query-condition-before", c.span).arg("keyword", "OR")
                    }
                    Some(Lexeme::Close(s)) => QueryError::new("query-unexpected-paren", *s),
                    _ => QueryError::new("query-empty-group", self.end()),
                },
            });
        }
        Ok(match items.len() {
            0 => None,
            1 => items.pop(),
            _ => Some(Expr::And(items)),
        })
    }

    /// `[NOT | -] atom`, or a group. `None` for `sort:`.
    fn unary(&mut self, depth: usize) -> Result<Option<Expr>, QueryError> {
        if depth >= MAX_DEPTH {
            let span = self.peek().map_or_else(|| self.end(), Lexeme::span);
            return Err(QueryError::new("query-too-deep", span));
        }
        let Some(lexeme) = self.peek().cloned() else {
            return Err(QueryError::new("query-empty-group", self.end()));
        };
        match lexeme {
            Lexeme::Close(s) => Err(QueryError::new("query-unexpected-paren", s)),
            Lexeme::Open(open) => {
                self.token(TokenKind::Paren, open);
                self.pos += 1;
                if let Some(Lexeme::Close(close)) = self.peek() {
                    return Err(QueryError::new("query-empty-group", open.join(*close)));
                }
                let (inner, _) = self.or(depth + 1)?;
                match self.peek() {
                    Some(Lexeme::Close(close)) => {
                        let close = *close;
                        self.token(TokenKind::Paren, close);
                        self.pos += 1;
                        let span = open.join(close);
                        match inner {
                            Some(e) => Ok(Some(Expr::Group(Box::new(e), span))),
                            None => Err(QueryError::new("query-empty-group", span)),
                        }
                    }
                    _ => Err(QueryError::new("query-unclosed-paren", open)),
                }
            }
            Lexeme::Chunk(chunk) => {
                if chunk.bare() == Some("NOT") {
                    self.token(TokenKind::Keyword, chunk.span);
                    self.pos += 1;
                    if self.peek().is_none()
                        || matches!(self.peek(), Some(Lexeme::Close(_)))
                        || self.peek_keyword("OR")
                        || self.peek_keyword("AND")
                    {
                        return Err(QueryError::new("query-condition-after", chunk.span)
                            .arg("keyword", "NOT"));
                    }
                    let inner = self.negated(depth, chunk.span)?;
                    let span = chunk.span.join(inner.span());
                    return Ok(Some(Expr::Not(Box::new(inner), Negation::Not, span)));
                }
                if let Some(minuses) = chunk.bare().filter(|b| b.bytes().all(|c| c == b'-')) {
                    // Minus signs right before a group negate it.
                    let count = minuses.len();
                    self.token(TokenKind::Negation, chunk.span);
                    self.pos += 1;
                    let adjacent = matches!(
                        self.peek(),
                        Some(Lexeme::Open(s)) if s.start == chunk.span.end
                    );
                    if !adjacent {
                        return Err(QueryError::new("query-nothing-to-negate", chunk.span));
                    }
                    if depth + count >= MAX_DEPTH {
                        return Err(QueryError::new("query-too-deep", chunk.span));
                    }
                    let mut expr = self.negated(depth + count - 1, chunk.span)?;
                    for i in (0..count).rev() {
                        let span = Span::new(chunk.span.start + i, expr.span().end);
                        expr = Expr::Not(Box::new(expr), Negation::Minus, span);
                    }
                    return Ok(Some(expr));
                }
                self.pos += 1;
                self.chunk(chunk, depth)
            }
        }
    }

    /// The expression after a negation; `sort:` can't be negated.
    fn negated(&mut self, depth: usize, negation: Span) -> Result<Expr, QueryError> {
        self.unary(depth + 1)?
            .ok_or_else(|| QueryError::new("query-sort-negated", negation))
    }

    /// One chunk: a negated chunk, `sort:`, a condition, a key or text.
    fn chunk(&mut self, chunk: Chunk, depth: usize) -> Result<Option<Expr>, QueryError> {
        if depth >= MAX_DEPTH {
            return Err(QueryError::new("query-too-deep", chunk.span));
        }
        if let Some(Part::Bare(first, span)) = chunk.parts.first()
            && let Some(rest) = first.strip_prefix('-')
        {
            let minus = Span::new(span.start, span.start + 1);
            self.token(TokenKind::Negation, minus);
            let mut parts = chunk.parts.clone();
            if rest.is_empty() {
                parts.remove(0);
            } else {
                parts[0] = Part::Bare(rest.to_owned(), Span::new(span.start + 1, span.end));
            }
            let Some(first) = parts.first() else {
                return Err(QueryError::new("query-nothing-to-negate", minus));
            };
            let inner = Chunk {
                span: Span::new(first.span().start, chunk.span.end),
                parts,
            };
            let inner = self
                .chunk(inner, depth + 1)?
                .ok_or_else(|| QueryError::new("query-sort-negated", minus))?;
            return Ok(Some(Expr::Not(
                Box::new(inner),
                Negation::Minus,
                chunk.span,
            )));
        }
        if let Some(Part::Bare(first, span)) = chunk.parts.first()
            && let Some((field_len, op, op_len)) = field_and_op(first)
        {
            let field = first[..field_len].to_owned();
            let field_span = Span::new(span.start, span.start + field_len);
            let op_span = Span::new(field_span.end, field_span.end + op_len);
            let mut rest = chunk.parts.clone();
            let after = &first[field_len + op_len..];
            if after.is_empty() {
                rest.remove(0);
            } else {
                rest[0] = Part::Bare(after.to_owned(), Span::new(op_span.end, span.end));
            }
            let values = self.values(&rest, &field, op, op_span)?;
            if field.eq_ignore_ascii_case("sort") {
                self.token(TokenKind::Sort, field_span.join(op_span));
                return self.sort(&values, chunk.span, depth).map(|()| None);
            }
            self.token(TokenKind::Field, field_span);
            self.token(TokenKind::Operator, op_span);
            for v in &values {
                self.token(TokenKind::Value, v.span);
            }
            return Ok(Some(Expr::Pred(Predicate {
                field,
                field_span,
                op,
                op_span,
                values,
                span: chunk.span,
            })));
        }
        match chunk.parts.as_slice() {
            [Part::Quoted(text, span)] => {
                self.token(TokenKind::Text, *span);
                Ok(Some(Expr::Text(TextTerm {
                    text: text.clone(),
                    quoted: true,
                    span: *span,
                })))
            }
            [Part::Bare(text, span)] => {
                if let Some((project, number)) = task_key(text) {
                    self.token(TokenKind::Key, *span);
                    return Ok(Some(Expr::Key(KeyTerm {
                        project,
                        number,
                        span: *span,
                    })));
                }
                self.token(TokenKind::Text, *span);
                Ok(Some(Expr::Text(TextTerm {
                    text: text.clone(),
                    quoted: false,
                    span: *span,
                })))
            }
            parts => {
                // Text and quotes run together: `abc"def"` or `"abc"def`.
                let bad = parts
                    .windows(2)
                    .find_map(|w| match w {
                        [Part::Bare(..), q @ Part::Quoted(..)] => {
                            Some(QueryError::new("query-unexpected-quote", q.span()))
                        }
                        [Part::Quoted(..), b] => {
                            Some(QueryError::new("query-text-after-quote", b.span()))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| QueryError::new("query-unexpected-quote", chunk.span));
                Err(bad)
            }
        }
    }

    /// The comma-separated values after an operator.
    fn values(
        &mut self,
        parts: &[Part],
        field: &str,
        op: Op,
        op_span: Span,
    ) -> Result<Vec<Value>, QueryError> {
        let missing = |span: Span| {
            QueryError::new("query-missing-value", span)
                .arg("field", field)
                .arg("op", op.as_str())
        };
        let mut values = Vec::new();
        // The value being read: its text, whether quoted, and its span.
        let mut current: Option<(String, bool, Span)> = None;
        let finish = |current: &mut Option<(String, bool, Span)>,
                      values: &mut Vec<Value>,
                      at: Span|
         -> Result<(), QueryError> {
            let Some((text, quoted, span)) = current.take() else {
                return Err(missing(at));
            };
            let kind = if quoted {
                ValueKind::Word { text, quoted }
            } else if let Some((a, b)) = text.split_once("..") {
                if a.is_empty() || b.is_empty() {
                    return Err(QueryError::new("query-empty-range-end", span));
                }
                ValueKind::Range(a.to_owned(), b.to_owned())
            } else {
                ValueKind::Word { text, quoted }
            };
            values.push(Value { kind, span });
            Ok(())
        };
        if parts.is_empty() {
            return Err(missing(op_span));
        }
        for part in parts {
            match part {
                Part::Quoted(text, span) => {
                    if current.is_some() {
                        return Err(QueryError::new("query-unexpected-quote", *span));
                    }
                    current = Some((text.clone(), true, *span));
                }
                Part::Bare(text, span) => {
                    let mut offset = span.start;
                    for (i, piece) in text.split(',').enumerate() {
                        if i > 0 {
                            let comma = Span::new(offset - 1, offset);
                            finish(&mut current, &mut values, comma)?;
                            self.tokens.push(Token {
                                kind: TokenKind::Comma,
                                span: comma,
                            });
                        }
                        let piece_span = Span::new(offset, offset + piece.len());
                        if !piece.is_empty() {
                            match &mut current {
                                Some((_, true, _)) => {
                                    return Err(QueryError::new(
                                        "query-text-after-quote",
                                        piece_span,
                                    ));
                                }
                                Some((t, false, s)) => {
                                    t.push_str(piece);
                                    *s = s.join(piece_span);
                                }
                                None => current = Some((piece.to_owned(), false, piece_span)),
                            }
                        }
                        offset += piece.len() + 1;
                    }
                }
            }
        }
        let end = parts.last().map_or(op_span, Part::span);
        finish(&mut current, &mut values, Span::new(end.end, end.end))?;
        Ok(values)
    }

    fn sort(&mut self, values: &[Value], span: Span, depth: usize) -> Result<(), QueryError> {
        if depth > 0 || self.sort.is_some() {
            return Err(QueryError::new("query-sort-placement", span));
        }
        let mut keys = Vec::with_capacity(values.len());
        for v in values {
            let text = v.text();
            let (descending, field) = match text.strip_prefix('-') {
                Some(f) => (true, f.to_owned()),
                None => (false, text),
            };
            if field.is_empty() {
                return Err(QueryError::new("query-sort-empty", v.span));
            }
            self.token(TokenKind::Sort, v.span);
            keys.push(SortKey {
                field,
                descending,
                span: v.span,
            });
        }
        self.sort = Some(SortClause { keys, span });
        Ok(())
    }
}

/// The length of a field name at the start of `text` and the operator after
/// it, with the operator's length.
fn field_and_op(text: &str) -> Option<(usize, Op, usize)> {
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let len = text
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
        .map_or(text.len(), |(i, _)| i);
    let rest = &text[len..];
    Op::ALL
        .iter()
        .find(|op| rest.starts_with(op.as_str()))
        .map(|op| (len, *op, op.as_str().len()))
}

/// The project key and number of a bare task key (`TM-41`): an upper-case
/// letter, then 1 to 9 upper-case letters or digits, a hyphen and digits.
#[must_use]
pub fn task_key(text: &str) -> Option<(String, u32)> {
    let (project, number) = text.split_once('-')?;
    let mut chars = project.chars();
    let first = chars.next()?;
    if !first.is_ascii_uppercase()
        || !(2..=10).contains(&project.len())
        || !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        || number.is_empty()
        || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let number: u32 = number.parse().ok()?;
    (number >= 1).then(|| (project.to_owned(), number))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(src: &str) -> String {
        parse(src).map_or_else(
            |e| format!("error {}: {}", e.id, e.message()),
            |q| q.shape(),
        )
    }

    #[test]
    fn simple_conditions_and_text() {
        assert_eq!(
            shape("is:open due<+7d"),
            r#"(and (pred "is" : ["open"]) (pred "due" < ["+7d"]))"#
        );
        assert_eq!(
            shape(r#"label:bug,crash -status:Done updated>-3d "login error""#),
            r#"(and (pred "label" : ["bug" "crash"]) (not (pred "status" : ["Done"])) (pred "updated" > ["-3d"]) (text "login error" true))"#
        );
        assert_eq!(shape("TM-41"), "(key TM-41)");
        assert_eq!(shape("\"TM-41\""), r#"(text "TM-41" true)"#);
        assert_eq!(
            shape("rock and roll"),
            r#"(and (text "rock" false) (text "and" false) (text "roll" false))"#
        );
        assert_eq!(shape(""), "()");
        assert_eq!(shape("   "), "()");
    }

    #[test]
    fn precedence_not_then_and_then_or() {
        assert_eq!(
            shape("a b OR c"),
            r#"(or (and (text "a" false) (text "b" false)) (text "c" false))"#
        );
        assert_eq!(
            shape("(label:bug OR type:Bug) is:open -is:blocked"),
            r#"(and (group (or (pred "label" : ["bug"]) (pred "type" : ["Bug"]))) (pred "is" : ["open"]) (not (pred "is" : ["blocked"])))"#
        );
        assert_eq!(
            shape("NOT a AND b"),
            r#"(and (not (text "a" false)) (text "b" false))"#
        );
        assert_eq!(
            shape("-(a OR b)"),
            r#"(not (group (or (text "a" false) (text "b" false))))"#
        );
        assert_eq!(shape("--a"), r#"(not (not (text "a" false)))"#);
    }

    #[test]
    fn values_ranges_and_quotes() {
        assert_eq!(
            shape("estimate:1h..4h -has:start"),
            r#"(and (pred "estimate" : ["1h".."4h"]) (not (pred "has" : ["start"])))"#
        );
        assert_eq!(
            shape(r#"status:"in progress",'Review'"#),
            r#"(pred "status" : ["in progress" "Review"])"#
        );
        assert_eq!(
            shape(r#"title:"say \"hi\"""#),
            r#"(pred "title" : ["say \"hi\""])"#
        );
        assert_eq!(shape("priority>=High"), r#"(pred "priority" >= ["High"])"#);
        assert_eq!(shape("due!=today"), r#"(pred "due" != ["today"])"#);
    }

    #[test]
    fn sort_is_taken_out_of_the_filter() {
        assert_eq!(
            shape("type:Feature priority>=High sort:due,-priority"),
            r#"(and (pred "type" : ["Feature"]) (pred "priority" >= ["High"])) (sort due,-priority)"#
        );
        assert_eq!(shape("sort:due"), "() (sort due)");
    }

    #[test]
    fn mistakes_point_at_their_place() {
        let err = |src: &str| parse(src).expect_err(src);
        let e = err("(a sort:due)");
        assert_eq!(e.id, "query-sort-placement");
        assert_eq!(e.span.of("(a sort:due)"), "sort:due");
        assert_eq!(err("a OR sort:due").id, "query-sort-placement");
        assert_eq!(err("sort:due sort:key").id, "query-sort-placement");
        assert_eq!(err("(a b").id, "query-unclosed-paren");
        assert_eq!(err("a b)").id, "query-unexpected-paren");
        assert_eq!(err("()").id, "query-empty-group");
        assert_eq!(err("\"abc").id, "query-unclosed-quote");
        assert_eq!(err("status:").id, "query-missing-value");
        assert_eq!(err("label:a,,b").id, "query-missing-value");
        assert_eq!(err("label:a,").id, "query-missing-value");
        assert_eq!(err("OR a").id, "query-condition-before");
        assert_eq!(err("a OR").id, "query-condition-after");
        assert_eq!(err("a AND").id, "query-condition-after");
        assert_eq!(err("NOT").id, "query-condition-after");
        assert_eq!(err("- a").id, "query-nothing-to-negate");
        assert_eq!(err("-").id, "query-nothing-to-negate");
        assert_eq!(err("-sort:due").id, "query-sort-placement");
        assert_eq!(err("NOT sort:due").id, "query-sort-placement");
        assert_eq!(err("abc\"def\"").id, "query-unexpected-quote");
        assert_eq!(err("estimate:..4h").id, "query-empty-range-end");
        assert_eq!(err(&"(".repeat(100)).id, "query-too-deep");
        assert_eq!(err(&"a ".repeat(5000)).id, "query-too-long");
    }

    #[test]
    fn tokens_cover_the_parts() {
        let src = "-label:bug,crash (a OR \"b c\") sort:due";
        let (q, tokens) = parse_with_tokens(src);
        assert!(q.is_ok());
        let kinds: Vec<(&str, &str)> = tokens
            .iter()
            .map(|t| (t.kind.as_str(), t.span.of(src)))
            .collect();
        assert_eq!(
            kinds,
            [
                ("negation", "-"),
                ("field", "label"),
                ("operator", ":"),
                ("value", "bug"),
                ("comma", ","),
                ("value", "crash"),
                ("paren", "("),
                ("text", "a"),
                ("keyword", "OR"),
                ("text", "\"b c\""),
                ("paren", ")"),
                ("sort", "sort:"),
                ("sort", "due"),
            ]
        );
    }

    #[test]
    fn task_keys() {
        assert_eq!(task_key("TM-41"), Some(("TM".to_owned(), 41)));
        assert_eq!(task_key("A1B-7"), Some(("A1B".to_owned(), 7)));
        assert_eq!(task_key("tm-41"), None);
        assert_eq!(task_key("T-41"), None);
        assert_eq!(task_key("TM-0"), None);
        assert_eq!(task_key("TM-"), None);
        assert_eq!(task_key("TM-99999999999"), None);
    }
}
