//! Shell formatting after shfmt's default layout: one statement per line, bodies indented one
//! level, `then`/`do` on the line of their head, case patterns at the `case` column and their
//! bodies indented. A compound command written on one line stays on one line. Words, strings,
//! expansions, substitutions and tests are kept as written; only the layout between them changes.
//!
//! concept: block-and-statements, line-continuation-layout

use crate::generated::kind as k;
use core_config::shared;
use core_format::shapes::continuation_flow::{OperatorFlow, embedded_statement};
use core_format::shapes::dangling_comments;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};
use std::borrow::Cow;

mod arith;
#[cfg(test)]
mod tests;
mod words;

include!("schema.generated.rs");

pub struct ShellFormatter;

impl NodeFormatter for ShellFormatter {
    /// The blanks inside `$(( ))` may change, and the leading tabs of the lines of a `<<-`
    /// heredoc (which it ignores), nothing else but trivia.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) if matches!(t.kind(), k::HEREDOC_BODY | k::HEREDOC_END) => {
                core_format::shapes::continuation::heredoc_value(t, k::HEREDOC_OP, "<<-")
            }
            SyntaxElement::Token(t) => t.text().split_whitespace().collect::<String>().into(),
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn comment_style(
        &self,
        options: &core_config::ResolvedOptions,
    ) -> core_format::comments::CommentStyle {
        core_format::comments::CommentStyle::column_option(
            options,
            core_config::opt::COMMENT_COLUMN,
        )
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::PROGRAM => program(cx, node)?,
            k::COMMAND | k::PIPELINE | k::LIST | k::REDIRECTED | k::NEGATED => flow(cx, node)?,
            k::IF
            | k::ELIF
            | k::ELSE
            | k::DO_GROUP
            | k::GROUP
            | k::SUBSHELL
            | k::WHILE
            | k::FOR
            | k::C_FOR => compound(cx, node)?,
            k::CASE => case(cx, node)?,
            k::CASE_ITEM => case_item(cx, node)?,
            k::FUNCTION => function(cx, node)?,
            k::HEREDOC_REDIRECT => heredoc(cx, node)?,
            k::REDIRECT => words::redirect(cx, node)?,
            k::ASSIGN | k::TEST | k::STRING | k::CONCAT | k::SUBST | k::ARRAY => {
                words::words(cx, node)?
            }
            _ => return Ok(None),
        }))
    }
}

/// A significant child; `cont` when a line continuation (`\` and a line break) precedes it.
struct E {
    el: SyntaxElement,
    cont: bool,
}

impl E {
    fn tok(&self) -> Option<&SyntaxToken> {
        self.el.as_token()
    }
    /// The text of a keyword token.
    fn kw(&self) -> Option<&str> {
        self.tok()
            .filter(|t| t.kind() == k::KEYWORD)
            .map(|t| t.text())
    }
    /// The text of an operator token.
    fn op(&self) -> Option<&str> {
        self.tok()
            .filter(|t| matches!(t.kind(), k::OP | k::CASE_END))
            .map(|t| t.text())
    }
    fn is_kw(&self, words: &[&str]) -> bool {
        self.kw().is_some_and(|w| words.contains(&w))
    }
    fn is(&self, kind: core_syntax::SyntaxKind) -> bool {
        self.el.kind() == kind
    }
    fn first(&self, cx: &FormatCtx<'_>) -> Option<SyntaxToken> {
        match &self.el {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.first_token(n),
        }
    }
    fn last(&self, cx: &FormatCtx<'_>) -> Option<SyntaxToken> {
        match &self.el {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.last_token(n),
        }
    }
    fn doc(&self, cx: &FormatCtx<'_>) -> Result<Doc, FormatError> {
        match &self.el {
            SyntaxElement::Token(t) if t.kind() == k::ARITH => Ok(doc::concat(vec![
                cx.leading_comments_doc(t),
                core_format::context::verbatim_text(&unpadded(t.text(), true)),
                cx.trailing_comments_doc(t),
            ])),
            SyntaxElement::Token(t) => Ok(cx.token(t)),
            SyntaxElement::Node(n) => cx.format(n),
        }
    }
}

/// The kinds and operators of a statement's chain of flows (see [`OperatorFlow`]).
const FLOW: OperatorFlow<'static> = OperatorFlow {
    flows: &[k::COMMAND, k::PIPELINE, k::LIST, k::REDIRECTED, k::NEGATED],
    leaf: k::COMMAND,
    pair: &[k::LIST, k::REDIRECTED],
    operator_kinds: &[k::OP, k::CASE_END],
    operators: &["&&", "||", "|", "|&"],
    pipe: k::PIPELINE,
    lead_operators: &["&&", "||"],
    unglued: &[k::REDIRECT, k::HEREDOC_REDIRECT],
    blocks: &[
        k::IF,
        k::WHILE,
        k::FOR,
        k::C_FOR,
        k::CASE,
        k::SUBSHELL,
        k::GROUP,
        k::FUNCTION,
    ],
    whitespace: k::WHITESPACE,
};

/// The significant children; `cont` when a line continuation precedes the child.
fn elems(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<E> {
    FLOW.items_with(cx, node, |el, cont| E { el, cont })
}

/// Is there a line break (not a continuation) between `t` and the next significant token?
fn breaks(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    FLOW.breaks(cx, t)
}

/// How many of the comments in front of a keyword that ends a body stand deeper than the
/// keyword: those belong to the body, the others to the keyword.
fn deeper_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> usize {
    let cs = comments::leading_comments(cx.grammar, t);
    comments::indented_run(&cs, comments::column(t) + 1)
}

/// A keyword that ends a body (`fi`, `}`, `;;`, `else`): with the comments in front of it that
/// do not belong to the body (see [`dangling`]) and its trailing comments.
fn kw(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    doc::concat(vec![kw_lead(cx, t), cx.trailing_comments_doc(t)])
}

/// [`kw`] without the trailing comments.
fn kw_lead(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    let cs = comments::leading_comments(cx.grammar, t);
    doc::concat(vec![
        core_format::context::leading_comments_doc(&cs[deeper_comments(cx, t)..]),
        cx.token_text(t),
    ])
}

/// Is the code the shell text of a Dockerfile instruction (`EmbedMode::Continued`)? Its line breaks
/// are line continuations: a break between statements is one, and every `;` stays, because without
/// it the next statement would join the words of the one before.
fn continued_mode(cx: &FormatCtx<'_>) -> bool {
    core_format::EmbedMode::of(cx.options) == Some(core_format::EmbedMode::Continued)
}

/// The end of a line that a source continuation keeps: ` \`. In continued mode the printer
/// writes the continuation of every line break itself.
fn continuation(cx: &FormatCtx<'_>) -> Doc {
    if continued_mode(cx) {
        Doc::Nil
    } else {
        doc::lit(" \\")
    }
}

/// The blank lines kept between statements. In continued mode each blank line is a line of the
/// text (a `\` of its own): none is dropped.
fn max_blank(cx: &FormatCtx<'_>) -> usize {
    if continued_mode(cx) {
        usize::MAX
    } else {
        cx.options.max_blank_lines()
    }
}

/// Does the node span a line break, from its first to its last token?
fn is_block(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let mut n = node.clone();
    while matches!(n.kind(), k::ELIF | k::ELSE | k::DO_GROUP) {
        match n.parent() {
            Some(p) => n = p,
            None => break,
        }
    }
    let lines = match (cx.first_token(&n), cx.last_token(&n)) {
        (Some(a), Some(b)) => cx.has_line_break(
            &n,
            TextRange::new(a.text_range().start(), b.text_range().end()),
        ),
        _ => false,
    };
    // (in a Dockerfile's shell text a case item written on one line stays on it)
    lines || (!(continued_mode(cx) && n.kind() == k::CASE_ITEM) && multi_statement_body(cx, &n))
}

/// Does a compound command at or below `node` have a body of several statements?
fn multi_statement_body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    statements_in_body(cx, node) > 1 || node.children().any(|c| multi_statement_body(cx, &c))
}

/// The statements in the body of a compound command or case item (0 for other nodes).
fn statements_in_body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> usize {
    let opener: &[&str] = match node.kind() {
        k::IF | k::ELIF => &["then"],
        k::ELSE | k::DO_GROUP => &["else", "do"],
        k::GROUP => &["{"],
        k::SUBSHELL => &["("],
        k::CASE_ITEM => &[")"],
        _ => return 0,
    };
    let es = elems(cx, node);
    let Some(oi) = es.iter().position(|e| e.is_kw(opener)) else {
        return 0;
    };
    (es[oi + 1..])
        .iter()
        .take_while(|e| !(e.is(k::ELIF) || e.is(k::ELSE)))
        .filter(|e| e.el.as_node().is_some())
        .count()
}

fn program(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es: Vec<E> = elems(cx, node)
        .into_iter()
        .filter(|e| !e.is(k::EOF))
        .collect();
    // (a `;` after the last statement of a Dockerfile's shell text ends nothing, unless it
    // stands on a line of its own)
    let end = match es.last() {
        Some(e)
            if continued_mode(cx) && !e.cont && e.tok().is_some_and(|t| t.kind() == k::SEMI) =>
        {
            es.len() - 1
        }
        _ => es.len(),
    };
    let mut parts = vec![statements(cx, &es[..end], continued_mode(cx))?];
    let eof = node
        .last_token()
        .ok_or_else(|| FormatError::Shape("Program without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !es.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(core_format::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

/// Statements one per line, with the source's blank lines (up to `max_blank_lines`); `;` goes,
/// `&` stays after its statement.
fn block_list(cx: &FormatCtx<'_>, es: &[E]) -> Result<Doc, FormatError> {
    statements(cx, es, false)
}

/// [`block_list`]. With `program` (the shell text of a Dockerfile instruction) every line after
/// the first is one level in from the instruction: the statements after the first, and the first
/// unless it is a chain of commands, whose lines after its first operator break stand at the
/// level the chain itself gives them (see [`flow`]).
fn statements(cx: &FormatCtx<'_>, es: &[E], program: bool) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let max = max_blank(cx);
    let continued = continued_mode(cx);
    let mut parts = Vec::new();
    let mut first_stmt = true;
    for e in es {
        match e.tok() {
            Some(t) if t.kind() == k::SEMI => {
                if continued {
                    // (a `;` that stands on a line of its own stays there)
                    if e.cont {
                        let line =
                            doc::concat(vec![continuation(cx), doc::hardline(), doc::lit(";")]);
                        parts.push(if program { doc::indent(line) } else { line });
                    } else {
                        parts.push(doc::lit(";"));
                    }
                }
                parts.push(cx.trailing_comments_doc(t));
            }
            Some(t) if e.op() == Some("&") => parts.extend([doc::lit(" "), cx.token(t)]),
            _ => {
                let start = parts.len();
                if !first_stmt && continued && !e.cont {
                    parts.push(doc::lit(" "));
                } else if !first_stmt {
                    parts.push(doc::hardline());
                    if let Some(first) = e.first(cx) {
                        let blank = comments::blank_lines_before(g, &first).min(max);
                        parts.extend((0..blank).map(|_| doc::hardline()));
                    }
                }
                let natural = first_stmt
                    && e.el
                        .as_node()
                        .is_some_and(|n| FLOW.chain_led(n, |b| is_block(cx, b)));
                parts.push(e.doc(cx)?);
                first_stmt = false;
                if program {
                    let chunk = doc::concat(parts.split_off(start));
                    parts.push(embedded_statement(chunk, program, natural));
                }
            }
        }
    }
    Ok(doc::concat(parts))
}

/// Statements on one line: `a; b`, `a & b`; with `terminate` a `;` ends the last one.
fn inline_list(cx: &FormatCtx<'_>, es: &[E], terminate: bool) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let (mut any, mut amp) = (false, false);
    for e in es {
        match e.tok() {
            Some(t) if t.kind() == k::SEMI => parts.push(cx.trailing_comments_doc(t)),
            Some(t) if e.op() == Some("&") => {
                parts.extend([doc::lit(" "), cx.token(t)]);
                amp = true;
            }
            _ => {
                if any {
                    // No blank before leading comments: their line break would put it ahead of
                    // the previous statement's trailing comment, one more blank per pass.
                    let comments_follow = e
                        .first(cx)
                        .is_some_and(|t| !comments::leading_comments(cx.grammar, &t).is_empty());
                    parts.push(doc::lit(match (amp, comments_follow) {
                        (true, _) => " ",
                        (false, true) => ";",
                        (false, false) => "; ",
                    }));
                }
                parts.push(e.doc(cx)?);
                any = true;
                amp = false;
            }
        }
    }
    if terminate && any && !amp {
        parts.push(doc::lit(";"));
    }
    Ok(doc::concat(parts))
}

/// Elements joined by a space, with the layout of `a &&` + line break + `b` kept (the rest
/// indented one level) and line continuations kept.
fn flow(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let e_doc = |el: &SyntaxElement| {
        E {
            el: el.clone(),
            cont: false,
        }
        .doc(cx)
    };
    FLOW.layout(cx, node, &continuation(cx), &|n| is_block(cx, n), &e_doc)
}

/// `<<EOF` and what follows it on the line; the body and the delimiter line are printed at the
/// next line break, at column 0.
fn heredoc(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let mut glue = true;
    let mut digits = false;
    for e in elems(cx, node) {
        if e.is(k::HEREDOC_BODY) || e.is(k::HEREDOC_END) {
            continue;
        }
        // `3<<EOF`: the descriptor and the operator, and the operator and the delimiter, touch.
        if !glue && !(digits && e.is(k::HEREDOC_OP)) {
            parts.push(doc::lit(" "));
        }
        glue = e.is(k::HEREDOC_OP);
        digits = e
            .tok()
            .is_some_and(|t| t.text().bytes().all(|b| b.is_ascii_digit()));
        parts.push(e.doc(cx)?);
    }
    // The body starts on the line after the last element, with its indentation (blanks that
    // are content), and runs to the end of the delimiter line.
    let kids: Vec<SyntaxElement> = node.children_with_tokens().collect();
    let is_body = |e: &SyntaxElement| matches!(e.kind(), k::HEREDOC_BODY | k::HEREDOC_END);
    let first = kids.iter().position(is_body).unwrap_or(kids.len());
    let last = kids.iter().rposition(is_body).unwrap_or(0);
    let real = kids[..first]
        .iter()
        .rposition(|e| e.as_node().is_some() || !cx.grammar.is_trivia(e.kind()))
        .map_or(0, |i| i + 1);
    let from = (real..first)
        .find(|&i| kids[i].kind() == k::NEWLINE)
        .map_or(real, |i| i + 1);
    let raw: String = kids[from..=last]
        .iter()
        .map(|e| match e {
            SyntaxElement::Token(t) => t.text().to_string(),
            SyntaxElement::Node(n) => n.text().to_string(),
        })
        .collect();
    let op = elems(cx, node)
        .iter()
        .find(|e| e.is(k::HEREDOC_OP))
        .and_then(|e| e.tok().map(|t| t.text().to_string()))
        .unwrap_or_default();
    parts.push(core_format::shapes::continuation::heredoc_by_option(
        cx,
        &raw,
        (&op, "<<-"),
        core_config::opt::HEREDOC_INDENT,
    ));
    Ok(doc::concat(parts))
}

/// `(( a ))` is `((a))` and the operators inside are spaced like shfmt does ([`arith`]). A blank
/// before `))` goes only with `tail`: tree-sitter makes `++ ))` one token, whose text then must
/// stay.
fn unpadded(text: &str, tail: bool) -> String {
    let at = text.find("((").unwrap_or(0);
    match text[at..]
        .strip_prefix("((")
        .and_then(|t| t.strip_suffix("))"))
    {
        Some(inner)
            if !inner.contains('\n')
                && (at == 0 || matches!(text[..at].trim_end(), "for" | "$")) =>
        {
            let for_c = at > 0 && text[..at].trim_end() == "for";
            let rest = inner.trim_end_matches([' ', '\t']);
            let Some(spaced) = arith::respace(rest, for_c, &text[..at] == "$") else {
                let inner = inner.trim_start_matches([' ', '\t']);
                let inner = if tail {
                    inner.trim_end_matches([' ', '\t'])
                } else {
                    inner
                };
                return format!("{}(({inner}))", &text[..at]);
            };
            // An empty initializer keeps its blank: `for (( ; a; b))`.
            let pad = if for_c && spaced.starts_with(';') {
                " "
            } else {
                ""
            };
            let blanks = if tail { "" } else { &inner[rest.len()..] };
            format!("{}(({pad}{spaced}{blanks}))", &text[..at])
        }
        _ => text.to_string(),
    }
}

/// The source from `first` to `last` (`((...))`, or `for ((...))` without the `for`), unpadded.
fn arithmetic_range(cx: &FormatCtx<'_>, first: &SyntaxToken, last: &SyntaxToken) -> Doc {
    let span = TextRange::new(first.text_range().start(), last.text_range().end());
    let raw = first
        .parent_ancestors()
        .find(|a| a.text_range().contains_range(span))
        .map(|n| {
            let start = n.text_range().start();
            n.text()
                .slice(TextRange::new(span.start() - start, span.end() - start))
                .to_string()
        })
        .unwrap_or_default();
    doc::concat(vec![
        cx.leading_comments_doc(first),
        core_format::context::verbatim_text(&unpadded(&raw, last.text() == "))")),
        cx.trailing_comments_doc(last),
    ])
}

/// A compound command: head, opener, body, clauses, closer.
fn compound(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    let kind = node.kind();
    let arithmetic = kind == k::GROUP && !es.first().is_some_and(|e| e.is_kw(&["{"]));
    // A `for` list may continue over lines; elsewhere a continuation is kept by keeping the node.
    let continued = es.iter().enumerate().any(|(i, e)| {
        e.cont && (kind != k::FOR || i + 1 == es.len()) && !(continued_mode(cx) && kind != k::FOR)
    });
    if arithmetic && !continued {
        return Ok(match (cx.first_token(node), cx.last_token(node)) {
            (Some(a), Some(b)) => arithmetic_range(cx, &a, &b),
            _ => Doc::Nil,
        });
    }
    if arithmetic || continued {
        return Ok(cx.verbatim_node(node));
    }
    let block = is_block(cx, node);
    let (mid, closer) = match es.split_last() {
        Some((c, mid)) if c.is_kw(&["fi", "done", "}", ")"]) => (mid, Some(c)),
        _ => (&es[..], None),
    };

    // `while`, `for`: the head, then the `do` group.
    if matches!(kind, k::WHILE | k::FOR | k::C_FOR) {
        let Some((body, head)) = mid.split_last() else {
            return Ok(cx.verbatim_node(node));
        };
        return Ok(doc::concat(vec![
            head_doc(cx, kind, head)?,
            doc::lit("; "),
            body.doc(cx)?,
        ]));
    }

    let oi = mid
        .iter()
        .position(|e| e.is_kw(&["then", "do", "else", "{", "("]));
    let Some(oi) = oi else {
        return Ok(cx.verbatim_node(node));
    };
    let (head, opener) = (&mid[..oi], &mid[oi]);
    let rest = &mid[oi + 1..];
    let split = rest
        .iter()
        .position(|e| e.is(k::ELIF) || e.is(k::ELSE))
        .unwrap_or(rest.len());
    let (body, clauses) = rest.split_at(split);
    // (continued mode: the `;` before the `)` of a subshell is dropped, the `)` ends it)
    let body = match body.split_last() {
        Some((e, init)) if kind == k::SUBSHELL && continued_mode(cx) && e.is(k::SEMI) => init,
        _ => body,
    };

    let mut parts = Vec::new();
    if !head.is_empty() {
        parts.push(head_doc(cx, kind, head)?);
        parts.push(doc::lit("; "));
    }
    let open_tok = opener.tok().expect("an opener is a token");
    parts.push(if kind == k::ELSE {
        kw(cx, open_tok)
    } else {
        cx.token(open_tok)
    });
    let subshell = kind == k::SUBSHELL;
    if block {
        let after = clauses
            .first()
            .and_then(|c| c.first(cx))
            .or_else(|| closer.and_then(|c| c.tok().cloned()))
            .or_else(|| comments::next_significant(cx.grammar, &mid_last(cx, mid)));
        let mut inner = Vec::new();
        if !body.is_empty() {
            inner.extend([
                doc::hardline(),
                opening_blank(cx, body),
                block_list(cx, body)?,
            ]);
        }
        inner.push(dangling(cx, after, !body.is_empty()));
        parts.push(doc::indent(doc::concat(inner)));
        for c in clauses {
            parts.push(doc::hardline());
            parts.push(c.doc(cx)?);
        }
        if let Some(c) = closer.and_then(E::tok) {
            parts.push(doc::hardline());
            parts.push(kw(cx, c));
        }
    } else {
        // shfmt keeps the blanks between `(` and a statement that starts with `(` (`( (a) )`).
        let first = body.iter().find_map(|e| e.first(cx));
        let first_lparen = first.as_ref().is_some_and(|t| {
            t.text() == "("
                || t.text() == "!"
                    && comments::next_significant(cx.grammar, t).is_some_and(|n| n.text() == "(")
        });
        let last_rparen = body
            .iter()
            .rev()
            .find_map(|e| e.last(cx))
            .is_some_and(|t| t.text() == ")");
        if !body.is_empty() {
            if !subshell || first_lparen {
                parts.push(doc::lit(" "));
            }
            parts.push(inline_list(cx, body, !subshell)?);
        }
        for c in clauses {
            parts.push(doc::lit(" "));
            parts.push(c.doc(cx)?);
        }
        if let Some(c) = closer.and_then(E::tok) {
            if !subshell || first_lparen && last_rparen {
                parts.push(doc::lit(" "));
            }
            parts.push(kw(cx, c));
        }
    }
    Ok(doc::concat(parts))
}

fn mid_last(cx: &FormatCtx<'_>, mid: &[E]) -> SyntaxToken {
    mid.iter()
        .rev()
        .find_map(|e| e.last(cx))
        .expect("a compound has tokens")
}

/// The blank lines the source has between the opener of a body and its first statement.
fn opening_blank(cx: &FormatCtx<'_>, body: &[E]) -> Doc {
    let first = body.iter().find_map(|e| e.first(cx));
    let blank = first.map_or(0, |t| comments::blank_lines_before(cx.grammar, &t));
    doc::concat(
        (0..blank.min(max_blank(cx)))
            .map(|_| doc::hardline())
            .collect(),
    )
}

/// Comments in front of the token that ends a body, inside the body (when they are deeper than
/// the token), or else a blank line.
fn dangling(cx: &FormatCtx<'_>, after: Option<SyntaxToken>, blank: bool) -> Doc {
    let Some(t) = after else { return Doc::Nil };
    let cs = comments::leading_comments(cx.grammar, &t);
    let cs = &cs[..deeper_comments(cx, &t)];
    if cs.is_empty() {
        // A blank line before the keyword stays.
        let blank_line = blank && comments::blank_lines_before(cx.grammar, &t) > 0;
        return if blank_line {
            doc::hardline()
        } else {
            Doc::Nil
        };
    }
    dangling_comments(
        cs,
        blank && comments::blank_lines_before(cx.grammar, &t) > 0,
    )
}

/// `if cond`, `elif cond`, `while cond`, `for i in words`, `for ((...))`.
fn head_doc(
    cx: &FormatCtx<'_>,
    kind: core_syntax::SyntaxKind,
    head: &[E],
) -> Result<Doc, FormatError> {
    let Some((first, rest)) = head.split_first() else {
        return Ok(Doc::Nil);
    };
    let Some(first_tok) = first.tok() else {
        return Ok(Doc::Nil);
    };
    let words: Vec<&E> = head.iter().filter(|e| !e.is(k::SEMI)).collect();
    if kind == k::C_FOR {
        let (a, b) = (first.first(cx), words.last().and_then(|e| e.last(cx)));
        return Ok(match (a, b) {
            (Some(a), Some(b)) => arithmetic_range(cx, &a, &b),
            _ => Doc::Nil,
        });
    }
    let lead = if kind == k::ELIF {
        kw(cx, first_tok)
    } else {
        cx.token(first_tok)
    };
    if kind == k::FOR {
        let mut parts = vec![lead];
        let mut tail = Vec::new();
        for e in &words[1..] {
            let target = if tail.is_empty() && !e.cont {
                &mut parts
            } else {
                &mut tail
            };
            target.push(if e.cont {
                continuation(cx)
            } else {
                doc::lit(" ")
            });
            if e.cont {
                target.push(doc::hardline());
            }
            target.push(e.doc(cx)?);
        }
        parts.push(doc::indent(doc::concat(tail)));
        return Ok(doc::concat(parts));
    }
    Ok(doc::concat(vec![
        lead,
        doc::lit(" "),
        inline_list(cx, rest, false)?,
    ]))
}

fn case(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    if !continued_mode(cx) && es.iter().any(|e| e.cont) {
        return Ok(cx.verbatim_node(node));
    }
    let Some(ii) = es.iter().position(|e| e.is_kw(&["in"])) else {
        return Ok(cx.verbatim_node(node));
    };
    let (head, rest) = es.split_at(ii + 1);
    let (esac, items) = match rest.split_last() {
        Some((c, items)) if c.is_kw(&["esac"]) => (c, items),
        _ => return Ok(cx.verbatim_node(node)),
    };
    let g = cx.grammar;
    let mut parts = Vec::new();
    for (i, e) in head.iter().enumerate() {
        if i > 0 {
            parts.push(doc::lit(" "));
        }
        parts.push(e.doc(cx)?);
    }
    let block = is_block(cx, node);
    let mut arms = Vec::new();
    for e in items {
        if block {
            arms.push(doc::hardline());
            if let Some(first) = e.first(cx) {
                let blank = comments::blank_lines_before(g, &first).min(max_blank(cx));
                arms.extend((0..blank).map(|_| doc::hardline()));
            }
        } else {
            arms.push(doc::lit(" "));
        }
        arms.push(e.doc(cx)?);
    }
    if block && cx.options.bool(core_config::opt::INDENT_CASE_ARMS) {
        parts.push(doc::indent(doc::concat(arms)));
    } else {
        parts.extend(arms);
    }
    parts.push(if block {
        doc::hardline()
    } else {
        doc::lit(" ")
    });
    if block
        && !items.is_empty()
        && esac
            .first(cx)
            .is_some_and(|t| comments::blank_lines_before(g, &t) > 0)
    {
        parts.push(doc::hardline());
    }
    parts.push(esac.doc(cx)?);
    Ok(doc::concat(parts))
}

/// `a | b)`, the body, `;;`. On one line when the source has it on one line.
fn case_item(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    let Some(pi) = es.iter().position(|e| e.is_kw(&[")"])) else {
        return Ok(cx.verbatim_node(node));
    };
    if !continued_mode(cx) && es.iter().any(|e| e.cont) {
        return Ok(cx.verbatim_node(node));
    }
    if es[0].is_kw(&["("]) {
        return Ok(cx.verbatim_node(node));
    }
    let (pats, rest) = es.split_at(pi);
    let rest = &rest[1..];
    let (body, term) = match rest.split_last() {
        Some((t, body)) if t.is(k::CASE_END) => (body, Some(t)),
        _ => (rest, None),
    };
    // (continued mode: the `;` before `;;` is dropped, the arm ends there)
    let body = match body.split_last() {
        Some((e, init)) if term.is_some() && continued_mode(cx) && e.is(k::SEMI) => init,
        _ => body,
    };
    let mut parts = Vec::new();
    // Alternatives are separated by ` | `; the words of one pattern (`*"/.bashrc"`) touch.
    let mut after_bar = true;
    for e in pats {
        if e.op() == Some("|") {
            parts.push(doc::lit(" | "));
            after_bar = true;
            continue;
        }
        if !after_bar
            && e.first(cx).is_some_and(|t| {
                t.prev_token()
                    .is_some_and(|p| cx.grammar.is_trivia(p.kind()))
            })
        {
            parts.push(doc::lit(" "));
        }
        after_bar = false;
        parts.push(e.doc(cx)?);
    }
    parts.push(cx.token(es[pi].tok().expect("`)` is a token")));
    if is_block(cx, node) {
        let after = term.and_then(|t| t.tok().cloned());
        let mut inner = Vec::new();
        if !body.is_empty() {
            inner.extend([
                doc::hardline(),
                opening_blank(cx, body),
                block_list(cx, body)?,
            ]);
        }
        if term.is_some() {
            inner.push(dangling(cx, after, !body.is_empty()));
        }
        inner.extend(end(cx, term, true));
        parts.push(doc::indent(doc::concat(inner)));
    } else {
        if !body.is_empty() {
            parts.push(doc::lit(" "));
            parts.push(inline_list(cx, body, false)?);
        }
        parts.extend(end(cx, term, false));
    }
    Ok(doc::concat(parts))
}

/// The `;;` of a case arm, the one it has or the one `case_last_terminator` adds.
fn end(cx: &FormatCtx<'_>, term: Option<&E>, own_line: bool) -> Option<Doc> {
    core_format::shapes::continuation::item_terminator(
        cx,
        term.and_then(E::tok).map(|t| kw(cx, t)),
        (core_config::opt::CASE_LAST_TERMINATOR, k::CASE_END),
        own_line,
    )
}

/// `name() body` and `function name body`.
fn function(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    if !continued_mode(cx) && es.iter().any(|e| e.cont) {
        return Ok(cx.verbatim_node(node));
    }
    let mut parts = Vec::new();
    let mut glue = true;
    for e in &es {
        match e.kw() {
            Some("function") => {
                parts.push(e.doc(cx)?);
                parts.push(doc::lit(" "));
                glue = true;
            }
            Some(p @ ("(" | ")")) => {
                parts.push(doc::lit(if p == "(" { "(" } else { ")" }));
                glue = p == "(";
            }
            _ => {
                if !glue {
                    parts.push(doc::lit(" "));
                }
                parts.push(e.doc(cx)?);
                glue = false;
            }
        }
    }
    Ok(doc::concat(parts))
}
