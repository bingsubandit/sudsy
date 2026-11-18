//! Helpers shared by the formatting modules: children, tokens, options, document shapes.

use crate::generated::kind as k;
use core_config::OptionValue;
use core_format::doc::{self, Doc, concat, group, if_break, indent, line, lit, softline};
use core_format::{FormatCtx, FormatError, comments};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

pub(crate) type R = Result<Doc, FormatError>;

thread_local! {
    /// Is the expression being built inside brackets (or parentheses that may appear)? Operators
    /// break there; outside, only the brackets of the expression split.
    static OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

thread_local! {
    /// May a chain of calls break at its dots? (Not as an expression statement.)
    static FLUENT: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub(crate) fn with_fluent<T>(on: bool, f: impl FnOnce() -> T) -> T {
    let prev = FLUENT.replace(on);
    let out = f();
    FLUENT.set(prev);
    out
}

pub(crate) fn fluent_allowed() -> bool {
    FLUENT.get()
}

/// Builds `f` with the bracket context set to `open`.
pub(crate) fn with_open<T>(open: bool, f: impl FnOnce() -> T) -> T {
    let prev = OPEN.replace(open);
    let out = f();
    OPEN.set(prev);
    out
}

/// A space that may break the line, where ruff breaks only inside brackets.
pub(crate) fn brk() -> Doc {
    if OPEN.get() { line() } else { lit(" ") }
}

pub(crate) fn soft_brk() -> Doc {
    if OPEN.get() { softline() } else { Doc::Nil }
}

/// The `string` row of Python's string token.
pub(crate) fn syntax() -> &'static core_syntax::strings::StringSyntax {
    let g = core_syntax::grammars::PYTHON.get();
    g.string_rows(crate::generated::kind::STRING)
        .next()
        .expect("the Python grammar has a string row")
}

pub(crate) struct Style {
    pub quote: core_format::literals::prefixed::Quote,
    pub magic_comma: bool,
    pub width: usize,
    pub indent: usize,
}

pub(crate) fn style<'a>(cx: &'a FormatCtx<'_>) -> &'a Style {
    cx.cached(|| Style {
        quote: match cx.options.get("quote_style") {
            Some(OptionValue::String(s)) if s == "single" => {
                core_format::literals::prefixed::Quote::Single
            }
            Some(OptionValue::String(s)) if s == "preserve" => {
                core_format::literals::prefixed::Quote::Preserve
            }
            _ => core_format::literals::prefixed::Quote::Double,
        },
        magic_comma: !matches!(cx.options.get("magic_trailing_comma"),
            Some(OptionValue::String(s)) if s == "ignore"),
        width: cx.options.int("line_width") as usize,
        indent: cx.options.int("indent_size") as usize,
    })
}

pub(crate) fn kids(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|e| !cx.grammar.is_trivia(e.kind()))
        .collect()
}

pub(crate) fn tokens(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxToken> {
    kids(cx, node)
        .into_iter()
        .filter_map(|e| e.into_token())
        .collect()
}

pub(crate) fn tok_kind(e: &SyntaxElement) -> Option<SyntaxKind> {
    e.as_token().map(|t| t.kind())
}

pub(crate) fn shape_error(what: &str) -> FormatError {
    FormatError::Shape(what.to_string())
}

pub(crate) fn has_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    !comments::leading_comments(cx.grammar, t).is_empty()
        || !comments::trailing_comments(cx.grammar, t).is_empty()
}

/// `( inner )` that breaks onto indented lines when it does not fit.
pub(crate) fn soft_block(inner: Doc) -> Doc {
    concat(vec![indent(concat(vec![softline(), inner])), softline()])
}

pub(crate) fn parenthesized(open: Doc, inner: Doc, close: Doc) -> Doc {
    group(concat(vec![open, soft_block(inner), close]))
}

/// [`parenthesized`] around bracket tokens; own-line comments in front of the closing one stay
/// inside the indent.
pub(crate) fn bracketed_by(
    cx: &FormatCtx<'_>,
    open: &SyntaxToken,
    inner: Doc,
    close: &SyntaxToken,
) -> Doc {
    let end = core_format::comments::leading_comments(cx.grammar, close);
    if end.is_empty() {
        return parenthesized(cx.token(open), inner, cx.token(close));
    }
    cx.hold_leading(close);
    let end = concat(vec![doc::hardline(), core_format::comment_lines(&end)]);
    parenthesized(cx.token(open), concat(vec![inner, end]), cx.token(close))
}

/// Operands of the binary-like chains and other places that name a bracket kind.
pub(crate) fn is_open(kind: SyntaxKind) -> bool {
    matches!(kind, k::LPAR | k::LSQB | k::LBRACE)
}

pub(crate) fn is_stmt(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::STMT_FUNCTION_DEF
            | k::STMT_CLASS_DEF
            | k::STMT_RETURN
            | k::STMT_DELETE
            | k::STMT_TYPE_ALIAS
            | k::STMT_ASSIGN
            | k::STMT_AUG_ASSIGN
            | k::STMT_ANN_ASSIGN
            | k::STMT_FOR
            | k::STMT_WHILE
            | k::STMT_IF
            | k::STMT_WITH
            | k::STMT_MATCH
            | k::STMT_RAISE
            | k::STMT_TRY
            | k::STMT_ASSERT
            | k::STMT_IMPORT
            | k::STMT_IMPORT_FROM
            | k::STMT_GLOBAL
            | k::STMT_NONLOCAL
            | k::STMT_EXPR
            | k::STMT_PASS
            | k::STMT_BREAK
            | k::STMT_CONTINUE
    )
}

/// An `if_break` that adds the trailing comma of a broken list.
pub(crate) fn trailing_comma() -> Doc {
    if_break(lit(","), Doc::Nil)
}

pub(crate) fn sp() -> Doc {
    lit(" ")
}

pub(crate) fn ln() -> Doc {
    line()
}

/// A token printed as `text` (restyled) with its own comments.
pub(crate) fn restyled(cx: &FormatCtx<'_>, t: &SyntaxToken, text: &str) -> Doc {
    concat(vec![
        cx.leading_comments_doc(t),
        doc::text(text),
        cx.trailing_comments_doc(t),
    ])
}

/// ruff leaves the comments of tools out of the line width.
pub(crate) fn is_pragma(comment: &str) -> bool {
    let t = comment.trim_start_matches('#').trim_start();
    [
        "noqa", "type:", "pyright:", "pylint:", "flake8:", "ruff:", "isort:", "nosec",
    ]
    .iter()
    .any(|p| t.to_ascii_lowercase().starts_with(p))
}

/// The comment after `tok` on its line: its text counts for the width of what comes before (as
/// ruff reserves it) unless it is a tool's pragma. The token must have been held with
/// [`FormatCtx::hold_trailing`].
pub(crate) fn trailing_inline(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> Doc {
    let mut parts = Vec::new();
    for c in core_format::comments::trailing_comments(cx.grammar, tok) {
        let text = c.text().to_string();
        if is_pragma(&text) {
            parts.push(doc::line_suffix(doc::text(format!("  {text}"))));
        } else {
            parts.push(doc::text(format!("  {text}")));
        }
    }
    concat(parts)
}

/// The document printed on one line: a group never breaks.
pub(crate) fn flatten(d: Doc) -> Doc {
    match d {
        Doc::Line => lit(" "),
        Doc::SoftLine => Doc::Nil,
        Doc::Group(g) => flatten(g.contents),
        Doc::IfBreak { flat, .. } => flatten(*flat),
        Doc::Concat(ps) | Doc::Fill(ps) => Doc::Concat(ps.into_iter().map(flatten).collect()),
        Doc::Indent(d) | Doc::Align(_, d) | Doc::Label(_, d) | Doc::IndentIfBreak(_, d) => {
            flatten(*d)
        }
        Doc::Shared(s) => flatten(s.doc.clone()),
        other => other,
    }
}

/// `elif`, `else`, `except` and `case` clauses: nodes that belong to a compound statement.
pub(crate) fn is_clause(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::ELIF_ELSE_CLAUSE | k::EXCEPT_HANDLER_EXCEPT_HANDLER | k::MATCH_CASE
    )
}

/// Statements with a block.
pub(crate) fn is_compound(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::STMT_FUNCTION_DEF
            | k::STMT_CLASS_DEF
            | k::STMT_FOR
            | k::STMT_WHILE
            | k::STMT_IF
            | k::STMT_WITH
            | k::STMT_MATCH
            | k::STMT_TRY
    )
}
