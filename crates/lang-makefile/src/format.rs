//! Makefile formatting: statements one per line with the source's blank lines, one space around
//! assignment operators and after rule colons, one tab before each recipe command, conditional
//! bodies indented. Values, recipe commands and `define` bodies are kept as written (trailing
//! blanks aside); recipe commands are Shell code, which the Shell language formats on its own.
//!
//! concept: block-and-statements, key-value-layout, line-continuation-layout

use crate::generated::kind as k;
use core_config::shared;
use core_format::shapes::continuation::{self, Continued};
use core_format::shapes::statement_sequence;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxToken, TextRange};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct MakefileFormatter;

impl NodeFormatter for MakefileFormatter {
    /// The formatter trims the blanks around values, prefixes and keywords.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            // `A+=b` is the name `A+` and `=` in the tree, `A += b` the name `A` and `+=`: both read the same
            SyntaxElement::Token(t) if t.kind() == k::WORD && glued_operator(t) => {
                t.text()[..t.text().len() - 1].to_string().into()
            }
            SyntaxElement::Token(t) if t.kind() == k::OP => {
                let carried = t
                    .prev_token()
                    .filter(|p| p.kind() == k::WORD && glued_operator(p))
                    .and_then(|p| p.text().chars().last());
                match carried {
                    Some(c) => format!("{c}{}", t.text()).into(),
                    None => t.text().trim().into(),
                }
            }
            SyntaxElement::Token(t) if t.kind() == k::VALUE => {
                continuation::condensed(t.text().trim()).into()
            }
            SyntaxElement::Token(t) => t.text().trim().into(),
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let kept = keeps_indent(cx, node)
            .then(|| cx.first_token(node))
            .flatten();
        let statement = || -> Result<Option<Doc>, FormatError> {
            Ok(Some(match node.kind() {
                k::DOCUMENT => document(cx, node)?,
                k::RULE => rule(cx, node)?,
                k::RECIPE_LINE => recipe_line(cx, node, false),
                k::CONDITIONAL => conditional(cx, node)?,
                k::DEFINE => define(cx, node),
                k::ASSIGN | k::DIRECTIVE | k::CALL => {
                    let mut line = Line::default();
                    relay(cx, node.children_with_tokens(), &mut line);
                    line.finish()
                }
                _ => return Ok(None),
            }))
        };
        match kept {
            Some(first) => cx
                .at_source_indent(&first, || Ok(statement()?.unwrap_or(Doc::Nil)))
                .map(Some),
            None => statement(),
        }
    }
}

/// Does a line of this node start with the blanks of the source (`conditional_indent: keep`)? So
/// do the lines of a conditional and of what stands in its body, outside a recipe.
fn keeps_indent(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    if conditional_indent(cx.options) != ConditionalIndent::Keep
        || !matches!(
            node.kind(),
            k::RULE | k::CONDITIONAL | k::DEFINE | k::ASSIGN | k::DIRECTIVE | k::CALL
        )
    {
        return false;
    }
    if node.kind() == k::CONDITIONAL && !in_recipe(node) {
        return true;
    }
    for a in node.ancestors().skip(1) {
        match a.kind() {
            k::RECIPE | k::DOCUMENT => return false,
            k::CONDITIONAL | k::ELSE => return true,
            _ => {}
        }
    }
    false
}

fn in_recipe(node: &SyntaxNode) -> bool {
    node.ancestors().skip(1).any(|a| a.kind() == k::RECIPE)
}

/// The directive line `first` starts, at the blanks of the source when the conditional keeps them.
fn kept_line(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    first: &SyntaxToken,
    line: impl FnOnce() -> Doc,
) -> Doc {
    if conditional_indent(cx.options) == ConditionalIndent::Keep && !in_recipe(node) {
        cx.at_source_indent(first, || Ok::<_, FormatError>(line()))
            .unwrap_or(Doc::Nil)
    } else {
        line()
    }
}

fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence(
        cx,
        &items,
        cx.options.max_blank_lines(),
    )?];
    // The comments at the end of the file belong to the end-of-file token.
    let eof = node
        .last_token()
        .ok_or_else(|| FormatError::Shape("Document without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !items.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(core_format::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

/// One logical line: the tokens up to the first line continuation, then the indented rest.
#[derive(Default)]
struct Line {
    out: Continued,
    prev: Option<SyntaxKind>,
    blank: bool,
    /// The grammar reads `A+=b` as the name `A+` and `=`: the `+` goes to the operator.
    carry: Option<char>,
}

impl Line {
    fn push(&mut self, d: Doc) {
        self.out.push(d);
    }

    fn finish(self) -> Doc {
        self.out.finish()
    }
}

/// Appends the elements to the line: blanks collapse to one space where the source has any, and
/// the style options set the spaces around colons and assignment operators.
fn relay(cx: &FormatCtx<'_>, elements: impl Iterator<Item = SyntaxElement>, line: &mut Line) {
    for e in elements {
        let t = match e {
            SyntaxElement::Node(n) => {
                match n.kind() {
                    k::RECIPE | k::CONDITIONAL | k::ELSE => {}
                    k::DEFINE => match cx.format(&n) {
                        Ok(d) => line.push(d),
                        Err(_) => line.push(cx.verbatim_node(&n)),
                    },
                    _ => relay(cx, n.children_with_tokens(), line),
                }
                continue;
            }
            SyntaxElement::Token(t) => t,
        };
        match t.kind() {
            k::WHITESPACE if t.text().starts_with('\\') => {
                match (
                    continuation_indent(cx.options),
                    comments::next_with_text(&t),
                ) {
                    (ContinuationIndent::Keep, Some(next)) => {
                        line.out.end_line_at("\\", comments::line_prefix(&next))
                    }
                    _ => line.out.end_line("\\"),
                }
                line.blank = false;
            }
            k::WHITESPACE | k::NEWLINE => line.blank = true,
            k::TAB | k::COMMENT => {}
            kind => {
                let space = match (line.prev, kind) {
                    (None, _) => false,
                    (_, k::COLON) => {
                        t.text().starts_with('&') || cx.options.bool(SPACE_BEFORE_COLON)
                    }
                    (Some(k::COLON | k::PIPE | k::SEMI), _) | (_, k::PIPE | k::SEMI) => true,
                    (_, k::OP) | (Some(k::OP), _) => cx.options.bool(SPACE_AROUND_ASSIGNMENT),
                    _ => line.blank,
                };
                if space {
                    line.out.space();
                }
                let carried = line.carry.take();
                let d = match kind {
                    k::VALUE => value(cx, &t, line.prev == Some(k::OP)),
                    k::KEYWORD => keyword(cx, &t),
                    k::OP if carried.is_some() => {
                        doc::text(format!("{}{}", carried.unwrap_or(' '), t.text()))
                    }
                    k::WORD if glued_operator(&t) => {
                        let name = t.text();
                        line.carry = name.chars().last();
                        doc::concat(vec![
                            cx.leading_comments_doc(&t),
                            doc::text(&name[..name.len() - 1]),
                        ])
                    }
                    _ => cx.token(&t),
                };
                line.push(d);
                line.prev = Some(kind);
                line.blank = false;
            }
        }
    }
}

/// Is `t` the name of `A+=b`, `A?=b` or `A!=b` that the grammar reads as `A+` and `=`?
fn glued_operator(t: &SyntaxToken) -> bool {
    t.text().ends_with(['+', '?', '!'])
        && t.text().len() > 1
        && t.next_token()
            .is_some_and(|n| n.kind() == k::OP && n.text() == "=")
}

/// A keyword without the blanks before it, which its token holds.
fn keyword(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    doc::concat(vec![
        cx.leading_comments_doc(t),
        doc::text(t.text().trim()),
        cx.trailing_comments_doc(t),
    ])
}

/// An assignment's value as written, without the blanks around it; the lines after a continuation
/// keep their blanks.
fn value(cx: &FormatCtx<'_>, t: &core_syntax::SyntaxToken, assignment: bool) -> Doc {
    let text = if assignment {
        t.text().trim_start()
    } else {
        t.text()
    };
    doc::concat(vec![
        cx.leading_comments_doc(t),
        continuation::as_written_value(text),
        cx.trailing_comments_doc(t),
    ])
}

/// `targets: prerequisites | order-only ; command`, then one command per line.
fn rule(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut head = Line::default();
    relay(cx, node.children_with_tokens(), &mut head);
    let mut parts = vec![head.finish()];
    if let Some(recipe) = node.child(k::RECIPE) {
        let mut items: Vec<SyntaxNode> = recipe.children().collect();
        // After `;` comes the command of the rule line, if the line goes on.
        let mut after_semi = recipe
            .children_with_tokens()
            .skip_while(|e| e.kind() != k::SEMI);
        if after_semi.next().is_some() {
            parts.push(doc::lit(" ;"));
            let next = after_semi.find(|e| e.kind() != k::WHITESPACE);
            if next.is_some_and(|e| e.kind() == k::RECIPE_LINE) {
                parts.push(doc::lit(" "));
                parts.push(recipe_line(cx, &items.remove(0), true));
            }
        }
        if !items.is_empty() {
            parts.push(doc::hardline());
            // the blank lines between the rule line and what follows it stay
            parts.push(blank_lines(
                cx,
                items.first().and_then(|n| cx.first_token(n)),
            ));
            parts.push(statement_sequence(
                cx,
                &items,
                cx.options.max_blank_lines(),
            )?);
        }
    }
    Ok(doc::concat(parts))
}

/// The command after one tab (none for the one after `;`), as written.
fn recipe_line(cx: &FormatCtx<'_>, node: &SyntaxNode, inline: bool) -> Doc {
    let Some((_, text)) = code(node) else {
        return doc::nil();
    };
    let leading = cx
        .first_token(node)
        .map_or(doc::nil(), |t| cx.leading_comments_doc(&t));
    let text = text.trim_start_matches([' ', '\t']);
    doc::concat(vec![leading, continuation::as_written(text, !inline)])
}

/// The source text of a node from its first to its last token that is not trivia, and its range.
pub(crate) fn code(node: &SyntaxNode) -> Option<(TextRange, String)> {
    let tokens: Vec<SyntaxToken> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .collect();
    let significant =
        |t: &SyntaxToken| !matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT);
    let first = tokens.iter().position(significant)?;
    let last = tokens.iter().rposition(significant)?;
    let text = tokens[first..=last].iter().map(|t| t.text()).collect();
    let range = tokens[first].text_range().cover(tokens[last].text_range());
    Some((range, text))
}

/// `ifeq (...)`, the body indented (not inside a recipe, where it is a command), `else`, `endif`.
fn conditional(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let in_recipe = in_recipe(node);
    let keep = conditional_indent(cx.options) == ConditionalIndent::Keep;
    let body = |items: &[SyntaxNode]| -> Result<Doc, FormatError> {
        if items.is_empty() {
            return Ok(doc::nil());
        }
        let seq = doc::concat(vec![
            doc::hardline(),
            blank_lines(cx, items.first().and_then(|n| cx.first_token(n))),
            statement_sequence(cx, items, cx.options.max_blank_lines())?,
        ]);
        Ok(if in_recipe || keep {
            seq
        } else {
            doc::indent(seq)
        })
    };
    let mut parts = Vec::new();
    let mut items = Vec::new();
    for e in node.children_with_tokens() {
        match e {
            SyntaxElement::Node(n) if n.kind() == k::DIRECTIVE && parts.is_empty() => {
                let mut line = Line::default();
                relay(cx, n.children_with_tokens(), &mut line);
                parts.push(line.finish());
            }
            SyntaxElement::Node(n) if n.kind() == k::ELSE => {
                parts.push(body(&std::mem::take(&mut items))?);
                let mut inner = Vec::new();
                let head = |inner: &mut Vec<SyntaxNode>| {
                    let mut line = Line::default();
                    relay_header(cx, &n, &mut line, inner);
                    line.finish()
                };
                parts.push(doc::hardline());
                parts.push(blank_lines(cx, cx.first_token(&n)));
                parts.push(match cx.first_token(&n) {
                    Some(first) => kept_line(cx, &n, &first, || head(&mut inner)),
                    None => head(&mut inner),
                });
                parts.push(body(&inner)?);
            }
            SyntaxElement::Node(n) => items.push(n),
            SyntaxElement::Token(t) if t.kind() == k::KEYWORD => {
                parts.push(body(&std::mem::take(&mut items))?);
                parts.push(doc::hardline());
                parts.push(blank_lines(cx, Some(t.clone())));
                parts.push(kept_line(cx, node, &t, || keyword(cx, &t)));
            }
            SyntaxElement::Token(_) => {}
        }
    }
    Ok(doc::concat(parts))
}

/// The blank lines in front of `first` that the source has, after the line break before it.
fn blank_lines(cx: &FormatCtx<'_>, first: Option<SyntaxToken>) -> Doc {
    let count = first.map_or(0, |t| comments::blank_lines_before(cx.grammar, &t));
    doc::concat(
        (0..count.min(cx.options.max_blank_lines()))
            .map(|_| doc::hardline())
            .collect(),
    )
}

/// The `else` line of an `Else` node (its keyword and the condition after it); the other children
/// are its body.
fn relay_header(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    line: &mut Line,
    body: &mut Vec<SyntaxNode>,
) {
    for e in node.children_with_tokens() {
        match e {
            // `else override A = b`: only an `if...` directive is the condition, any other starts the body
            SyntaxElement::Node(n)
                if n.kind() == k::DIRECTIVE
                    && body.is_empty()
                    && n.first_token()
                        .is_some_and(|t| t.text().trim_start().starts_with("if")) =>
            {
                relay(cx, n.children_with_tokens(), line);
            }
            SyntaxElement::Node(n) => body.push(n),
            e => relay(cx, std::iter::once(e), line),
        }
    }
}

/// `define NAME`, the body as written, `endef`.
fn define(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let mut head = Line::default();
    let mut parts = Vec::new();
    let mut elements = node.children_with_tokens().peekable();
    while let Some(e) = elements.next_if(|e| e.kind() != k::RAW && !is_endef(e)) {
        relay(cx, std::iter::once(e), &mut head);
    }
    parts.push(std::mem::take(&mut head).finish());
    for e in elements {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::RAW => {
                let body = t.text().strip_suffix('\n').unwrap_or(t.text());
                // blank lines that the line break after `define NAME` took belong to the value
                let blank = comments::blank_lines_before(cx.grammar, &t);
                parts.extend((0..blank + usize::from(blank > 0)).map(|_| doc::hardline()));
                // The printer starts a line only after text; after verbatim text it takes a hardline.
                for (i, l) in body.split('\n').enumerate() {
                    let line = doc::verbatim(l.trim_end_matches('\r'));
                    let line = if i == 0 {
                        line
                    } else {
                        doc::concat(vec![doc::hardline(), line])
                    };
                    parts.push(doc::outdent(usize::MAX, line));
                }
            }
            SyntaxElement::Token(t) if t.kind() == k::KEYWORD => {
                parts.push(doc::hardline());
                parts.push(kept_line(cx, node, &t, || keyword(cx, &t)));
            }
            _ => {}
        }
    }
    doc::concat(parts)
}

fn is_endef(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.kind() == k::KEYWORD && t.text().trim() == "endef")
}
