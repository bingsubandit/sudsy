//! Compilation units and everything with a braced body: types, namespaces, blocks, switch
//! statements and expressions, enums, accessor lists.
//!
//! concept: block-and-statements, declaration-kit

use super::flat;
use super::options::{
    empty_body_text, expand_empty_bodies, hanging_base_list, indent_switch_section_block,
    inline_accessor_bodies, preserve_single_line_blocks, preserve_single_line_statements,
    remove_accessor_blank_lines, separate_all_members, trailing_comma,
};
use super::seq::{
    R, Sep, block_is_empty, braced_element, direct_tokens, is_empty_braces, keeps_empty_inline,
    sep_doc, separator, seq_of,
};
use crate::generated::kind as k;
use core_format::blank;
use core_format::seq::{children, el};
use core_format::shapes::{
    BlockStyle, HeaderStyle, ListLayout, ListStyle, block_between, delimited_list_between, header,
    statement_sequence_with,
};
use core_format::{FormatCtx, FormatError, comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

fn nodes_of(els: &[SyntaxElement]) -> Vec<SyntaxNode> {
    els.iter()
        .filter_map(|e| match e {
            SyntaxElement::Node(n) => Some(n.clone()),
            SyntaxElement::Token(_) => None,
        })
        .collect()
}

fn position(els: &[SyntaxElement], t: &SyntaxToken) -> Option<usize> {
    els.iter()
        .position(|e| matches!(e, SyntaxElement::Token(x) if x == t))
}

/// The members, then the comments that end the file.
pub fn compilation_unit(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let items: Vec<SyntaxNode> = nodes_of(&els);
    let mut parts = vec![statement_sequence_with(
        cx,
        &items,
        1,
        blank::rule(cx),
        preserve_single_line_statements(cx.options),
    )?];
    if let Some(SyntaxElement::Token(eof)) = els.last() {
        let trailing = comments::leading_comments(cx.grammar, eof);
        if !trailing.is_empty() {
            if !items.is_empty() {
                parts.push(doc::hardline());
                if comments::blank_lines_before(cx.grammar, eof) > 0 {
                    parts.push(doc::hardline());
                }
            }
            parts.push(core_format::context::leading_comments_doc(&trailing));
        }
    }
    Ok(doc::concat(parts))
}

/// `namespace N;` and then the members, which are not indented.
pub fn file_scoped_namespace(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let semi = direct_tokens(node, k::SEMICOLON_TOKEN)
        .into_iter()
        .next()
        .and_then(|t| position(&els, &t))
        .ok_or_else(|| FormatError::Shape("FileScopedNamespaceDeclaration without `;`".into()))?;
    let header = seq_of(cx, node, &els[..=semi])?;
    let rest = nodes_of(&els[semi + 1..]);
    let Some(first) = rest.first().and_then(|n| cx.first_token(n)) else {
        return Ok(header);
    };
    let mut parts = vec![header, doc::hardline()];
    if comments::blank_lines_before(cx.grammar, &first) > 0 || separate_all_members(cx.options) {
        parts.push(doc::hardline());
    }
    parts.push(statement_sequence_with(
        cx,
        &rest,
        1,
        blank::rule(cx),
        false,
    )?);
    Ok(doc::concat(parts))
}

/// The body in braces of a node: `pre {block} post`, where `pre` and `post` are the other
/// children of the node laid out in sequence.
fn with_braces(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    body: impl FnOnce(&SyntaxToken, &SyntaxToken) -> R,
) -> R {
    let els = children(cx, node);
    let open = direct_tokens(node, k::OPEN_BRACE_TOKEN).into_iter().next();
    let close = direct_tokens(node, k::CLOSE_BRACE_TOKEN).into_iter().last();
    let (Some(open), Some(close)) = (open, close) else {
        return heading(cx, node, &els);
    };
    let (Some(oi), Some(ci)) = (position(&els, &open), position(&els, &close)) else {
        return heading(cx, node, &els);
    };
    let mut parts = Vec::new();
    if oi > 0 {
        parts.push(heading(cx, node, &els[..oi])?);
        let sep = if (is_empty_braces(cx, &open, &close) && keeps_empty_inline(cx, node))
            || flat::inside(cx, node)
        {
            Sep::Space
        } else {
            Sep::Brace(braced_element(node.kind()))
        };
        parts.push(sep_doc(cx, sep));
    }
    parts.push(body(&open, &close)?);
    if ci + 1 < els.len() {
        let sep = separator(cx, node, &els[ci], &els[ci + 1]);
        parts.push(sep_doc(cx, sep));
        parts.push(seq_of(cx, node, &els[ci + 1..])?);
    }
    Ok(doc::concat(parts))
}

fn block_style(cx: &FormatCtx<'_>) -> BlockStyle {
    BlockStyle {
        empty: empty_body_text(cx.options),
        ..BlockStyle::DEFAULT
    }
}

/// Blocks, namespaces, types and switch statements.
pub fn braced(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    if flat::inside(cx, node) && !block_is_empty(cx, node) {
        return with_braces(cx, node, |open, close| flat::block(cx, node, open, close));
    }
    let style = BlockStyle {
        force_blank: blank::rule(cx),
        expand_empty: expand_empty_bodies(cx.options) && !keeps_empty_inline(cx, node),
        keep_same_line: node.kind() == k::BLOCK && preserve_single_line_statements(cx.options),
        ..block_style(cx)
    };
    with_braces(cx, node, |open, close| {
        block_between(cx, node, open, close, &style)
    })
}

/// `{ get; set; }` on one line when nothing in it needs more room.
pub fn accessors_inline(cx: &FormatCtx<'_>, list: &SyntaxNode) -> bool {
    let g = cx.grammar;
    let open = direct_tokens(list, k::OPEN_BRACE_TOKEN).into_iter().next();
    let close = direct_tokens(list, k::CLOSE_BRACE_TOKEN).into_iter().last();
    let (Some(open), Some(close)) = (open, close) else {
        return false;
    };
    let range = TextRange::new(open.text_range().start(), close.text_range().start());
    let blocks = preserve_single_line_blocks(cx.options);
    let simple = list.children().all(|a| {
        g.is_trivia(a.kind())
            || a.children().all(|c| match c.kind() {
                k::BLOCK => blocks && flat::is_root(cx, &c),
                k::ARROW_EXPRESSION_CLAUSE | k::ATTRIBUTE_LIST => false,
                _ => true,
            })
    });
    let has_comment = list
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .any(|t| g.has_tag(t.kind(), "comment"));
    let has_body = list
        .children()
        .any(|a| a.children().any(|c| c.kind() == k::BLOCK));
    simple
        && !has_comment
        && !((has_body || !inline_accessor_bodies(cx.options)) && cx.has_line_break(list, range))
}

pub fn accessor_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    if !accessors_inline(cx, node) {
        // `accessor_blank_lines: remove` drops the blank lines between accessors.
        let style = BlockStyle {
            max_blank_lines: if remove_accessor_blank_lines(cx.options) {
                0
            } else {
                1
            },
            force_blank: blank::rule(cx),
            ..block_style(cx)
        };
        return with_braces(cx, node, |open, close| {
            block_between(cx, node, open, close, &style)
        });
    }
    let mut parts = Vec::new();
    for (i, e) in els.iter().enumerate() {
        if i > 0 {
            parts.push(doc::lit(" "));
        }
        parts.push(el(cx, e)?);
    }
    Ok(doc::concat(parts))
}

/// `get { return x; }`: with `inline_accessor_bodies` the body of an accessor stays on its line
/// when it is a single simple statement without comments and everything fits; otherwise the
/// braces get their own lines. `None` for any accessor this does not apply to.
pub fn accessor_declaration(cx: &FormatCtx<'_>, accessor: &SyntaxNode) -> Option<R> {
    let g = cx.grammar;
    let els = children(cx, accessor);
    let (SyntaxElement::Node(block), head) = els.split_last()? else {
        return None;
    };
    let body = children(cx, block);
    let [
        SyntaxElement::Token(open),
        SyntaxElement::Node(statement),
        SyntaxElement::Token(close),
    ] = body.as_slice()
    else {
        return None;
    };
    let simple = block.kind() == k::BLOCK
        && matches!(
            statement.kind(),
            k::RETURN_STATEMENT | k::THROW_STATEMENT | k::EXPRESSION_STATEMENT
        );
    let has_comment = accessor
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .any(|t| g.has_tag(t.kind(), "comment"));
    if !simple || has_comment {
        return None;
    }
    Some((|| {
        let head = seq_of(cx, accessor, head)?;
        let brace = sep_doc(cx, Sep::Brace("accessors"));
        let statement = doc::shared(cx.format(statement)?);
        if statement.clone().propagate_breaks() {
            return Ok(doc::concat(vec![head, brace, cx.format(block)?]));
        }
        let (open, close) = (cx.token(open), cx.token(close));
        let one_line = doc::concat(vec![
            head.clone(),
            doc::lit(" "),
            open.clone(),
            doc::lit(" "),
            statement.clone(),
            doc::lit(" "),
            close.clone(),
        ]);
        let expanded = doc::concat(vec![
            head,
            brace,
            open,
            doc::indent(doc::concat(vec![doc::hardline(), statement])),
            doc::hardline(),
            close,
        ]);
        Ok(doc::conditional_group(vec![one_line, expanded]))
    })())
}

/// The text of `: A, B` after a type, one further type per line when it breaks.
fn base_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut els = children(cx, node).into_iter();
    let Some(SyntaxElement::Token(colon)) = els.next() else {
        return Err(FormatError::Shape("base list without `:`".into()));
    };
    let mut first = None;
    let mut rest = Vec::new();
    for e in els {
        match e {
            SyntaxElement::Node(n) if first.is_none() => first = Some(cx.format(&n)?),
            SyntaxElement::Node(n) => {
                rest.push(doc::line());
                rest.push(cx.format(&n)?);
            }
            SyntaxElement::Token(comma) => rest.push(cx.token(&comma)),
        }
    }
    Ok(doc::concat(vec![
        cx.token(&colon),
        doc::lit(" "),
        first.unwrap_or(doc::Doc::Nil),
        doc::indent(doc::concat(rest)),
    ]))
}

/// The children of a node in front of its body. With `base_list_layout: hanging` the `#heritage`
/// base list is a clause of the type's header: it stays on the line while it fits, else it moves
/// to the next line.
fn heading(cx: &FormatCtx<'_>, node: &SyntaxNode, els: &[SyntaxElement]) -> R {
    let heritage = els
        .iter()
        .position(|e| cx.grammar.has_tag(e.kind(), "heritage"));
    let (Some(at), true) = (heritage, hanging_base_list(cx.options)) else {
        return seq_of(cx, node, els);
    };
    let SyntaxElement::Node(list) = &els[at] else {
        return seq_of(cx, node, els);
    };
    let style = HeaderStyle {
        grouped: true,
        continuation: 1,
        ..HeaderStyle::default()
    };
    let mut parts = vec![header(
        seq_of(cx, node, &els[..at])?,
        vec![(doc::line(), base_list(cx, list)?)],
        &style,
    )];
    if at + 1 < els.len() {
        parts.push(sep_doc(cx, separator(cx, node, &els[at], &els[at + 1])));
        parts.push(seq_of(cx, node, &els[at + 1..])?);
    }
    Ok(doc::concat(parts))
}

/// `: base(args)` of a constructor, on its own line below the signature.
pub fn constructor_initializer(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    Ok(doc::indent(doc::concat(vec![
        doc::hardline(),
        seq_of(cx, node, &els)?,
    ])))
}

/// Labels one per line, then the statements indented below them.
pub fn switch_section(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let g = cx.grammar;
    let (labels, statements): (Vec<_>, Vec<_>) = nodes_of(&children(cx, node))
        .into_iter()
        .partition(|n| g.has_tag(n.kind(), "switch-label"));
    if flat::inside(cx, node) {
        let mut parts = Vec::new();
        for n in labels.iter().chain(&statements) {
            if !parts.is_empty() {
                parts.push(doc::lit(" "));
            }
            parts.push(cx.format(n)?);
        }
        return Ok(doc::concat(parts));
    }
    let mut parts = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        if i > 0 {
            parts.push(doc::hardline());
        }
        parts.push(cx.format(label)?);
    }
    // `indent_switch_section_block: false`: a block that is the only statement of a section is not indented.
    if !indent_switch_section_block(cx.options)
        && matches!(statements.as_slice(), [only] if only.kind() == k::BLOCK)
    {
        parts.push(doc::hardline());
        parts.push(cx.format(&statements[0])?);
    } else if !statements.is_empty() {
        let on_label_line = preserve_single_line_statements(cx.options)
            && flat::follows_on_line(cx, &statements[0]);
        // A blank line after the last label stays.
        let blank_after_label = !on_label_line
            && cx
                .first_token(&statements[0])
                .is_some_and(|t| comments::blank_lines_before(g, &t) > 0);
        parts.push(doc::indent(doc::concat(vec![
            if on_label_line {
                doc::lit(" ")
            } else {
                doc::hardline()
            },
            if blank_after_label {
                doc::hardline()
            } else {
                doc::Doc::Nil
            },
            statement_sequence_with(
                cx,
                &statements,
                1,
                blank::rule(cx),
                preserve_single_line_statements(cx.options),
            )?,
        ])));
    }
    Ok(doc::concat(parts))
}

fn arms_style(cx: &FormatCtx<'_>, node: &SyntaxNode) -> ListStyle {
    let enum_in_line = node.kind() == k::ENUM_DECLARATION && flat::inside(cx, node);
    ListStyle {
        layout: if enum_in_line {
            ListLayout::Flat
        } else {
            ListLayout::Break
        },
        padding: enum_in_line,
        trailing_comma: trailing_comma(cx.options),
        empty: empty_body_text(cx.options),
        expand_empty: node.kind() == k::ENUM_DECLARATION && !keeps_empty_inline(cx, node),
        force_blank: blank::rule(cx).filter(|_| node.kind() == k::ENUM_DECLARATION),
        ..ListStyle::default()
    }
}

/// `x switch`, then the arms between braces on their own lines.
pub fn switch_expression(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    with_braces(cx, node, |open, close| {
        delimited_list_between(cx, node, open, close, &arms_style(cx, node))
    })
}

pub fn enum_declaration(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    with_braces(cx, node, |open, close| {
        delimited_list_between(cx, node, open, close, &arms_style(cx, node))
    })
}
