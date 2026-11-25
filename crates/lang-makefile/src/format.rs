//! Makefile formatting: statements one per line with the source's blank lines, one space around
//! assignment operators and after rule colons, one tab before each recipe command, conditional
//! bodies indented. Values, recipe commands and `define` bodies are kept as written (trailing
//! blanks aside); recipe commands are Shell code, which the Shell language formats on its own.
//!
//! concept: block-and-statements, key-value-layout, line-continuation-layout

use crate::generated::kind as k;
use core_config::shared;
use core_format::plain_assignment::AssignmentForm;
use core_format::shapes::continuation::{self, Continued, Reindent};
use core_format::shapes::{source_blank_lines, statement_sequence, statements_after_break};
use core_format::source_indent::SourceIndent;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxToken, TextRange};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct MakefileFormatter;

/// The lines that keep the blanks of the source when `conditional_body_indent` is `keep`: those of
/// a conditional and of the statements in its body, a `define` and its `endef`; not in a recipe.
const SOURCE_INDENT: SourceIndent = SourceIndent {
    lines: &[
        k::RULE,
        k::CONDITIONAL,
        k::ELSE,
        k::DEFINE,
        k::ASSIGN,
        k::DIRECTIVE,
        k::CALL,
        k::RECIPE_LINE,
    ],
    containers: &[k::CONDITIONAL, k::ELSE, k::DEFINE],
    stops: &[],
    direct: &[k::RECIPE_LINE],
};

/// The tokens of `name op value`.
const ASSIGNMENT: AssignmentForm = AssignmentForm {
    names: &[k::WORD],
    operators: &[k::OP],
};

impl NodeFormatter for MakefileFormatter {
    /// The formatter trims the blanks around values (not the blanks at the end of one: they are
    /// part of the value), prefixes and keywords.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) if t.kind() == k::VALUE || is_multiline_word(t) => {
                continuation::unindented(&continuation::condensed(t.text().trim_start())).into()
            }
            SyntaxElement::Token(t) => continuation::without_continuation(t.text().trim()).into(),
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let statement = || -> Result<Doc, FormatError> {
            match node.kind() {
                k::DOCUMENT => document(cx, node),
                k::RULE => rule(cx, node),
                k::RECIPE_LINE => Ok(recipe_line(cx, node, false)),
                k::CONDITIONAL => conditional(cx, node),
                k::DEFINE => define(cx, node),
                _ => {
                    let mut line = Line::new(cx, node);
                    relay(cx, node.children_with_tokens(), &mut line)?;
                    Ok(line.finish())
                }
            }
        };
        if !matches!(
            node.kind(),
            k::DOCUMENT
                | k::RULE
                | k::RECIPE_LINE
                | k::CONDITIONAL
                | k::DEFINE
                | k::ASSIGN
                | k::DIRECTIVE
                | k::CALL
        ) {
            return Ok(None);
        }
        if keeps_source_indent(cx) && SOURCE_INDENT.applies(node)
            || keeps_source_indent(cx) && node.kind() == k::ASSIGN && !spaced(cx, node)
        {
            // (an assignment that stays as written keeps its indentation too)
            match cx.first_token(node) {
                Some(first) => cx.at_source_indent(&first, statement),
                None => statement(),
            }
        } else {
            statement()
        }
        .map(Some)
    }
}

/// Do the spacing options apply to the assignment `node`?
fn spaced(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    ASSIGNMENT.spaced(cx, node, keeps_source_indent(cx))
}

fn keeps_source_indent(cx: &FormatCtx<'_>) -> bool {
    conditional_body_indent(cx.options) == ConditionalBodyIndent::Keep
}

/// The line that the keyword `first` of `node` starts, at the blanks of the source when the
/// conditional keeps them.
fn keyword_line(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    first: &SyntaxToken,
    line: impl FnOnce() -> Result<Doc, FormatError>,
) -> Result<Doc, FormatError> {
    if keeps_source_indent(cx) {
        cx.source_indented_at(node, first, &SOURCE_INDENT, line)
    } else {
        line()
    }
}

fn in_recipe(node: &SyntaxNode) -> bool {
    node.ancestors().skip(1).any(|a| a.kind() == k::RECIPE)
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
struct Line {
    out: Continued,
    prev: Option<SyntaxKind>,
    blank: bool,
    /// An assignment that `spaced_assignments` leaves as written: the blanks around its operator
    /// and in its value stay.
    exact: bool,
    /// The blanks that the lines of the block share under `continued_line_indent: first`.
    block_indent: Option<String>,
    /// Not an assignment whose operator is on the first physical line: make may read the line as a
    /// rule's, and leading spaces of its second line give one tab.
    rule: bool,
}

impl Line {
    fn new(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Self {
        Line {
            out: Continued::default(),
            prev: None,
            blank: false,
            exact: node.kind() == k::ASSIGN && !spaced(cx, node),
            block_indent: None,
            rule: !operator_on_first_line(node),
        }
    }

    fn push(&mut self, d: Doc) {
        self.out.push(d);
    }

    fn finish(self) -> Doc {
        self.out.finish()
    }
}

/// Appends the elements to the line: blanks collapse to one space where the source has any, and
/// the style options set the spaces around colons and assignment operators.
fn relay(
    cx: &FormatCtx<'_>,
    elements: impl Iterator<Item = SyntaxElement>,
    line: &mut Line,
) -> Result<(), FormatError> {
    for e in elements {
        let t = match e {
            SyntaxElement::Node(n) => {
                match n.kind() {
                    k::RECIPE | k::CONDITIONAL | k::ELSE => {}
                    k::DEFINE => line.push(cx.format(&n)?),
                    k::ASSIGN => {
                        // (`export A=b`: the assignment inside a directive)
                        // (the blank in front of the assignment belongs to the line around it)
                        if line.blank && line.prev.is_some() {
                            line.out.space();
                            line.blank = false;
                        }
                        let outer = line.exact;
                        line.exact = !spaced(cx, &n);
                        relay(cx, n.children_with_tokens(), line)?;
                        line.exact = outer;
                    }
                    _ => relay(cx, n.children_with_tokens(), line)?,
                }
                continue;
            }
            SyntaxElement::Token(t) => t,
        };
        match t.kind() {
            k::WHITESPACE if is_continuation(&t) => {
                match (
                    continued_line_indent(cx.options),
                    comments::next_with_text(&t),
                ) {
                    (ContinuedLineIndent::Keep, Some(next)) => {
                        line.out.end_line_at("\\", comments::line_prefix(&next))
                    }
                    (ContinuedLineIndent::First, Some(next)) => {
                        let indent = line.block_indent.get_or_insert_with(|| {
                            continuation::block_indent(&comments::line_prefix(&next), line.rule)
                        });
                        line.out.end_line_at("\\", indent.clone())
                    }
                    _ => line.out.end_line("\\"),
                }
                line.blank = false;
            }
            k::WHITESPACE
                if line.exact
                    && line.prev.is_some()
                    && !after_continuation(&t)
                    && !t.next_token().is_some_and(|n| is_continuation(&n)) =>
            {
                line.push(doc::verbatim(t.text()))
            }
            k::WHITESPACE | k::NEWLINE => line.blank = true,
            k::TAB | k::COMMENT => {}
            kind => {
                let space = !line.exact
                    && match (line.prev, kind) {
                        (None, _) => false,
                        (_, k::COLON) => {
                            t.text().starts_with('&') || cx.options.bool(SPACE_BEFORE_COLON)
                        }
                        (Some(k::COLON | k::PIPE | k::SEMI), _) | (_, k::PIPE | k::SEMI) => true,
                        (_, k::OP) | (Some(k::OP), _) => {
                            !line.exact && cx.options.bool(SPACE_AROUND_ASSIGNMENT)
                        }
                        _ => line.blank,
                    };
                if space {
                    line.out.space();
                }
                let d = match kind {
                    k::VALUE => value(cx, &t, line.prev == Some(k::OP), line.exact),
                    k::WORD if is_multiline_word(&t) => value(cx, &t, false, line.exact),
                    k::KEYWORD => keyword(cx, &t),
                    _ => cx.token(&t),
                };
                line.push(d);
                line.prev = Some(kind);
                line.blank = false;
            }
        }
    }
    Ok(())
}

/// Is `node` an assignment with the operator before the first line continuation?
fn operator_on_first_line(node: &SyntaxNode) -> bool {
    node.kind() == k::ASSIGN
        && node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .take_while(|t| !(t.kind() == k::WHITESPACE && t.text().starts_with('\\')))
            .any(|t| t.kind() == k::OP)
}

/// Is `t` a backslash and line break?
fn is_continuation(t: &SyntaxToken) -> bool {
    t.kind() == k::WHITESPACE && t.text().starts_with('\\')
}

/// Does the blank token `t` start a continued line (it follows a backslash and line break)?
fn after_continuation(t: &SyntaxToken) -> bool {
    t.prev_token().is_some_and(|p| is_continuation(&p))
}

/// Shell words that open or close a block: a command that has one keeps the indentation of its
/// continued lines, which show the block's nesting.
const SHELL_BLOCK_WORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "do", "done", "while", "until", "case", "esac",
];

/// With `continued_line_indent: first`, the text of a continued line starts every line after the
/// first at the blanks of the second, unless a line starts with one of `keep_words`.
fn reindent<'a>(
    cx: &FormatCtx<'_>,
    keep_words: &'a [&'a str],
    recipe: bool,
    tab_for_spaces: bool,
) -> Option<Reindent<'a>> {
    (continued_line_indent(cx.options) == ContinuedLineIndent::First).then_some(Reindent {
        keep_words,
        tab_for_spaces,
        recipe,
    })
}

/// A word that holds line continuations (`A-$(call f, a \` over several lines).
fn is_multiline_word(t: &SyntaxToken) -> bool {
    t.kind() == k::WORD && continuation::is_continued_text(t.text())
}

/// A keyword without the blanks before it, which its token holds.
fn keyword(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    doc::concat(vec![
        cx.leading_comments_doc(t),
        doc::text(t.text().trim()),
        cx.trailing_comments_doc(t),
    ])
}

/// An assignment's value as written, without the blanks before it (`exact`: with them); the lines
/// after a continuation keep their blanks, and the blanks before a continuing backslash collapse
/// to one unless `exact`.
fn value(cx: &FormatCtx<'_>, t: &SyntaxToken, assignment: bool, exact: bool) -> Doc {
    let text = if assignment && !exact {
        t.text().trim_start()
    } else {
        t.text()
    };
    doc::concat(vec![
        cx.leading_comments_doc(t),
        continuation::as_written_value(text, true, reindent(cx, &[], false, false)),
        cx.trailing_comments_doc(t),
    ])
}

/// `targets: prerequisites | order-only ; command`, then one command per line.
fn rule(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut head = Line::new(cx, node);
    relay(cx, node.children_with_tokens(), &mut head)?;
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
            // the blank lines between the rule line and what follows it stay
            parts.push(statements_after_break(
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
    // (a tab at the start of a continued line inside a conditional is not parsed as part of the
    // command, so the line keeps its blanks there)
    let in_conditional = node.parent().is_some_and(|p| p.kind() == k::CONDITIONAL);
    let words = reindent(cx, SHELL_BLOCK_WORDS, true, !in_conditional);
    // (the line that keeps the blanks of the source has them in front already)
    let tab = !inline && !(keeps_source_indent(cx) && SOURCE_INDENT.applies(node));
    doc::concat(vec![leading, continuation::as_written(text, tab, words)])
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
    let keep = keeps_source_indent(cx);
    let max = cx.options.max_blank_lines();
    let body = |items: &[SyntaxNode]| -> Result<Doc, FormatError> {
        if items.is_empty() {
            return Ok(doc::nil());
        }
        let seq = statements_after_break(cx, items, max)?;
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
                let mut line = Line::new(cx, &n);
                relay(cx, n.children_with_tokens(), &mut line)?;
                parts.push(line.finish());
            }
            SyntaxElement::Node(n) if n.kind() == k::ELSE => {
                parts.push(body(&std::mem::take(&mut items))?);
                let mut inner = Vec::new();
                let first = cx.first_token(&n);
                let mut head = || -> Result<Doc, FormatError> {
                    let mut line = Line::new(cx, &n);
                    relay_header(cx, &n, &mut line, &mut inner)?;
                    Ok(line.finish())
                };
                parts.push(doc::hardline());
                parts.push(source_blank_lines(cx, first.as_ref(), max));
                parts.push(match first {
                    Some(first) => keyword_line(cx, &n, &first, head)?,
                    None => head()?,
                });
                parts.push(body(&inner)?);
            }
            SyntaxElement::Node(n) => items.push(n),
            SyntaxElement::Token(t) if t.kind() == k::KEYWORD => {
                parts.push(body(&std::mem::take(&mut items))?);
                parts.push(doc::hardline());
                parts.push(source_blank_lines(cx, Some(&t), max));
                parts.push(keyword_line(cx, node, &t, || Ok(keyword(cx, &t)))?);
            }
            SyntaxElement::Token(_) => {}
        }
    }
    Ok(doc::concat(parts))
}

/// The `else` line of an `Else` node (its keyword and the condition after it); the other children
/// are its body.
fn relay_header(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    line: &mut Line,
    body: &mut Vec<SyntaxNode>,
) -> Result<(), FormatError> {
    for e in node.children_with_tokens() {
        match e {
            // `else override A = b`: only an `if...` directive is the condition, any other starts the body
            SyntaxElement::Node(n)
                if n.kind() == k::DIRECTIVE
                    && body.is_empty()
                    && n.first_token()
                        .is_some_and(|t| t.text().trim_start().starts_with("if")) =>
            {
                relay(cx, n.children_with_tokens(), line)?;
            }
            SyntaxElement::Node(n) => body.push(n),
            e => relay(cx, std::iter::once(e), line)?,
        }
    }
    Ok(())
}

/// `define NAME`, the body as written, `endef`.
fn define(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut head = Line::new(cx, node);
    let mut parts = Vec::new();
    let mut elements = node.children_with_tokens().peekable();
    while let Some(e) = elements.next_if(|e| e.kind() != k::RAW && !is_endef(e)) {
        relay(cx, std::iter::once(e), &mut head)?;
    }
    parts.push(head.finish());
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
                parts.push(keyword_line(cx, node, &t, || Ok(keyword(cx, &t)))?);
            }
            _ => {}
        }
    }
    Ok(doc::concat(parts))
}

fn is_endef(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.kind() == k::KEYWORD && t.text().trim() == "endef")
}
