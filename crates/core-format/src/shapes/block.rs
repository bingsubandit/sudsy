//! concept: block-and-statements
//! what: Braced blocks and statement sequences with blank-line rules.
//! tools: Prettier statements, google-java-format blocks, ktlint curly-spacing
//! reads: tag:block

use super::{
    BlankRule, Rendered, Slot, container, container_between, dangling_comments, gap, render_slots,
};
use crate::FormatCtx;
use crate::comments::{self, Comment};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxNode, SyntaxToken};

#[derive(Clone, Debug)]
pub struct BlockStyle {
    /// Blank lines between items kept from the source (`usize::MAX`: all of them, and a blank
    /// line before the closing delimiter stays too).
    pub max_blank_lines: usize,
    /// What stands between the delimiters of an empty block.
    pub empty: &'static str,
    /// Items between which a blank line is required.
    pub force_blank: Option<BlankRule>,
    /// An empty block puts the closing delimiter on a line of its own instead of `empty`.
    pub expand_empty: bool,
    /// Items that share a source line (no comment between) stay on one line, separated by a space.
    pub keep_same_line: bool,
    /// A blank line after the opening delimiter stays.
    pub keep_leading_blank: bool,
}

impl Default for BlockStyle {
    fn default() -> Self {
        BlockStyle {
            max_blank_lines: 1,
            empty: "",
            force_blank: None,
            expand_empty: false,
            keep_same_line: false,
            keep_leading_blank: false,
        }
    }
}

/// `open`, the items one per line and indented, `close`. An empty block is `open close`. Separator
/// tokens between items are kept as written.
pub fn block(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(cx, container(cx, node)?, style, false)
}

/// [`block`] where comments after the opening delimiter move inside, to lines of their own.
pub fn block_open_inside(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(cx, container(cx, node)?, style, true)
}

/// [`block`] for the delimiters `open` and `close` among the children of `node`.
pub fn block_between(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(
        cx,
        container_between(cx, node, Some((open, close)))?,
        style,
        false,
    )
}

fn block_of(
    cx: &FormatCtx<'_>,
    c: super::Container,
    style: &BlockStyle,
    open_inside: bool,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let moved: Vec<Comment> = if open_inside {
        comments::trailing_comments(g, &c.open)
    } else {
        Vec::new()
    };
    let Some(first) = c.slots.first() else {
        let ends = ends_of(cx, &c.open, &c.close, &moved);
        return Ok(braces_of(cx, ends, &c.close, None, false, &moved, style));
    };
    let lead_blank = if moved.is_empty() {
        style.keep_leading_blank && blank_before(cx, &first.node)
    } else {
        // A blank line after the comment that moved inside stays.
        cx.first_token(&first.node)
            .is_some_and(|t| comments::blank_lines_before(g, &t) > 0)
    };
    let ends = ends_of(cx, &c.open, &c.close, &moved);
    let items = sequence_items(
        cx,
        render_slots(cx, &c.slots)?,
        style.max_blank_lines,
        style.force_blank,
        style.keep_same_line,
    );
    Ok(braces_of(
        cx,
        ends,
        &c.close,
        Some(items),
        lead_blank,
        &moved,
        style,
    ))
}

/// Blank lines the source has in front of the node (comments in front of it excluded).
pub fn blank_before(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    super::leading_info(cx, node).is_ok_and(|(blank, ..)| blank > 0)
}

/// `open`, a body already laid out (`None`: nothing in it), `close`, with the comments in front
/// of `close` inside, after the body. `lead_blank`: a blank line stays after `open`.
pub fn braces(
    cx: &FormatCtx<'_>,
    open: &SyntaxToken,
    close: &SyntaxToken,
    body: Option<Doc>,
    lead_blank: bool,
    style: &BlockStyle,
) -> Doc {
    let ends = ends_of(cx, open, close, &[]);
    braces_of(cx, ends, close, body, lead_blank, &[], style)
}

/// The delimiters as printed. Made before the items are, so that the comments they own are
/// taken in source order.
struct Ends {
    opening: Doc,
    closing: Doc,
}

fn ends_of(cx: &FormatCtx<'_>, open: &SyntaxToken, close: &SyntaxToken, moved: &[Comment]) -> Ends {
    let opening = if moved.is_empty() {
        cx.token(open)
    } else {
        doc::concat(vec![cx.leading_comments_doc(open), cx.token_text(open)])
    };
    let closing = doc::concat(vec![cx.token_text(close), cx.trailing_comments_doc(close)]);
    Ends { opening, closing }
}

fn braces_of(
    cx: &FormatCtx<'_>,
    ends: Ends,
    close: &SyntaxToken,
    body: Option<Doc>,
    lead_blank: bool,
    moved: &[Comment],
    style: &BlockStyle,
) -> Doc {
    let g = cx.grammar;
    let dangling = comments::leading_comments(g, close);
    let Ends { opening, closing } = ends;
    if body.is_none() && dangling.is_empty() && moved.is_empty() {
        return doc::concat(vec![
            opening,
            if style.expand_empty {
                doc::hardline()
            } else {
                doc::lit(style.empty)
            },
            closing,
        ]);
    }
    let blank_before_close = body.is_some()
        && (!dangling.is_empty() || style.max_blank_lines == usize::MAX)
        && comments::blank_lines_before(g, close) > 0;
    let first_hardline = match (&body, lead_blank) {
        (None, _) => Doc::Nil,
        (Some(_), true) => doc::concat(vec![doc::hardline(), doc::hardline()]),
        (Some(_), false) => doc::hardline(),
    };
    doc::concat(vec![
        opening,
        doc::indent(doc::concat(vec![
            dangling_comments(moved, false),
            first_hardline,
            body.unwrap_or(Doc::Nil),
            dangling_comments(&dangling, blank_before_close),
            if dangling.is_empty() && blank_before_close {
                doc::hardline()
            } else {
                Doc::Nil
            },
        ])),
        doc::hardline(),
        closing,
    ])
}

/// Items one per line with the source's blank lines (at most `max_blank_lines`) between them.
pub fn statement_sequence(
    cx: &FormatCtx<'_>,
    items: &[SyntaxNode],
    max_blank_lines: usize,
) -> Result<Doc, FormatError> {
    statement_sequence_with(cx, items, max_blank_lines, None, false)
}

/// [`statement_sequence`] with a rule for blank lines the source does not have.
pub fn statement_sequence_with(
    cx: &FormatCtx<'_>,
    items: &[SyntaxNode],
    max_blank_lines: usize,
    force_blank: Option<BlankRule>,
    keep_same_line: bool,
) -> Result<Doc, FormatError> {
    let slots: Vec<Slot> = items
        .iter()
        .map(|n| Slot {
            node: n.clone(),
            sep: None,
            hole: false,
        })
        .collect();
    Ok(sequence_items(
        cx,
        render_slots(cx, &slots)?,
        max_blank_lines,
        force_blank,
        keep_same_line,
    ))
}

fn sequence_items(
    cx: &FormatCtx<'_>,
    mut items: Vec<Rendered>,
    max_blank: usize,
    force_blank: Option<BlankRule>,
    keep_same_line: bool,
) -> Doc {
    let mut parts = Vec::new();
    for i in 0..items.len() {
        if i > 0 && keep_same_line && shares_line(cx, &items[i - 1], &items[i]) {
            parts.push(doc::lit(" "));
        } else if i > 0 {
            parts.push(gap(
                cx,
                &items[i - 1],
                &items[i],
                doc::hardline(),
                max_blank,
                force_blank,
                false,
            ));
        }
        parts.push(std::mem::replace(&mut items[i].doc, Doc::Nil));
        if let Some(sep) = &items[i].sep {
            parts.push(cx.token(sep));
        }
    }
    doc::concat(parts)
}

/// Do two neighbouring items stand on one source line, each of them written on one line, with
/// no comment or separator token between them?
fn shares_line(cx: &FormatCtx<'_>, prev: &Rendered, next: &Rendered) -> bool {
    let g = cx.grammar;
    let (Some(start), Some(end), Some(after)) = (
        cx.first_token(&prev.first),
        cx.last_token(&prev.last),
        cx.first_token(&next.first),
    ) else {
        return false;
    };
    let range = core_syntax::TextRange::new(start.text_range().start(), end.text_range().end());
    prev.sep.is_none()
        && !comments::break_after(g, &end)
        && !cx.has_line_break(&prev.first, range)
        && comments::trailing_comments(g, &end).is_empty()
        && comments::leading_comments(g, &after).is_empty()
}

/// A document of statements: the root's items as a [`statement_sequence_with`], then the comments
/// at the end of the file, which the zero-width end-of-file token owns.
pub fn document(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    max_blank_lines: usize,
    force_blank: Option<BlankRule>,
) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence_with(
        cx,
        &items,
        max_blank_lines,
        force_blank,
        false,
    )?];
    let eof = cx
        .last_token(node)
        .or_else(|| node.last_token())
        .ok_or_else(|| FormatError::Shape("Document without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !items.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(crate::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

/// Items between which a blank line stands when the block's items are `#case` nodes and one of
/// them spans lines (or ends in a braced `#block`), and none otherwise.
pub fn spread_cases_style(cx: &FormatCtx<'_>, node: &SyntaxNode) -> BlockStyle {
    let g = cx.grammar;
    let spans = |n: &SyntaxNode| match (cx.first_token(n), cx.last_token(n)) {
        (Some(a), Some(b)) if a != b => cx.has_line_break(
            n,
            core_syntax::TextRange::new(a.text_range().start(), b.text_range().start()),
        ),
        _ => false,
    };
    let braced_body = |n: &SyntaxNode| {
        n.children().last().is_some_and(|b| {
            g.has_tag(b.kind(), "block")
                && cx.first_token(&b).is_some_and(|t| t.text() == "{")
                && b.children().any(|c| !c.text_range().is_empty())
        })
    };
    let spread = node
        .children()
        .any(|n| g.has_tag(n.kind(), "case") && (spans(&n) || braced_body(&n)));
    BlockStyle {
        max_blank_lines: usize::from(spread),
        force_blank: spread.then_some(case_blank as BlankRule),
        ..BlockStyle::default()
    }
}

fn case_blank(cx: &FormatCtx<'_>, _: &SyntaxNode, node: &SyntaxNode) -> bool {
    cx.grammar.has_tag(node.kind(), "case")
}

/// Does `prev`, an element of a node, end in an annotation (tag `#annotation`) of a declaration
/// (tags `#modifiers` and `#declaration`), which the style puts on a line of its own?
pub fn annotation_line(cx: &FormatCtx<'_>, prev: &core_syntax::SyntaxElement) -> bool {
    let g = cx.grammar;
    let Some(n) = prev.as_node() else {
        return false;
    };
    let declaration = |d: &SyntaxNode| g.has_tag(d.kind(), "declaration");
    if g.has_tag(n.kind(), "modifiers") {
        let annotation = |c: &core_syntax::SyntaxElement| g.has_tag(c.kind(), "annotation");
        n.parent().is_some_and(|d| declaration(&d))
            && n.children_with_tokens()
                .filter(|e| !g.is_trivia(e.kind()) && !e.text_range().is_empty())
                .last()
                .is_some_and(|c| annotation(&c))
    } else if g.has_tag(n.kind(), "annotation") {
        n.parent().is_some_and(|m| {
            g.has_tag(m.kind(), "modifiers") && m.parent().is_some_and(|d| declaration(&d))
        })
    } else {
        false
    }
}
