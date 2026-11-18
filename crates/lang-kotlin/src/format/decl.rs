//! Files, bodies, declarations and the lists of parameters and arguments.

use super::*;
use core_format::shapes::{self, ListLayout, TrailingComma};

pub(super) fn rule(cx: &FormatCtx<'_>, node: &SyntaxNode, kind: SyntaxKind) -> Option<R> {
    Some(match kind {
        k::KT_FILE => file(cx, node),
        k::IMPORT_LIST => statements(cx, node, None, 0),
        k::CLASS_BODY => body(cx, node),
        k::BLOCK => block(cx, node),
        k::VALUE_ARGUMENT_LIST | k::VALUE_PARAMETER_LIST => paren_list(cx, node),
        k::PROPERTY | k::FUN | k::PROPERTY_ACCESSOR | k::VALUE_ARGUMENT => with_value(cx, node),
        k::STRING_TEMPLATE => Ok(shapes::verbatim(cx, node)),
        k::MODIFIER_LIST | k::FILE_ANNOTATION_LIST => FLOW.flat(cx, node),
        _ => return None,
    })
}

fn is_decl(kind: SyntaxKind) -> bool {
    [
        k::CLASS,
        k::FUN,
        k::PROPERTY,
        k::OBJECT_DECLARATION,
        k::SECONDARY_CONSTRUCTOR,
        k::TYPEALIAS,
        k::CLASS_INITIALIZER,
    ]
    .contains(&kind)
}

fn bodiless(n: &SyntaxNode) -> bool {
    n.kind() == k::SECONDARY_CONSTRUCTOR
        && !n
            .children()
            .any(|c| matches!(c.kind(), k::BLOCK | k::CONSTRUCTOR_DELEGATION_CALL))
}

/// Must a blank line stand between `prev` and `node` (siblings in a file, class body or block)?
fn needs_blank(cx: &FormatCtx<'_>, prev: &SyntaxNode, node: &SyntaxNode) -> bool {
    let parent = parent_kind(node);
    if parent == k::KT_FILE
        && [
            k::PACKAGE_DIRECTIVE,
            k::IMPORT_LIST,
            k::FILE_ANNOTATION_LIST,
        ]
        .contains(&prev.kind())
    {
        return !(prev.kind() == k::PACKAGE_DIRECTIVE && node.kind() == k::IMPORT_LIST
            || prev.kind() == k::FILE_ANNOTATION_LIST && node.kind() == k::PACKAGE_DIRECTIVE);
    }
    let same = |kinds: &[SyntaxKind]| kinds.contains(&node.kind()) && prev.kind() == node.kind();
    let commented = cx
        .first_token(node)
        .is_some_and(|t| !comments::leading_comments(cx.grammar, &t).is_empty());
    match parent {
        k::KT_FILE | k::CLASS_BODY => {
            (node.kind() == k::ENUM_ENTRY && prev.kind() == k::ENUM_ENTRY && commented)
                || is_decl(node.kind())
                    && (!same(&[k::PROPERTY, k::TYPEALIAS]) || commented)
                    && !(bodiless(node) && bodiless(prev))
                    && !(commented && blank_in_comments(cx, node))
        }
        k::BLOCK => is_decl(node.kind()) && node.kind() != k::PROPERTY,
        _ => false,
    }
}

/// Items one per line: the nodes of a file or of a body without braces.
fn statements(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    force: Option<shapes::BlankRule>,
    max_blank: usize,
) -> R {
    let items: Vec<SyntaxNode> = node
        .children()
        .filter(|n| !n.text_range().is_empty())
        .collect();
    if items.len() != kids(cx, node).len() {
        return generic(cx, node);
    }
    shapes::statement_sequence_with(cx, &items, max_blank, force, false)
}

fn file(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = vec![statements(cx, node, Some(needs_blank), 1)?];
    let eof = node.last_token().filter(|t| t.kind() == k::END_OF_FILE);
    if let Some(eof) = eof {
        let trailing = comments::leading_comments(cx.grammar, &eof);
        if !trailing.is_empty() {
            if node.children().any(|c| !c.text_range().is_empty()) {
                parts.push(doc::hardline());
                if blank_before_comments(cx, &eof) {
                    parts.push(doc::hardline());
                }
            }
            parts.push(core_format::context::leading_comments_doc(&trailing));
        }
    }
    Ok(doc::concat(parts))
}

fn body_style() -> shapes::BlockStyle {
    shapes::BlockStyle {
        force_blank: Some(needs_blank),
        ..shapes::BlockStyle::default()
    }
}

/// `{}`, `{ }` and `{` newline `}` stay as written when there is nothing between the braces.
pub(super) fn empty_braces(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> Option<R> {
    let [SyntaxElement::Token(open), SyntaxElement::Token(close)] = els else {
        return None;
    };
    if open.kind() != k::LBRACE || close.kind() != k::RBRACE {
        return None;
    }
    if !comments::leading_comments(cx.grammar, close).is_empty() {
        return None;
    }
    let gap = if comments::break_after(cx.grammar, open) {
        doc::hardline()
    } else if open.text_range().end() != close.text_range().start() {
        doc::lit(" ")
    } else {
        doc::lit("")
    };
    Some(Ok(doc::concat(vec![cx.token(open), gap, cx.token(close)])))
}

fn body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    if let Some(r) = empty_braces(cx, &els) {
        return r;
    }
    // `enum class E { A }` stays on its line.
    if let [open, entry, close] = &els[..]
        && entry.kind() == k::ENUM_ENTRY
        && !multiline(cx, node)
    {
        return Ok(doc::concat(vec![
            el(cx, open)?,
            doc::lit(" "),
            el(cx, entry)?,
            doc::lit(" "),
            el(cx, close)?,
        ]));
    }
    shapes::block(cx, node, &body_style()).or_else(|_| generic(cx, node))
}

/// A block with braces is a body; one without (a lambda's statements) is a sequence.
fn block(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    match cx.first_token(node).map(|t| t.kind()) {
        Some(k::LBRACE) => body(cx, node),
        _ => statements(cx, node, Some(needs_blank), 1),
    }
}

/// A declaration with a value after `=` (a property, a function with an expression body, an
/// accessor): the value moves below the operator when it does not fit or spans lines.
fn with_value(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let Some(eq) = els.iter().position(|e| e.kind() == k::EQ) else {
        return generic(cx, node);
    };
    if eq + 2 != els.len() {
        return generic(cx, node);
    }
    let SyntaxElement::Token(eq_tok) = &els[eq] else {
        return generic(cx, node);
    };
    assign(cx, node, seq(cx, &els[..eq])?, eq_tok, &els[eq + 1])
}

/// `left = right` as the shared assignment layouts.
pub(super) fn assign(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    left: Doc,
    eq_tok: &SyntaxToken,
    right: &SyntaxElement,
) -> R {
    let op = doc::concat(vec![doc::lit(" "), cx.token(eq_tok)]);
    // A comment at the end of the line stays there, it does not push the value below the `=`.
    cx.trailing_after(last(cx, right).as_ref(), || {
        shapes::assignment(
            cx,
            shapes::Assign {
                node,
                left,
                op,
                op_token: Some(eq_tok),
                right: Some(right),
                short_key: false,
            },
            &shapes::AssignPolicy::of(cx.options),
            &KeepOnLine,
        )
    })
}

/// The call or the declaration whose parentheses `list` has.
fn paren_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let parent = parent_kind(node);
    let (min, comma) = match parent {
        k::FUN => (
            cx.options.int("function_signature_wrap_min_parameters") as usize,
            true,
        ),
        k::PRIMARY_CONSTRUCTOR => (
            cx.options.int("class_signature_wrap_min_parameters") as usize,
            true,
        ),
        k::FUNCTION_TYPE | k::CATCH | k::PROPERTY_ACCESSOR => (usize::MAX, false),
        _ => (usize::MAX, true),
    };
    let multiline = cx.options.str("trailing_commas") == "multiline";
    let trailing = if comma && multiline {
        TrailingComma::WhenBroken
    } else {
        TrailingComma::Never
    };
    let otherwise = if min == usize::MAX && comma {
        ListLayout::PreserveItemBreak
    } else {
        ListLayout::Fit
    };
    shapes::parameter_list(cx, node, min, trailing, otherwise).or_else(|_| generic(cx, node))
}

/// Does a blank line stand between the last code and the first comment before the token?
fn blank_before_comments(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> bool {
    comments::leading_trivia(cx.grammar, tok)
        .iter()
        .take_while(|e| !cx.grammar.has_tag(e.kind(), "comment"))
        .any(|e| {
            e.as_token()
                .is_some_and(|t| comments::count_breaks(t.text()) > 0)
        })
}

/// Does the source have a blank line between the comments in front of the node, or after them?
fn blank_in_comments(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    cx.first_token(node).is_some_and(|t| {
        comments::leading_comments(cx.grammar, &t)
            .iter()
            .any(|c| c.breaks_after >= 2)
    })
}

/// A string that spans lines stays on the line of the `=`.
struct KeepOnLine;

impl shapes::AssignHooks for KeepOnLine {
    fn layout(
        &self,
        cx: &FormatCtx<'_>,
        _: &shapes::Assign<'_>,
        right: &SyntaxElement,
        _: &shapes::AssignPolicy,
    ) -> Option<shapes::AssignLayout> {
        let n = right.as_node()?;
        let stays = match n.kind() {
            k::STRING_TEMPLATE => multiline(cx, n),
            k::LAMBDA_EXPRESSION => true,
            _ => false,
        };
        stays.then_some(shapes::AssignLayout::NeverBreakAfterOperator)
    }
}
