//! concept: block-and-statements
//! what: Braced blocks and statement sequences with blank-line rules.
//! tools: Prettier statements, google-java-format blocks, ktlint curly-spacing
//! reads: tag:block

use super::{BlankRule, Rendered, Slot, container, container_between, gap, render_slots};
use crate::FormatCtx;
use crate::comments::{self, Comment};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

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
    /// An empty block is `empty` when its line fits, else the closing delimiter goes on a line of its own.
    pub empty_fits: bool,
    /// Items that share a source line (no comment between) stay on one line, separated by a space.
    pub keep_same_line: bool,
    /// A blank line after the opening delimiter stays.
    pub keep_leading_blank: bool,
    /// A blank line before the closing delimiter stays (google-java-format: before the `}` that an
    /// `else` or `catch` follows).
    pub keep_trailing_blank: bool,
    /// The items are not indented inside the delimiters (a C++ namespace).
    pub flat: bool,
    /// A block of one item and no comments stays on one line, `{ item }`, when it fits.
    pub fit_single: bool,
    /// The width of the signature before the block written on one line, when the source breaks it
    /// over several lines: the block stays on one line when signature and block fit one line (0: the
    /// block fits the line it stands on).
    pub fit_prefix: usize,
    /// Items of these kinds get a blank line before them by `blank_mode` (`never`, `always`, or
    /// `logical_block`: unless a comment stands in front); `leave` or no kinds: as the source has.
    pub blank_before: Vec<SyntaxKind>,
    pub blank_mode: &'static str,
}

impl BlockStyle {
    /// What `Default` gives, for `const` styles: `BlockStyle { .. BlockStyle::DEFAULT }`.
    pub const DEFAULT: BlockStyle = BlockStyle {
        max_blank_lines: 1,
        empty: "",
        force_blank: None,
        expand_empty: false,
        empty_fits: false,
        keep_same_line: false,
        keep_leading_blank: false,
        keep_trailing_blank: false,
        flat: false,
        fit_single: false,
        fit_prefix: 0,
        blank_before: Vec::new(),
        blank_mode: "leave",
    };
}

impl Default for BlockStyle {
    fn default() -> Self {
        BlockStyle::DEFAULT
    }
}

/// The parts of a compound statement that follow its first block: the kinds of the clauses
/// (`catch`, `finally`) and the keywords (`else`).
#[derive(Clone, Copy, Debug)]
pub struct Continuations<'a> {
    pub clauses: &'a [SyntaxKind],
    pub words: &'a [&'a str],
}

impl Continuations<'_> {
    /// `base` for a block of a compound statement: an empty block that is part of a longer
    /// statement keeps its closing delimiter on a line of its own, and a blank line before the
    /// closing delimiter stays when a clause or keyword follows the block.
    pub fn style(&self, cx: &FormatCtx<'_>, block: &SyntaxNode, base: BlockStyle) -> BlockStyle {
        BlockStyle {
            expand_empty: block.children().next().is_none() && self.within(cx, block),
            keep_trailing_blank: self.follows(cx, block),
            ..base
        }
    }

    /// Does the next significant element after `block` start a clause or is a keyword of the set?
    fn follows(&self, cx: &FormatCtx<'_>, block: &SyntaxNode) -> bool {
        let mut next = block.next_sibling_or_token();
        while let Some(e) = &next {
            if !matches!(e, SyntaxElement::Token(t) if cx.grammar.is_trivia(t.kind())) {
                break;
            }
            next = e.next_sibling_or_token();
        }
        next.is_some_and(|e| match &e {
            SyntaxElement::Node(n) => self.clauses.contains(&n.kind()),
            SyntaxElement::Token(t) => self.words.contains(&t.text()),
        })
    }

    /// Is `block` part of a longer statement: the body of a clause, of a `#try` that has one, or
    /// of a statement that has a keyword of the set?
    fn within(&self, cx: &FormatCtx<'_>, block: &SyntaxNode) -> bool {
        let Some(parent) = block.parent() else {
            return false;
        };
        self.clauses.contains(&parent.kind())
            || (cx.grammar.has_tag(parent.kind(), "try")
                && parent.children().any(|c| self.clauses.contains(&c.kind())))
            || parent
                .children_with_tokens()
                .any(|c| c.as_token().is_some_and(|t| self.words.contains(&t.text())))
    }
}

/// Where a labeled statement puts its label.
#[derive(Clone, Copy, Debug, Default)]
pub struct LabelStyle {
    /// The body goes on the next line (at the indentation of the statement), else after the label.
    pub body_on_next_line: bool,
    /// Levels the label moves out of the statement's indentation.
    pub outdent: usize,
    /// A blank line after the label in the source stays.
    pub keep_blank: bool,
}

/// A labeled statement: the `head` (`label:`), then the `body`. A `bare` body (an empty
/// statement) follows the head directly.
pub fn labeled(head: Doc, body: Doc, style: LabelStyle, bare: bool) -> Doc {
    let head = if style.outdent > 0 {
        doc::outdent(style.outdent, head)
    } else {
        head
    };
    let sep = if bare && !style.body_on_next_line {
        Doc::Nil
    } else if style.body_on_next_line {
        doc::hardline()
    } else {
        doc::lit(" ")
    };
    doc::concat(vec![head, sep, body])
}

/// A `label: body` node (`#labeled`): `print` prints a run of its elements (the label with its
/// colon, then the body). A node of fewer than three elements is a shape error.
pub fn labeled_statement(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: LabelStyle,
    print: &dyn Fn(&[SyntaxElement]) -> Result<Doc, FormatError>,
) -> Result<Doc, FormatError> {
    let els = crate::seq::kids(cx, node);
    if els.len() < 3 {
        return Err(FormatError::Shape("labeled statement".into()));
    }
    let bare = els[2]
        .as_node()
        .is_some_and(|b| cx.grammar.has_tag(b.kind(), "empty-statement"));
    let body = print(&els[2..])?;
    let blank = style.keep_blank
        && !bare
        && style.body_on_next_line
        && cx
            .first_token(node)
            .and_then(|_| match &els[2] {
                SyntaxElement::Node(n) => cx.first_token(n),
                SyntaxElement::Token(t) => Some(t.clone()),
            })
            .is_some_and(|t| comments::blank_lines_before(cx.grammar, &t) > 0);
    let body = if blank {
        doc::concat(vec![doc::hardline(), body])
    } else {
        body
    };
    Ok(labeled(print(&els[..2])?, body, style, bare))
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

/// [`block`] where comments after the opening delimiter move inside, to lines of their own; with
/// `open_comment_inside_parents` only in a body whose parent has one of the kinds.
pub fn block_open_inside(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(cx, container(cx, node)?, style, opens_inside(cx, node))
}

/// Does the body `node` move the comments after its opening delimiter inside
/// ([`block_open_inside`])?
pub fn opens_inside(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let parents = core_config::opt::OPEN_COMMENT_INSIDE_PARENTS;
    cx.options.strings(parents).next().is_none()
        || node
            .parent()
            .is_some_and(|p| cx.options.list_has(parents, cx.grammar.name(p.kind())))
}

/// The body `els` of `node` when it is `{` and `}` only, kept on one line (or split when the
/// source breaks after `{`); `None` when it holds comments that must stay in the body, or one
/// that [`block_open_inside`] moves inside.
pub fn empty_braces(cx: &FormatCtx<'_>, node: &SyntaxNode, els: &[SyntaxElement]) -> Option<Doc> {
    let [SyntaxElement::Token(open), SyntaxElement::Token(close)] = els else {
        return None;
    };
    if open.text() != "{" || close.text() != "}" {
        return None;
    }
    if !comments::leading_comments(cx.grammar, close).is_empty()
        || (comments::has_trailing_comment(cx.grammar, open) && opens_inside(cx, node))
    {
        return None;
    }
    let gap = if comments::break_after(cx.grammar, open) {
        doc::hardline()
    } else if open.text_range().end() != close.text_range().start() {
        doc::lit(" ")
    } else {
        doc::lit("")
    };
    Some(doc::concat(vec![cx.token(open), gap, cx.token(close)]))
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
        container_between(cx, node, Some((open, close)), &[])?,
        style,
        false,
    )
}

/// A braced body of entries of the kinds of `single_line_body_kinds` that is on one line in the
/// source stays on it, its entries a blank apart (`{ A, B }`); `None` for any other body.
pub fn single_line_body(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    els: &[SyntaxElement],
) -> Option<Result<Doc, FormatError>> {
    let [_, entries @ .., _] = els else {
        return None;
    };
    let kinds = |e: &SyntaxElement| {
        cx.options.list_has(
            core_config::opt::SINGLE_LINE_BODY_KINDS,
            cx.grammar.name(e.kind()),
        )
    };
    if entries.is_empty() || !entries.iter().all(kinds) || crate::seq::multiline(cx, node) {
        return None;
    }
    let all: Result<Vec<_>, _> = els.iter().map(|e| crate::seq::el(cx, e)).collect();
    Some(all.map(|d| doc::join(&doc::lit(" "), d)))
}

/// Comments in front of or after an opening delimiter, which move to the first body line
/// (`if (c) // x` newline `{`).
pub fn open_comments(cx: &FormatCtx<'_>, open: &SyntaxToken) -> Vec<Comment> {
    let g = cx.grammar;
    // Leading comments a shape holds back are printed by that shape.
    let mut all = if cx.is_held_leading(open) {
        Vec::new()
    } else {
        comments::leading_comments(g, open)
    };
    all.extend(comments::trailing_comments(g, open));
    all
}

/// `{ head body }`: an opening brace, an optional head on its line, the body indented below and
/// the closing brace. Comments after a head-less `{` move to the first body line and force the
/// break; otherwise it breaks when the source did or the body does.
pub fn headed_braces(
    cx: &FormatCtx<'_>,
    open: &SyntaxToken,
    close: &SyntaxToken,
    head: Option<Doc>,
    body: Doc,
    source_multiline: bool,
) -> Doc {
    let moved = if head.is_none() {
        open_comments(cx, open)
    } else {
        Vec::new()
    };
    let dangling = comments::leading_comments(cx.grammar, close);
    let broken = source_multiline || body.will_break() || !moved.is_empty() || !dangling.is_empty();
    let mut parts = vec![if moved.is_empty() {
        cx.token(open)
    } else {
        cx.token_text(open)
    }];
    if let Some(h) = head {
        parts.push(doc::lit(" "));
        parts.push(h);
    }
    parts.push(doc::indent(doc::concat(vec![
        super::dangling_comments_in(&cx.comment_style, &moved, false),
        doc::line(),
        body,
        super::dangling_comments_in(&cx.comment_style, &dangling, false),
    ])));
    parts.push(doc::line());
    parts.push(cx.token_text(close));
    parts.push(cx.trailing_comments_doc(close));
    let all = doc::concat(parts);
    if broken {
        doc::group_broken(all)
    } else {
        doc::group(all)
    }
}

fn block_of(
    cx: &FormatCtx<'_>,
    c: super::Container,
    style: &BlockStyle,
    open_inside: bool,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let moved: Vec<Comment> = if open_inside {
        open_comments(cx, &c.open)
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
    // (a body that holds a block of its own, `SCOPE_EXIT { a(); };`, is no short body; a lambda's is)
    let nested = |n: &SyntaxNode| {
        n.descendants().any(|d| {
            g.has_tag(d.kind(), "block")
                && d.parent().is_some_and(|p| !g.has_tag(p.kind(), "function"))
        })
    };
    let single = c.slots.len() == 1 && moved.is_empty() && !nested(&first.node);
    let mut rendered = render_slots(cx, &c.slots, true)?;
    for i in 1..rendered.len() {
        if style.blank_before.contains(&rendered[i].first.kind()) {
            // (a comment in front, a comment at the end of the line before or a directive before keeps the source)
            let last = &rendered[i - 1].last;
            let directive = cx.last_token(last).is_some_and(|t| {
                t.parent_ancestors()
                    .any(|n| g.has_tag(n.kind(), "directive"))
            });
            let comment = directive
                || cx
                    .first_token(&rendered[i].first)
                    .is_some_and(|t| !comments::leading_comments(g, &t).is_empty())
                || cx
                    .last_token(last)
                    .is_some_and(|t| !comments::trailing_comments(g, &t).is_empty());
            let item = &mut rendered[i];
            item.blank_before = match style.blank_mode {
                "never" => 0,
                "always" => item.blank_before.max(1),
                "logical_block" if !comment => item.blank_before.max(1),
                _ => item.blank_before,
            };
        }
    }
    let items = sequence_items(
        cx,
        rendered,
        style.max_blank_lines,
        style.force_blank,
        style.keep_same_line,
    );
    if style.fit_single
        && single
        && !lead_blank
        && comments::leading_comments(g, &c.close).is_empty()
    {
        let Ends { opening, closing } = ends;
        let parts = |items| {
            vec![
                opening.clone(),
                doc::indent(doc::concat(vec![doc::line(), items])),
                doc::line(),
                closing.clone(),
            ]
        };
        if style.fit_prefix > 0 {
            let items = doc::shared(items);
            let flat = doc::group(doc::concat(parts(items.clone())));
            let width = flat.flat_width().map(|w| w + style.fit_prefix + 1);
            let apart = doc::group_broken(doc::concat(parts(items)));
            return Ok(match width {
                Some(w) => doc::fits_below(0, w, flat, apart),
                None => apart,
            });
        }
        // (shared: a header that breaks prints the body over lines from a copy of this group)
        return Ok(doc::label(
            doc::SHORT_BODY,
            doc::group(doc::concat(parts(doc::shared(items)))),
        ));
    }
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
        cx.token_text(open)
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
        let d = doc::concat(vec![
            opening,
            if style.expand_empty {
                doc::hardline()
            } else if style.empty_fits && style.empty.is_empty() {
                doc::softline()
            } else {
                doc::lit(style.empty)
            },
            closing,
        ]);
        return if style.empty_fits && !style.expand_empty {
            doc::group(d)
        } else {
            d
        };
    }
    let blank_before_close = body.is_some()
        && (!dangling.is_empty()
            || style.keep_trailing_blank
            || style.max_blank_lines == usize::MAX)
        && comments::blank_lines_before(g, close) > 0;
    let first_hardline = match (&body, lead_blank) {
        (None, _) => Doc::Nil,
        (Some(_), true) => doc::concat(vec![doc::hardline(), doc::hardline()]),
        (Some(_), false) => doc::hardline(),
    };
    let inner = doc::concat(vec![
        super::dangling_comments_in(&cx.comment_style, moved, false),
        first_hardline,
        body.unwrap_or(Doc::Nil),
        super::dangling_comments_in(&cx.comment_style, &dangling, blank_before_close),
        if dangling.is_empty() && blank_before_close {
            doc::hardline()
        } else {
            Doc::Nil
        },
    ]);
    doc::concat(vec![
        opening,
        if style.flat {
            inner
        } else {
            doc::indent(inner)
        },
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

/// The blank lines in front of `first` that the source has (at most `max_blank_lines`), one line
/// break each, to follow the line break that ends the line before.
pub fn source_blank_lines(
    cx: &FormatCtx<'_>,
    first: Option<&SyntaxToken>,
    max_blank_lines: usize,
) -> Doc {
    let count = first.map_or(0, |t| comments::blank_lines_before(cx.grammar, t));
    doc::concat(
        (0..count.min(max_blank_lines))
            .map(|_| doc::hardline())
            .collect(),
    )
}

/// A line break, the blank lines the source has in front of the first item, and the items as a
/// [`statement_sequence`]: what follows a line that heads them (a rule line, `else`).
pub fn statements_after_break(
    cx: &FormatCtx<'_>,
    items: &[SyntaxNode],
    max_blank_lines: usize,
) -> Result<Doc, FormatError> {
    let first = items.first().and_then(|n| cx.first_token(n));
    Ok(doc::concat(vec![
        doc::hardline(),
        source_blank_lines(cx, first.as_ref(), max_blank_lines),
        statement_sequence(cx, items, max_blank_lines)?,
    ]))
}

/// [`statement_sequence`] with a rule for blank lines the source does not have.
pub fn statement_sequence_with(
    cx: &FormatCtx<'_>,
    items: &[SyntaxNode],
    max_blank_lines: usize,
    force_blank: Option<BlankRule>,
    keep_same_line: bool,
) -> Result<Doc, FormatError> {
    let slots: Vec<Slot> = items.iter().map(|n| Slot::new(n.clone())).collect();
    Ok(sequence_items(
        cx,
        render_slots(cx, &slots, true)?,
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
    let annotation = |c: &core_syntax::SyntaxElement| g.has_tag(c.kind(), "annotation");
    // A declaration whose annotations follow the source, or whose only annotation has no
    // arguments (`annotation_inline`).
    let inline = |m: &SyntaxNode| {
        let o = cx.options;
        let Some(d) = m.parent() else { return false };
        if o.list_has(core_config::opt::ANNOTATION_SOURCE_KINDS, g.name(d.kind())) {
            return true;
        }
        o.is(core_config::opt::ANNOTATION_INLINE, "single") && {
            let mut all = m.children_with_tokens().filter(&annotation);
            let first = all.next();
            all.next().is_none() && first.is_some_and(|a| !super::has_arguments(cx, &a))
        }
    };
    if g.has_tag(n.kind(), "modifiers") {
        n.parent().is_some_and(|d| declaration(&d))
            && !inline(n)
            && n.children_with_tokens()
                .filter(|e| !g.is_trivia(e.kind()) && !e.text_range().is_empty())
                .last()
                .is_some_and(|c| annotation(&c))
    } else if g.has_tag(n.kind(), "annotation") {
        n.parent().is_some_and(|m| {
            g.has_tag(m.kind(), "modifiers")
                && m.parent().is_some_and(|d| declaration(&d))
                && !inline(&m)
        })
    } else {
        false
    }
}
