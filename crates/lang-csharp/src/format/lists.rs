//! Delimited lists: argument and parameter lists, initializers, collection expressions,
//! attribute lists.

use super::ROWS;
use super::options::{
    anonymous_type_brace_on_new_line, collection_blank_lines, creation_brace_on_new_line,
    empty_braces, expand_crowded_initializers, hug_closing_paren, hug_only_sole_lambda,
    initializer_layout, nested_brace_on_new_line, trailing_comma,
};
use super::seq::{R, Sep, direct_tokens, sep_doc, separator, seq_of};
use crate::generated::kind as k;
use core_format::seq::{children, el};
use core_format::shapes::{
    self, CallStyle, HugLast, ListLayout, ListStyle, TrailingComma, delimited_list,
    delimited_list_between,
};
use core_format::{FormatCtx, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// `x is { A: 1 }` and `x is T { A: 1 }`: the braces of a property pattern start on their own
/// line when the pattern breaks, but not in `{ A: 1 } => ...`, where nothing precedes them.
fn property_pattern_follows_something(clause: &SyntaxNode) -> bool {
    let Some(pattern) = clause.parent() else {
        return false;
    };
    clause.prev_sibling().is_some()
        || pattern.parent().map(|p| p.kind()) == Some(k::IS_PATTERN_EXPRESSION)
}

/// Parameters, arguments, type arguments, attributes, tuples, patterns: one line if it fits,
/// else one item per line.
pub fn plain_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let style = match node.kind() {
        k::PROPERTY_PATTERN_CLAUSE => ListStyle {
            padding: true,
            empty: empty_braces(cx.options),
            leading_line: nested_brace_on_new_line(cx.options)
                && property_pattern_follows_something(node),
            ..ListStyle::default()
        },
        _ => call_style(cx, node),
    };
    delimited_list(cx, node, &style)
}

/// Parameter and argument lists: `)` follows the last item with `closing_paren_line: same_line`.
fn call_style(cx: &FormatCtx<'_>, node: &SyntaxNode) -> ListStyle {
    ListStyle {
        hug_close: hug_closing_paren(cx.options)
            && matches!(
                node.kind(),
                k::ARGUMENT_LIST | k::PARAMETER_LIST | k::ATTRIBUTE_ARGUMENT_LIST
            ),
        ..ListStyle::default()
    }
}

/// A delimited list that is never broken: the items separated by `, ` between the delimiters.
pub fn unbreakable_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let style = ListStyle {
        layout: ListLayout::Flat,
        ..ListStyle::default()
    };
    delimited_list(cx, node, &style)
}

fn creation_parent(kind: Option<SyntaxKind>) -> bool {
    matches!(
        kind,
        Some(
            k::OBJECT_CREATION_EXPRESSION
                | k::IMPLICIT_OBJECT_CREATION_EXPRESSION
                | k::ARRAY_CREATION_EXPRESSION
                | k::IMPLICIT_ARRAY_CREATION_EXPRESSION
                | k::STACK_ALLOC_ARRAY_CREATION_EXPRESSION
                | k::IMPLICIT_STACK_ALLOC_ARRAY_CREATION_EXPRESSION
                | k::WITH_EXPRESSION
        )
    )
}

/// Does `node` (an initializer or anonymous type) create an object with an initializer, or is it
/// an assignment of one?
fn creates_with_initializer(node: &SyntaxNode) -> bool {
    match node.kind() {
        k::OBJECT_CREATION_EXPRESSION | k::IMPLICIT_OBJECT_CREATION_EXPRESSION => node
            .children()
            .any(|c| c.kind() == k::OBJECT_INITIALIZER_EXPRESSION),
        k::SIMPLE_ASSIGNMENT_EXPRESSION => node
            .children()
            .last()
            .is_some_and(|c| creates_with_initializer(&c)),
        _ => false,
    }
}

/// `initializer_expand: crowded`: three or more member assignments, or two or more items one of
/// which creates an object with an initializer.
fn crowded(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let items: Vec<SyntaxNode> = node
        .children()
        .filter(|c| !cx.grammar.is_trivia(c.kind()))
        .collect();
    let assignments = items.iter().all(|c| {
        matches!(
            c.kind(),
            k::SIMPLE_ASSIGNMENT_EXPRESSION | k::ANONYMOUS_OBJECT_MEMBER_DECLARATOR
        )
    });
    (items.len() >= 3 && assignments)
        || (items.len() >= 2 && items.iter().any(creates_with_initializer))
}

fn brace_style(cx: &FormatCtx<'_>, node: &SyntaxNode) -> ListStyle {
    ListStyle {
        padding: true,
        // `{ key, value }` as an element of a collection initializer takes no trailing comma.
        trailing_comma: if node.kind() == k::COMPLEX_ELEMENT_INITIALIZER_EXPRESSION {
            TrailingComma::Never
        } else {
            trailing_comma(cx.options)
        },
        layout: if expand_crowded_initializers(cx.options) && crowded(cx, node) {
            ListLayout::Break
        } else {
            initializer_layout(cx.options)
        },
        max_blank_lines: if matches!(
            node.kind(),
            k::COLLECTION_INITIALIZER_EXPRESSION | k::ARRAY_INITIALIZER_EXPRESSION
        ) {
            collection_blank_lines(cx.options)
        } else {
            1
        },
        empty: empty_braces(cx.options),
        ..ListStyle::default()
    }
}

/// `{ a, b }`, or when it does not fit (or the source broke it) the braces on their own lines.
pub fn initializer(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut style = brace_style(cx, node);
    style.leading_line = (creation_parent(node.parent().map(|p| p.kind()))
        && creation_brace_on_new_line(cx.options))
        || (nested_brace_on_new_line(cx.options)
            && node.parent().map(|p| p.kind()) == Some(k::SIMPLE_ASSIGNMENT_EXPRESSION));
    delimited_list(cx, node, &style)
}

pub fn collection_expression(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let style = ListStyle {
        padding: false,
        trailing_comma: trailing_comma(cx.options),
        layout: initializer_layout(cx.options),
        max_blank_lines: collection_blank_lines(cx.options),
        ..ListStyle::default()
    };
    delimited_list(cx, node, &style)
}

/// Lists with other children around them: `new { A = 1 }`, `[1, 2] list`.
pub fn list_with_surroundings(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let (open_kind, close_kind) = if node.kind() == k::LIST_PATTERN {
        (k::OPEN_BRACKET_TOKEN, k::CLOSE_BRACKET_TOKEN)
    } else {
        (k::OPEN_BRACE_TOKEN, k::CLOSE_BRACE_TOKEN)
    };
    let els = children(cx, node);
    let open = direct_tokens(node, open_kind).into_iter().next();
    let close = direct_tokens(node, close_kind).into_iter().last();
    let (Some(open), Some(close)) = (open, close) else {
        return seq_of(cx, node, &els);
    };
    let at = |t: &SyntaxToken| {
        els.iter()
            .position(|e| matches!(e, SyntaxElement::Token(x) if x == t))
    };
    let (Some(oi), Some(ci)) = (at(&open), at(&close)) else {
        return seq_of(cx, node, &els);
    };
    let mut style = if node.kind() == k::LIST_PATTERN {
        ListStyle::default()
    } else {
        brace_style(cx, node)
    };
    style.trailing_comma = if node.kind() == k::LIST_PATTERN {
        TrailingComma::Never
    } else {
        style.trailing_comma
    };
    style.leading_line =
        node.kind() != k::LIST_PATTERN && anonymous_type_brace_on_new_line(cx.options);
    let mut parts = Vec::new();
    if oi > 0 {
        parts.push(seq_of(cx, node, &els[..oi])?);
        let sep = if style.leading_line {
            Sep::None
        } else {
            separator(cx, node, &els[oi - 1], &els[oi])
        };
        parts.push(sep_doc(cx, sep));
    }
    parts.push(delimited_list_between(cx, node, &open, &close, &style)?);
    if ci + 1 < els.len() {
        parts.push(sep_doc(cx, separator(cx, node, &els[ci], &els[ci + 1])));
        parts.push(seq_of(cx, node, &els[ci + 1..])?);
    }
    Ok(doc::concat(parts))
}

/// The argument list: the lambda that hugs the call (`lambda_argument_hug`), else the list.
pub fn argument_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let sole = hug_only_sole_lambda(cx.options);
    let style = CallStyle {
        first: false,
        last: if sole { HugLast::Sole } else { HugLast::Last },
        body: sole,
    };
    shapes::call_arguments(cx, node, ROWS.get(), &style, &call_style(cx, node))
}

/// `parameter_attribute_layout: hanging`: the attribute lists, then the rest of the parameter on
/// an indented line below them when the whole parameter does not fit.
pub fn parameter_with_attributes(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let split = els
        .iter()
        .position(|e| !matches!(e, SyntaxElement::Node(n) if n.kind() == k::ATTRIBUTE_LIST))
        .unwrap_or(els.len());
    let (lists, rest) = els.split_at(split);
    if rest.is_empty() {
        return seq_of(cx, node, &els);
    }
    let lists = lists
        .iter()
        .map(|e| el(cx, e))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(doc::group(doc::concat(vec![
        doc::join(&doc::lit(" "), lists),
        doc::indent(doc::concat(vec![doc::line(), seq_of(cx, node, rest)?])),
    ])))
}
