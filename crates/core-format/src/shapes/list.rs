//! concept: delimited-list
//! what: Delimited lists flat or one item per line with trailing comma and magic-comma policy.
//! tools: Prettier arrays objects and parameters, ruff magic trailing comma, google-java-format lists
//! reads: tag:delimited-list

use super::{
    BlankRule, container, container_between, dangling_comments, gap, render_slots,
    separator_comments, tabular,
};
use crate::FormatCtx;
use crate::comments;
use crate::directives::Directive;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingComma {
    Never,
    /// Only when the list is laid out on several lines.
    WhenBroken,
    /// Always, on one line too (Python's one-item tuple `(a,)`).
    Always,
    /// `WhenBroken` when the source has a separator after the last item, else `Never`.
    AsSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListLayout {
    /// One line if it fits, else one item per line.
    Fit,
    /// Like `Fit`, but a list with a line break anywhere between its delimiters in the source
    /// (nested items included) stays expanded.
    PreserveBreak,
    /// Like `Fit`, but a line break between the opening delimiter and the first item keeps the
    /// list expanded (Prettier's objects).
    PreserveFirstBreak,
    /// Like `Fit`, but a separator after the last item in the source keeps the list expanded
    /// (Black's and ruff's magic trailing comma).
    MagicTrailingComma,
    /// Items packed onto lines as they fit (Prettier's arrays of numbers); the list breaks, with
    /// a trailing comma, only when it does not fit on one line.
    Fill,
    /// Like `Fit`, but a list stays expanded when a line break separates two items in the source,
    /// or stands both after the opening and before the closing delimiter.
    PreserveItemBreak,
    /// Always one item per line.
    Break,
    /// Never a line break between the delimiters (blank lines and `BreakAll` are ignored), however
    /// long the line gets, and its items are not indented: for lists the language cannot break,
    /// like TOML inline tables.
    Flat,
}

#[derive(Clone, Debug)]
pub struct ListStyle {
    pub separator: &'static str,
    /// Space inside the delimiters when the list is on one line: `{ a }` rather than `{a}`.
    pub padding: bool,
    pub trailing_comma: TrailingComma,
    pub layout: ListLayout,
    /// Blank lines between items kept from the source.
    pub max_blank_lines: usize,
    /// A `line` before the opening delimiter, inside the group: a space when the list is on one
    /// line and a line break (at the current indentation) when it is expanded. Braces that sit
    /// on their own line only when the list breaks.
    pub leading_line: bool,
    /// What stands between the delimiters of an empty list.
    pub empty: &'static str,
    /// An empty list puts the closing delimiter on a line of its own instead of `empty`.
    pub expand_empty: bool,
    /// Items between which a blank line is required.
    pub force_blank: Option<BlankRule>,
    /// The closing delimiter follows the last item on its line when the list breaks (unless
    /// comments stand in front of it).
    pub hug_close: bool,
    /// No group of its own unless the source breaks the list: it breaks with the group around it
    /// (a type literal in a hugged parameter breaks with the parameter's pattern).
    pub shared: bool,
    /// The separator when the list breaks, if not `separator` (TypeScript members: `;` on one
    /// line, none or `;` per the `semi` option on several).
    pub broken_separator: Option<&'static str>,
    /// A blank line between two items in the source keeps the list expanded (Prettier's
    /// objects and parameters), instead of vanishing when the list fits.
    pub blank_breaks: bool,
    /// The list is expanded whatever its layout (the caller found a reason in the tree).
    pub force_break: bool,
    /// Fewer items than this stay on the line however long it gets, this many or more are one
    /// per line (0: off): Protobuf options.
    pub break_from: usize,
    /// Kinds of wrapper nodes whose children are items of the list (Protobuf's field options).
    pub unwrap: &'static [SyntaxKind],
    /// Comments after the opening delimiter stand on lines of their own inside the list.
    pub open_comments_own_line: bool,
    /// The items sit in a group of their own, inside the one of the delimiters: when they do not
    /// fit on the opening line they go together onto the next, and only when they do not fit
    /// there one per line (google-java-format's parameters). A single item has no such group.
    pub hang: bool,
    /// The group of the whole list, for a caller that asks later whether it broke.
    pub id: Option<doc::GroupId>,
    /// The group of the items (implies `hang`), for a caller that asks later whether they broke.
    pub inner_id: Option<doc::GroupId>,
    /// Indentation levels of the items; 0 counts as 1 (a continuation indent is 2).
    pub indent_levels: usize,
    /// A blank line in front of the comments before the closing delimiter stays.
    pub keep_blank_before_end_comments: bool,
    /// The ignore comments (`sudsy-ignore`, `fmt: off`) in front of an item keep it as written.
    pub ignore_marks: bool,
    /// Items that the source lays out in rows keep their rows: after the opening delimiter, one
    /// row per line (a row that is too wide wraps like a fill), the closing delimiter on a line
    /// of its own ([`tabular`](super::tabular)). Not for a list with comments.
    pub rows: bool,
    /// The items are filled (as with `ListLayout::Fill`) when each is shorter than this many
    /// characters in the source (0: never).
    pub fill_below: usize,
    /// The lines of the items start under the first item, not one indent in; when the widest
    /// item does not fit there, the items start on the line after the opening delimiter, this
    /// many columns in.
    pub align_open: bool,
    pub hang_indent: usize,
}

impl Default for ListStyle {
    fn default() -> Self {
        ListStyle {
            separator: ",",
            padding: false,
            trailing_comma: TrailingComma::Never,
            layout: ListLayout::Fit,
            max_blank_lines: 1,
            leading_line: false,
            empty: "",
            expand_empty: false,
            force_blank: None,
            hug_close: false,
            shared: false,
            broken_separator: None,
            blank_breaks: false,
            force_break: false,
            break_from: 0,
            unwrap: &[],
            open_comments_own_line: false,
            hang: false,
            id: None,
            inner_id: None,
            indent_levels: 1,
            keep_blank_before_end_comments: true,
            ignore_marks: true,
            rows: false,
            fill_below: 0,
            align_open: false,
            hang_indent: 4,
        }
    }
}

/// What a caller reads off the source of a list to choose its style.
pub struct ListFacts<'a> {
    /// The number of items.
    pub items: usize,
    /// The separator after the last item, if the source has one.
    pub last_sep: Option<&'a SyntaxToken>,
    /// The node of the only item, when the list has one item and it is a plain node.
    pub only: Option<&'a SyntaxNode>,
}

/// A style given, or chosen from the facts of the list.
#[derive(Clone, Copy)]
pub enum StyleOf<'a> {
    Given(&'a ListStyle),
    By(&'a dyn Fn(&ListFacts<'_>) -> ListStyle),
}

impl<'a> From<&'a ListStyle> for StyleOf<'a> {
    fn from(s: &'a ListStyle) -> Self {
        StyleOf::Given(s)
    }
}

/// `open item, item, item close` in a group; the node's directives `BreakAll` and `KeepTogether`
/// override the style's layout. Separator tokens are re-created from `style.separator`; their
/// comments are kept.
pub fn delimited_list<'a>(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: impl Into<StyleOf<'a>>,
) -> Result<Doc, FormatError> {
    list_of(cx, node, container(cx, node)?, style.into())
}

/// [`delimited_list`] for the delimiters `open` and `close` among the children of `node`; the
/// other children are the caller's business.
pub fn delimited_list_between(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
    style: &ListStyle,
) -> Result<Doc, FormatError> {
    list_of(
        cx,
        node,
        container_between(cx, node, Some((open, close)), style.unwrap)?,
        style.into(),
    )
}

/// [`delimited_list`] for the children of `items` between the delimiters `open` and `close`, which
/// stand outside it, in `node`.
pub fn delimited_list_around<'a>(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    items: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
    style: impl Into<StyleOf<'a>>,
) -> Result<Doc, FormatError> {
    list_of(
        cx,
        node,
        super::container_around(cx, items, open, close)?,
        style.into(),
    )
}

fn list_of(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    c: super::Container,
    style: StyleOf<'_>,
) -> Result<Doc, FormatError> {
    let owned;
    let style = match style {
        StyleOf::Given(s) => s,
        StyleOf::By(f) => {
            owned = f(&c.facts());
            &owned
        }
    };
    let g = cx.grammar;
    // The separator goes right after an item and its trailing comments after the separator:
    // `b, // c` and `b, /* c */` whether or not the source had a comma.
    let count = c.slots.len();
    let trailing = match style.trailing_comma {
        // (a list that never breaks has no place for it)
        _ if style.layout == ListLayout::Flat => TrailingComma::Never,
        TrailingComma::AsSource if c.slots.last().is_some_and(|s| s.sep.is_some()) => {
            TrailingComma::WhenBroken
        }
        TrailingComma::AsSource => TrailingComma::Never,
        t => t,
    };
    // Sparse (no allocation for a list without such comments): the slot and its last token.
    let mut tails: Vec<(usize, SyntaxToken)> = Vec::new();
    for (i, s) in c.slots.iter().enumerate() {
        let keeps = if i + 1 < count {
            s.sep.is_none()
        } else {
            trailing != TrailingComma::Never
        };
        if let Some(t) = (keeps && !s.hole)
            .then(|| s.last_token(cx))
            .flatten()
            .filter(|t| comments::has_trailing_comment(g, t))
        {
            cx.detach_trailing(&t);
            tails.push((i, t));
        }
    }
    let mut items = render_slots(cx, &c.slots, style.ignore_marks)?;
    attach_comments(cx, &mut items);
    let mut tail_comments: Vec<(usize, Doc)> = tails
        .iter()
        .map(|(i, t)| (*i, cx.reattach_trailing(t)))
        .collect();
    // Inline block comments in front of the closing delimiter stay against it; the others stand
    // on lines of their own inside the list.
    let mut dangling = comments::leading_comments(g, &c.close);
    let inline_close = !items.is_empty()
        && !dangling.is_empty()
        && dangling
            .iter()
            .all(|c| c.kind == comments::CommentKind::Block && c.breaks_after == 0);
    if inline_close {
        dangling.clear();
    }
    let mut open_cs = comments::trailing_comments(g, &c.open);
    // Block comments after the opening delimiter of an aligned list stand in front of the first
    // item, like those the source has on its line: both read the same on the next format.
    if style.align_open
        && !style.open_comments_own_line
        && !items.is_empty()
        && !open_cs.is_empty()
        && open_cs.iter().all(|c| c.kind == comments::CommentKind::Block)
    {
        let mut parts = Vec::new();
        for c in &open_cs {
            parts.push(crate::context::verbatim_text(&c.text()));
            if !(cx.comment_style.keep_attached && !crate::context::gap_follows(c)) {
                parts.push(doc::lit(" "));
            }
        }
        parts.push(std::mem::replace(&mut items[0].doc, Doc::Nil));
        items[0].doc = doc::concat(parts);
        open_cs.clear();
    }
    let open_comments = !open_cs.is_empty();
    let open_line_comment = open_cs
        .iter()
        .any(|c| c.kind != comments::CommentKind::Block);
    // Comments before the opening and after the closing delimiter belong outside the group: a
    // comment must not force the list itself to break.
    let before = cx.leading_comments_doc(&c.open);
    let after = cx.trailing_comments_doc(&c.close);
    let opening = doc::concat(vec![
        cx.token_text(&c.open),
        if style.open_comments_own_line && open_comments {
            doc::indent(dangling_comments(&open_cs, false))
        } else {
            open_comments_doc(
                cx,
                &open_cs,
                !items.is_empty(),
                style.padding && !items.is_empty(),
            )
        },
    ]);
    let closing = if inline_close {
        cx.close_token(&c.close)
    } else {
        cx.token_text(&c.close)
    };

    if items.is_empty() && dangling.is_empty() && !open_comments && c.prefix.is_none() {
        return Ok(doc::concat(vec![
            before,
            if style.leading_line {
                doc::lit(" ")
            } else {
                Doc::Nil
            },
            opening,
            if style.expand_empty {
                doc::hardline()
            } else {
                doc::lit(style.empty)
            },
            closing,
            after,
        ]));
    }

    if style.rows
        && c.prefix.is_none()
        && c.slots.len() >= 4
        && !commented(cx, node, &c.open, &c.close)
    {
        let els: Option<Vec<_>> = c.slots.iter().map(|s| s.element()).collect();
        let columns = els.and_then(|els| tabular::columns(cx, &els.iter().collect::<Vec<_>>()));
        if let Some(columns) = columns {
            let docs = items
                .iter_mut()
                .zip(&c.slots)
                .map(|(i, s)| {
                    let comma = if s.sep.is_some() {
                        doc::lit(",")
                    } else {
                        Doc::Nil
                    };
                    doc::concat(vec![std::mem::replace(&mut i.doc, Doc::Nil), comma])
                })
                .collect();
            let table = tabular::rows(docs, columns, &doc::line());
            return Ok(doc::concat(vec![
                before,
                doc::group(doc::concat(vec![
                    opening,
                    doc::indent(doc::concat(vec![doc::hardline(), table])),
                    doc::hardline(),
                    closing,
                ])),
                after,
            ]));
        }
    }

    let flat = style.layout == ListLayout::Flat
        || (style.break_from > 0
            && c.slots.len() < style.break_from
            && !commented(cx, node, &c.open, &c.close));
    let (line, soft) = if flat {
        (doc::lit(" "), Doc::Nil)
    } else {
        (doc::line(), doc::softline())
    };
    let pad = if style.padding {
        line.clone()
    } else if style.align_open {
        Doc::Nil
    } else {
        soft
    };
    let broken = style.broken_separator.unwrap_or(style.separator);
    let sep = match style.broken_separator {
        Some(b) => doc::if_break(doc::lit(b), doc::lit(style.separator)),
        None => doc::lit(style.separator),
    };
    let last = items.len().saturating_sub(1);
    // A hole (`[a, , b]`) is its separator: after the last item that comma is part of the list;
    // after a `#final` item (a rest element) no trailing comma is allowed.
    let (last_hole, last_final) = items.last().map_or((false, false), |i| {
        (i.hole, g.has_tag(i.last.kind(), "final"))
    });
    let fill = style.layout == ListLayout::Fill
        || (style.fill_below > 0 && c.slots.iter().all(|s| s.source_len(cx) < style.fill_below));
    // (a fill measures its contents flat: its trailing comma asks the group of the whole list)
    let fill_id = fill.then(doc::new_group_id);
    let mut fill_parts: Vec<Doc> = Vec::new();
    let mut lead = Vec::new();
    if !items.is_empty() || c.prefix.is_some() {
        lead.push(pad.clone());
    }
    if let Some(prefix) = &c.prefix {
        lead.push(cx.format(prefix)?);
        lead.push(doc::lit(" "));
    }
    let item_widths: Vec<Option<usize>> = items.iter().map(|i| i.doc.flat_width()).collect();
    let mut inner = Vec::new();
    let mut widest = 0;
    let mut widest_last = 0;
    let mut last_flat: Option<usize> = None;
    // The items on one line with the last one (a lambda) broken at its body, as a candidate.
    let hug_ok = style.align_open
        && !flat
        && c.slots.last().is_some_and(|sl| {
            sl.node
                .descendants()
                .any(|n| g.has_tag(n.kind(), "block"))
        })
        && !commented(cx, node, &c.open, &c.close)
        && items
            .iter()
            .all(|i| !i.verbatim && !i.hole && i.blank_before == 0);
    let hug_widths: Vec<Option<usize>> = if hug_ok {
        items.iter().map(|i| i.doc.flat_width()).collect()
    } else {
        Vec::new()
    };
    let hug_docs: Vec<Doc> = if hug_ok {
        items.iter().map(|i| doc::shared(i.doc.clone())).collect()
    } else {
        Vec::new()
    };
    for i in 0..items.len() {
        if i > 0 {
            // (a blank line in a fill is a hard break: the fill must not print `, ` before it)
            // (and after an item with a comment, which ends its line: where a fill packs the
            // next item depends on the width the comment gets, so a second format would move it)
            let line_comment = items[i - 1].sep.as_ref().is_some_and(|t| {
                let cs = comments::trailing_comments(g, t);
                cs.iter().any(|c| c.kind != comments::CommentKind::Block)
                    || (!cs.is_empty() && comments::break_after(g, t))
            }) || tail_comments.iter().any(|(j, d)| *j == i - 1 && !d.is_nil());
            let base = if fill && (items[i].blank_before > 0 || line_comment) {
                doc::hardline()
            } else {
                line.clone()
            };
            let between = gap(
                cx,
                &items[i - 1],
                &items[i],
                base,
                if flat { 0 } else { style.max_blank_lines },
                style.force_blank,
                style.blank_breaks,
            );
            if fill {
                fill_parts.push(doc::concat(std::mem::take(&mut inner)));
                fill_parts.push(between);
            } else {
                inner.push(between);
            }
        }
        let item = std::mem::replace(&mut items[i].doc, Doc::Nil);
        // (a chain of operators breaks at its operators, an item with a block (a lambda) at the
        // block: its first line is what must fit)
        let chain = c.slots.get(i).is_some_and(|sl| {
            let k = sl.node.kind();
            g.has_tag(k, "binary-chain")
                || g.has_tag(k, "conditional")
                || sl
                    .node
                    .descendants()
                    .any(|n| g.has_tag(n.kind(), "block"))
        });
        let w = if let Some(h) = c
            .slots
            .get(i)
            .and_then(|sl| {
                item.chain_widest()
                    .or_else(|| chain_head(cx, &sl.node))
                    .or_else(|| block_head(cx, &sl.node))
            })
        {
            h
        } else if chain || items.len() == 1 {
            item.head_width()
        } else {
            item.flat_width().unwrap_or_else(|| item.width_to_hardline())
        };
        // (a line comment after the item takes room on its line)
        let w = w + c.slots.get(i).map_or(0, |sl| {
            [sl.sep.clone(), sl.last_token(cx)]
                .iter()
                .flatten()
                .flat_map(|t| comments::trailing_comments(g, t))
                .filter(|c| c.kind != comments::CommentKind::Block)
                .map(|c| 1 + c.text().chars().count())
                .next()
                .unwrap_or(0)
        });
        if i == last {
            widest_last = w;
            // (the text after the list counts for an item that cannot break before its end)
            last_flat = if chain { None } else { item.flat_width() };
        } else {
            widest = widest.max(w);
        }
        // (an item that is too long for a line breaks inside itself, where the source breaks)
        // (the operands of a chain hang below its first operand)
        let item = if style.align_open
            && (i > 0 || items.len() > 1)
            && c.slots
                .get(i)
                .is_some_and(|sl| g.has_tag(sl.node.kind(), "binary-chain"))
        {
            doc::align(style.hang_indent, item)
        } else {
            item
        };
        inner.push(if style.align_open { doc::group(item) } else { item });
        // (`x: y,` kept as written in a list of `;` members ends with its separator too)
        let own_sep = !style.separator.is_empty()
            && items[i].verbatim
            && items[i].last_token(cx).is_some_and(|t| {
                t.text() == style.separator
                    || (matches!(t.text(), ";" | ",") && matches!(style.separator, ";" | ","))
            });
        inner.push(match (i == last, trailing) {
            _ if own_sep => Doc::Nil,
            (false, _) => sep.clone(),
            (true, _) if last_hole => doc::lit(style.separator),
            (true, _) if last_final => Doc::Nil,
            (true, TrailingComma::Never) => Doc::Nil,
            (true, TrailingComma::Always) => doc::lit(broken),
            (true, _) => match fill_id {
                Some(id) => doc::if_break_in(id, doc::lit(broken), Doc::Nil),
                None => doc::if_break(doc::lit(broken), Doc::Nil),
            },
        });
        if let Some((_, d)) = tail_comments
            .iter_mut()
            .find(|(j, d)| *j == i && !d.is_nil())
        {
            inner.push(std::mem::replace(d, Doc::Nil));
        }
        inner.push(separator_comments(cx, &items[i]));
    }
    let blank_before_close = style.keep_blank_before_end_comments
        && !dangling.is_empty()
        && comments::blank_lines_before(g, &c.close) > 0;
    inner.push(dangling_comments(&dangling, blank_before_close));
    let inner = if fill {
        fill_parts.push(doc::concat(inner));
        if style.align_open {
            // Each item after the first with the line before it in a group of its own: it moves to
            // the next line when it, and the text that follows it up to the next possible break
            // (the `)` and what stands after it, for the last), do not fit.
            let mut parts = fill_parts.into_iter();
            let mut out: Vec<Doc> = parts.next().into_iter().collect();
            let mut prev_breaks = out.first().is_some_and(Doc::will_break);
            while let (Some(between), Some(item)) = (parts.next(), parts.next()) {
                let breaks = item.will_break();
                out.push(if prev_breaks {
                    // (the item before spans lines: this one starts a line of its own)
                    doc::group_broken(doc::concat(vec![between, item]))
                } else if breaks && !between.will_break() {
                    // (an item with a forced break, a lambda body: it stays on the line when its
                    // first line fits, the group around would break at once)
                    let item = doc::shared(item);
                    doc::conditional_group(vec![
                        doc::concat(vec![doc::lit(" "), item.clone()]),
                        doc::concat(vec![between, item]),
                    ])
                } else {
                    doc::group(doc::concat(vec![between, item]))
                });
                prev_breaks = breaks;
            }
            doc::concat(out)
        } else {
            doc::fill(fill_parts)
        }
    } else {
        doc::concat(inner)
    };
    // The items in a group of their own, after the opening delimiter and its line break.
    let inner = if (style.hang && c.slots.len() > 1) || style.inner_id.is_some() {
        // (a magic trailing comma puts each item on a line of its own)
        let magic = style.layout == ListLayout::MagicTrailingComma
            && c.slots.last().is_some_and(|s| s.sep.is_some());
        match style.inner_id {
            Some(id) => doc::group_with_id(id, inner),
            None if magic => doc::group_broken(inner),
            None => doc::group(inner),
        }
    } else {
        inner
    };
    lead.push(inner);
    let inner = doc::concat(lead);
    let mut body_parts = Vec::new();
    if style.leading_line {
        body_parts.push(doc::line());
    }
    // (a block comment after the last item that ended its line in the source keeps the closing
    // delimiter on the next line: the next format would read the comment as one in front of the
    // delimiter, not as a trailing one to align)
    let ends_line = !flat
        && dangling.is_empty()
        && c.slots.last().is_some_and(|s| {
            [s.last_token(cx), s.sep.clone()]
                .iter()
                .flatten()
                .any(|t| comments::has_trailing_comment(g, t) && comments::break_after(g, t))
        });
    let close_pad = if ends_line {
        doc::hardline()
    } else if style.hug_close && dangling.is_empty() {
        Doc::Nil
    } else if style.align_open
        && dangling
            .last()
            .is_some_and(|c| c.kind != comments::CommentKind::Block)
    {
        // (a line comment or directive on a line of its own ends it: the closing delimiter
        // cannot follow on it)
        doc::hardline()
    } else if style.align_open {
        // (a comment pending after the last item ends the line before the closing delimiter)
        doc::line_suffix_boundary()
    } else {
        pad
    };
    // A flat list has no line breaks of its own to indent; a list nested in it indents itself.
    let inner = if flat {
        inner
    } else if style.align_open && open_line_comment && !items.is_empty() {
        // (a line comment after the opening delimiter ends its line: the items hang below it)
        doc::align(
            doc::ALIGN_FROM_LINE | style.hang_indent,
            doc::concat(vec![doc::hardline(), inner]),
        )
    } else if style.align_open {
        let inner = doc::shared(inner);
        // (every item fits up to its separator; the last one with the text after the list too)
        let hang = doc::align(
            doc::ALIGN_FROM_LINE | style.hang_indent,
            doc::concat(vec![doc::line(), inner.clone()]),
        );
        let aligned = doc::align(doc::ALIGN_TO_COLUMN, inner);
        let last_fit = match last_flat {
            // (a lone item that fits on a line of its own below the opening: a nested call
            // breaks there, not inside its arguments)
            Some(w) if items.len() == 1 => doc::fits_at_column_rest(
                w + 1,
                aligned.clone(),
                doc::fits_below(doc::FROM_LINE, w + 1 + style.hang_indent, hang.clone(), aligned),
            ),
            Some(w) => doc::fits_at_column_rest(w, aligned, hang.clone()),
            None => doc::fits_at_column(widest_last + 1, aligned, hang.clone()),
        };
        if items.len() > 1 {
            let widths: Option<Vec<usize>> = item_widths.iter().copied().collect();
            let fit = doc::fits_at_column(widest + 1, last_fit, hang.clone());
            match widths {
                Some(w) => doc::fits_fewer_lines(w, style.hang_indent, fit, hang),
                None => fit,
            }
        } else {
            last_fit
        }
    } else {
        (0..style.indent_levels.max(1)).fold(inner, |d, _| doc::indent(d))
    };
    let hugged = hug_ok.then(|| {
        // (all items on one line below the opening)
        let mut row = vec![doc::softline()];
        for (i, d) in hug_docs.iter().enumerate() {
            if i > 0 {
                row.push(sep.clone());
                row.push(doc::lit(" "));
            }
            row.push(d.clone());
        }
        let below = doc::group(doc::concat(vec![
            opening.clone(),
            doc::align(doc::ALIGN_FROM_LINE | style.hang_indent, doc::concat(row)),
            closing.clone(),
        ]));
        let mut parts = vec![opening.clone()];
        for (i, d) in hug_docs.into_iter().enumerate() {
            if i > 0 {
                parts.push(sep.clone());
                parts.push(doc::lit(" "));
                parts.push(d.clone());
            } else {
                parts.push(d.clone());
            }
            if i == last {
                parts.pop();
                parts.push(d.break_last_group());
            }
        }
        parts.push(closing.clone());
        (doc::concat(parts), below)
    });
    body_parts.extend([opening, inner, close_pad, closing]);
    let body = doc::concat(body_parts);

    let d = cx.directives_of(node);
    let broken = if flat {
        false
    } else if style.force_break
        || (style.break_from > 0
            && (c.slots.len() >= style.break_from || commented(cx, node, &c.open, &c.close)))
        || d.has(&Directive::BreakAll)
    {
        true
    } else if d.has(&Directive::KeepTogether) {
        false
    } else {
        match style.layout {
            ListLayout::Fit | ListLayout::Flat | ListLayout::Fill => false,
            ListLayout::MagicTrailingComma => c.slots.last().is_some_and(|s| s.sep.is_some()),
            ListLayout::Break => !items.is_empty(),
            ListLayout::PreserveBreak => {
                !items.is_empty() && spans_lines(cx, node, &c.open, &c.close)
            }
            ListLayout::PreserveFirstBreak => {
                !items.is_empty() && breaks_after_open(cx, node, &c.open)
            }
            ListLayout::PreserveItemBreak => !items.is_empty() && item_breaks(cx, node, &c),
        }
    };
    let list = if let Some(id) = fill_id.or(style.id) {
        let body = if broken {
            doc::concat(vec![body, doc::break_parent()])
        } else {
            body
        };
        doc::group_with_id(id, body)
    } else if broken {
        doc::group_broken(body)
    } else if style.shared {
        body
    } else {
        doc::group(body)
    };
    let list = match hugged {
        Some((h, below)) => {
            let list = doc::shared(list);
            // (items that fit on one line below the opening: no hug, the body stays on a line)
            let flats: Option<Vec<usize>> = hug_widths.iter().copied().collect();
            let total = flats.map(|w| {
                w.iter().sum::<usize>() + 2 * w.len().saturating_sub(1) + 2 + style.hang_indent
            });
            let hug = doc::conditional_group(vec![list.clone(), h, list.clone()]);
            match total {
                Some(t) => doc::fits_below(doc::FROM_LINE, t, below, hug),
                None => hug,
            }
        }
        None => list,
    };
    Ok(doc::concat(vec![before, list, after]))
}

/// Comments after the opening delimiter. A block comment sits against the delimiter and is
/// followed by a space when items follow it on the same line: `(/* a */ 1)`.
fn open_comments_doc(
    cx: &FormatCtx<'_>,
    cs: &[comments::Comment],
    items_follow: bool,
    padded: bool,
) -> Doc {
    let mut parts = Vec::new();
    for c in cs {
        if c.kind == comments::CommentKind::Block {
            if padded {
                parts.push(doc::lit(" "));
            }
            parts.push(crate::context::verbatim_text(&c.text()));
            let attached = cx.comment_style.keep_attached && !crate::context::gap_follows(c);
            if items_follow && !padded && !attached {
                parts.push(doc::if_break(Doc::Nil, doc::lit(" ")));
            }
        } else {
            parts.push(crate::context::trailing_comments_doc(
                &cx.comment_style,
                std::slice::from_ref(c),
            ));
        }
    }
    doc::concat(parts)
}

/// Does the source between the opening and the closing delimiter contain a line break? Trivia
/// outside the delimiters (the node's own leading and trailing trivia) do not count.
/// Does a comment stand between the delimiters?
fn commented(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
) -> bool {
    let range = TextRange::new(open.text_range().end(), close.text_range().start());
    node.descendants_with_tokens()
        .any(|e| cx.grammar.has_tag(e.kind(), "comment") && range.contains_range(e.text_range()))
}

fn spans_lines(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
) -> bool {
    let range = TextRange::new(open.text_range().start(), close.text_range().start());
    cx.has_line_break(node, range)
}

/// Does a line break separate two items, or stand both after the opening and before the closing
/// delimiter? (A break before the closing one after the separator of the last item counts.)
fn item_breaks(cx: &FormatCtx<'_>, node: &SyntaxNode, c: &super::Container) -> bool {
    let g = cx.grammar;
    let ends = |s: &super::Slot| s.sep.clone().or_else(|| s.last_token(cx));
    let breaks = |s: &super::Slot| ends(s).is_some_and(|t| comments::break_after(g, &t));
    c.slots.windows(2).any(|w| breaks(&w[0]))
        || (c.slots.last().is_some_and(breaks) && breaks_after_open(cx, node, &c.open))
}

/// Is there a line break between the opening delimiter and the first token after it that is not
/// trivia?
fn breaks_after_open(cx: &FormatCtx<'_>, node: &SyntaxNode, open: &SyntaxToken) -> bool {
    let g = cx.grammar;
    let mut next = open.next_token();
    while let Some(t) = &next {
        if !g.is_trivia(t.kind()) {
            break;
        }
        next = t.next_token();
    }
    let end = next.map_or(node.text_range().end(), |t| t.text_range().start());
    cx.has_line_break(node, TextRange::new(open.text_range().end(), end))
}

/// A block comment glued to the item after its separator (`f(a, /*b=*/c)`) moves in front of
/// that item, so that a line break falls before the comment, not between it and the item.
fn attach_comments(cx: &FormatCtx<'_>, items: &mut [super::Rendered]) {
    if !cx.comment_style.keep_attached {
        return;
    }
    let g = cx.grammar;
    for i in 1..items.len() {
        let [before, after] = &mut items[i - 1..=i] else {
            continue;
        };
        let Some(sep) = before.sep.as_ref() else {
            continue;
        };
        let own = comments::trailing_comments(g, sep);
        let [c] = own.as_slice() else {
            continue;
        };
        if c.kind != comments::CommentKind::Block
            || c.breaks_after > 0
            || crate::context::gap_follows(c)
            || !comments::leading_comments(g, sep).is_empty()
            || after.hole
            || after.verbatim
        {
            continue;
        }
        before.sep_comment_moved = true;
        let text = crate::context::verbatim_text(&c.text());
        let item = std::mem::replace(&mut after.doc, Doc::Nil);
        after.doc = doc::concat(vec![text, item]);
    }
}

/// The width of a chain of operators (or a conditional) up to its first operator, through the
/// leftmost nested chain: what must fit on the line the chain starts on.
fn chain_head(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<usize> {
    let g = cx.grammar;
    let is_chain =
        |n: &SyntaxNode| g.has_tag(n.kind(), "binary-chain") || g.has_tag(n.kind(), "conditional");
    if !is_chain(node) {
        return None;
    }
    let mut n = node.clone();
    while let Some(first) = n.first_child().filter(|c| is_chain(c)) {
        n = first;
    }
    let op = n
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| g.has_tag(t.kind(), "operator") || t.text() == "?")?;
    head_width(node, op.text_range().end())
}

/// The width of an item (a lambda) up to the opening brace of its first block: what must fit on
/// the line the item starts on.
fn block_head(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<usize> {
    let g = cx.grammar;
    let block = node.descendants().find(|n| g.has_tag(n.kind(), "block"))?;
    head_width(node, block.text_range().start() + TextSize::from(1))
}

/// The width of the source text of `node` up to `end`, runs of blanks as one.
fn head_width(node: &SyntaxNode, end: TextSize) -> Option<usize> {
    let text = node.text().to_string();
    let end = usize::from(end - node.text_range().start());
    let mut width = 0;
    let mut blank = false;
    for ch in text.get(..end)?.chars() {
        if ch.is_whitespace() {
            if !blank {
                width += 1;
            }
            blank = true;
        } else {
            width += 1;
            blank = false;
        }
    }
    Some(width)
}
