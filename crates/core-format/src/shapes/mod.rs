//! Shared shapes: the handful of layouts most syntax is made of. Each shape reads the node's
//! structure generically (delimiter tokens at both ends, child nodes in between, tokens tagged
//! `#separator`), merges its style with the node's layout directives, and returns a [`Doc`].
//!
//! concept: delimited-container
//! what: Containers of delimiters, children and slots, and verbatim text.
//! tools: none (internal)
//! reads: grammar:language

mod assignment;
mod binary_chain;
mod block;
mod call_arguments;
mod call_chain;
mod chain;
mod compound;
mod conditional;
pub mod continuation;
mod declaration;
pub mod fill;
pub mod flow;
mod heritage;
mod key_value;
mod list;
mod maybe_parenthesize;
mod parens;
mod section;
mod signature;

pub use assignment::{
    Assign, AssignHooks, AssignLayout, AssignPolicy, AssignStyle, NoAssignHooks, assignment,
};
pub use binary_chain::{
    BinaryHooks, BinaryPolicy, Hug, NoBinaryHooks, OperatorLine, Place, binary_chain, flattens,
};
pub use block::{
    BlockStyle, annotation_line, blank_before, block, block_between, block_open_inside, braces,
    document, spread_cases_style, statement_sequence, statement_sequence_with,
};
pub use call_arguments::{
    Args, CallStyle, Expanded, HugLast, Item, Plan, call_arguments, can_hug, expression_of,
    hugs_last, is_long_curried, plan,
};
pub use call_chain::{
    CHAIN_LABEL, ChainHooks, ChainPolicy, NoHooks, is_flat as flat_chain,
    member_chain as call_chain,
};
pub use chain::{MemberChain, ends_in_line_comment, gap_after_operator, member_chain};
pub use compound::{body as compound_body, continuation as compound_continuation};
pub use conditional::{
    Around, ConditionalHooks, ConditionalPolicy, NoConditionalHooks, conditional,
};
pub use declaration::{
    Annotations, Clause, HeaderStyle, Printer, annotation_args, annotations_above, enum_body,
    has_arguments, has_doc_comment, header, lead_annotations, members, modifiers, type_declaration,
};
pub use heritage::{HeritageLayout, heritage};
pub use key_value::{KeyValueStyle, key_value};
pub use list::{
    ListFacts, ListLayout, ListStyle, StyleOf, TrailingComma, delimited_list,
    delimited_list_around, delimited_list_between,
};
pub use maybe_parenthesize::{between, enclosed, head_test, keyword_argument, maybe_parenthesize};
pub use parens::{
    Need, NoParenHooks, ParenHooks, ParenLayout, ParenPolicy, Unneeded, binary_needs, first_char,
    is_dropped_close, paren_group, parenthesized, unwrap, wrap,
};
pub use section::section;
pub use signature::{Signature, parameter_list, signature};

use crate::FormatCtx;
use crate::comments::{self, Comment, CommentKind};
use crate::directives::Directive;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::ignore::{self, IgnoreMark};
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// Source text of a node exactly as written (plus its outer comments).
pub fn verbatim(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    cx.verbatim_node(node)
}

/// Decides whether a blank line must stand between two items (the previous item's last node and
/// the next item's first node) although the source has none.
pub type BlankRule = fn(&FormatCtx<'_>, &SyntaxNode, &SyntaxNode) -> bool;

/// A child node and the separator token that follows it, if any.
pub(crate) struct Slot {
    node: SyntaxNode,
    sep: Option<SyntaxToken>,
    /// An empty item that is only its separator (`[a, , b]`); `node` is then the node of the
    /// separator.
    hole: bool,
    /// What makes the item a bare token or a run; most items have none.
    ext: Option<Box<Ext>>,
}

#[derive(Default)]
struct Ext {
    /// An item that starts with a bare token (Python's `*` and `/` among parameters, the `**` of
    /// a dict); `node` is then its parent.
    bare: Option<SyntaxToken>,
    /// The elements after the first one of an item that is a run (`key: value`: a token tagged
    /// `#key-separator` and a node after it; `**x`: a token tagged `#rest` and the node after it).
    run: Vec<SyntaxElement>,
    /// The run waits for the node after its last token.
    open: bool,
}

impl Slot {
    fn new(node: SyntaxNode) -> Slot {
        Slot {
            node,
            sep: None,
            hole: false,
            ext: None,
        }
    }

    fn bare(&self) -> Option<&SyntaxToken> {
        self.ext.as_ref().and_then(|e| e.bare.as_ref())
    }

    fn run(&self) -> &[SyntaxElement] {
        self.ext.as_ref().map_or(&[], |e| &e.run)
    }

    /// The last node of a run.
    fn last_node(&self) -> SyntaxNode {
        self.run()
            .iter()
            .rev()
            .find_map(|e| e.as_node().cloned())
            .unwrap_or_else(|| self.node.clone())
    }

    /// The last token of the item.
    fn last_token(&self, cx: &FormatCtx<'_>) -> Option<SyntaxToken> {
        match self.run().last() {
            Some(SyntaxElement::Token(t)) => Some(t.clone()),
            Some(SyntaxElement::Node(n)) => cx.last_token(n),
            None => self.bare().cloned().or_else(|| cx.last_token(&self.node)),
        }
    }
}

/// A node with open and close delimiter tokens and items between them.
pub(crate) struct Container {
    open: SyntaxToken,
    close: SyntaxToken,
    /// A node tagged `#list-prefix` before the first item (a C# attribute target: `[assembly: A]`).
    prefix: Option<SyntaxNode>,
    slots: Vec<Slot>,
}

impl Container {
    /// What the source says about the list, for the choice of its style.
    pub(crate) fn facts(&self) -> ListFacts<'_> {
        ListFacts {
            items: self.slots.len(),
            last_sep: self.slots.last().and_then(|s| s.sep.as_ref()),
            only: match self.slots.as_slice() {
                [s] if s.ext.is_none() && !s.hole => Some(&s.node),
                _ => None,
            },
        }
    }
}

/// The container of a node whose first and last significant elements are its delimiters.
pub(crate) fn container(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Container, FormatError> {
    container_between(cx, node, None, &[])
}

/// The container whose items are the children of `items` and whose delimiters `open` and
/// `close` stand outside it (the parentheses of a tuple that has none of its own).
pub(crate) fn container_around(
    cx: &FormatCtx<'_>,
    items: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
) -> Result<Container, FormatError> {
    let g = cx.grammar;
    let mut slots: Vec<Slot> = Vec::new();
    for e in items
        .children_with_tokens()
        .filter(|e| !g.is_trivia(e.kind()))
    {
        place(g, &[], &mut slots, e)
            .map_err(|m| FormatError::Shape(format!("{}: {m}", g.name(items.kind()))))?;
    }
    Ok(Container {
        open: open.clone(),
        close: close.clone(),
        prefix: None,
        slots,
    })
}

/// The container formed by the elements of `node` from the delimiter token `open` to the
/// delimiter token `close` (the node's first and last when `None`); other children of the node
/// are the caller's business.
pub(crate) fn container_between(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    bounds: Option<(&SyntaxToken, &SyntaxToken)>,
    unwrap: &[SyntaxKind],
) -> Result<Container, FormatError> {
    let g = cx.grammar;
    let name = g.name(node.kind());
    let shape = |m: &str| FormatError::Shape(format!("{name}: {m}"));
    let mut slots: Vec<Slot> = Vec::new();
    let mut prefix = None;
    let mut add = |e: SyntaxElement| -> Result<(), FormatError> {
        if slots.is_empty()
            && prefix.is_none()
            && let SyntaxElement::Node(n) = &e
            && g.has_tag(n.kind(), "list-prefix")
        {
            prefix = Some(n.clone());
            return Ok(());
        }
        place(g, unwrap, &mut slots, e).map_err(|m| shape(&m))
    };
    let mut children = node.children_with_tokens().by_kind(|k| !g.is_trivia(k));
    let (open, close) = match bounds {
        // The first and the last element are the delimiters: one pass, no list of the children.
        None => {
            let missing = || shape("expected an opening and a closing token");
            let Some(SyntaxElement::Token(open)) = children.next() else {
                return Err(missing());
            };
            let mut held = children.next().ok_or_else(missing)?;
            for next in children {
                add(std::mem::replace(&mut held, next))?;
            }
            let SyntaxElement::Token(close) = held else {
                return Err(missing());
            };
            (open, close)
        }
        Some((open, close)) => {
            let missing = || shape("expected an opening and a closing token");
            let mut inside = false;
            let mut found = false;
            for e in children {
                match &e {
                    SyntaxElement::Token(t) if t == close && inside => {
                        found = true;
                        break;
                    }
                    SyntaxElement::Token(t) if t == open => inside = true,
                    _ if inside => add(e)?,
                    _ => {}
                }
            }
            if !found {
                return Err(missing());
            }
            (open.clone(), close.clone())
        }
    };
    Ok(Container {
        open,
        close,
        prefix,
        slots,
    })
}

/// Adds a child between the delimiters: a node starts a slot, a separator ends the last one.
fn place(
    g: &Grammar,
    unwrap: &[SyntaxKind],
    slots: &mut Vec<Slot>,
    e: SyntaxElement,
) -> Result<(), String> {
    match e {
        // (a zero-width node, which some parsers leave in a list, holds nothing to print)
        SyntaxElement::Node(node) if node.text_range().is_empty() => {}
        // A wrapper of the items (`ListStyle::unwrap`: Protobuf's field options) holds items of the list.
        SyntaxElement::Node(node) if unwrap.contains(&node.kind()) => {
            for c in node
                .children_with_tokens()
                .filter(|c| !g.is_trivia(c.kind()))
            {
                place(g, unwrap, slots, c)?;
            }
        }
        // A hole (`#hole`: JS `[a, , b]`) is an item that is only its own separator.
        SyntaxElement::Node(node) if g.has_tag(node.kind(), "hole") => {
            let sep = node
                .children_with_tokens()
                .filter_map(SyntaxElement::into_token)
                .find(|t| !g.is_trivia(t.kind()));
            slots.push(Slot {
                hole: true,
                sep,
                ..Slot::new(node)
            })
        }
        SyntaxElement::Node(node) => {
            match slots
                .last_mut()
                .and_then(|s| s.ext.as_deref_mut())
                .filter(|e| e.open)
            {
                Some(ext) => {
                    ext.run.push(SyntaxElement::Node(node));
                    ext.open = false;
                }
                None => slots.push(Slot::new(node)),
            }
        }
        SyntaxElement::Token(t) if g.has_tag(t.kind(), "separator") => {
            match slots.last_mut().filter(|s| s.sep.is_none()) {
                Some(slot) => slot.sep = Some(t),
                // (a `#hole` separator with no item before it is an empty item)
                None if g.has_tag(t.kind(), "hole") => {
                    let node = t.parent().ok_or("separator without a parent")?;
                    slots.push(Slot {
                        sep: Some(t),
                        hole: true,
                        ..Slot::new(node)
                    });
                }
                None => return Err("separator without a preceding item".into()),
            }
        }
        // `;` after a statement: not an item, and not kept.
        SyntaxElement::Token(t) if g.has_tag(t.kind(), "stray") => {}
        // The separator of a key and its value continues the item.
        SyntaxElement::Token(t) if g.has_tag(t.kind(), "key-separator") => {
            match slots
                .last_mut()
                .filter(|s| s.sep.is_none() && !s.ext.as_ref().is_some_and(|e| e.open))
            {
                Some(slot) => {
                    let ext = slot.ext.get_or_insert_with(Default::default);
                    ext.run.push(SyntaxElement::Token(t));
                    ext.open = true;
                }
                None => return Err("key separator without a preceding key".into()),
            }
        }
        // Any other token is an item of its own; a `#rest` one (`**`) starts a run.
        SyntaxElement::Token(t) => slots.push(Slot {
            ext: Some(Box::new(Ext {
                open: g.has_tag(t.kind(), "rest"),
                bare: Some(t.clone()),
                run: Vec::new(),
            })),
            ..Slot::new(t.parent().ok_or("token without a parent")?)
        }),
    }
    Ok(())
}

// A formatted item, or a run of items kept as written by an ignore region.
pub(crate) struct Rendered {
    /// First node of the item (directives are read from it).
    first: SyntaxNode,
    /// Last node of the item.
    last: SyntaxNode,
    /// Blank lines the source has before the item (see [`comments::blank_lines_before`]).
    blank_before: usize,
    doc: Doc,
    /// Kept as written by an ignore comment: it ends with its own separator when the source had one.
    verbatim: bool,
    /// Separator after the last item of the run.
    sep: Option<SyntaxToken>,
    /// An empty item (see [`Slot::hole`]).
    hole: bool,
    /// The last token of an item that is a bare token or a run (see [`Slot`]).
    tail: Option<SyntaxToken>,
}

impl Rendered {
    /// The last token of the item.
    fn last_token(&self, cx: &FormatCtx<'_>) -> Option<SyntaxToken> {
        match &self.tail {
            Some(t) => Some(t.clone()),
            None => cx.last_token(&self.last),
        }
    }
}

/// The document of an item that is a bare token or a run, a blank after the separator of a key.
fn run_doc(cx: &FormatCtx<'_>, slot: &Slot, commented: bool) -> Result<Doc, FormatError> {
    let mut parts = vec![match slot.bare() {
        Some(t) => cx.token(t),
        None => cx.format_known(&slot.node, commented)?,
    }];
    for e in slot.run() {
        match e {
            SyntaxElement::Node(n) => parts.push(cx.format_known(n, false)?),
            SyntaxElement::Token(t) => {
                parts.push(cx.token(t));
                if cx.grammar.has_tag(t.kind(), "key-separator") {
                    parts.push(doc::lit(" "));
                }
            }
        }
    }
    Ok(doc::concat(parts))
}

fn no_tokens() -> FormatError {
    FormatError::Shape("item without tokens".into())
}

/// The blank lines before a slot and its ignore mark. Only a comment can carry a mark, so the
/// first token is looked at only when there is one.
fn leading_info(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
) -> Result<(usize, Option<IgnoreMark>, bool), FormatError> {
    let g = cx.grammar;
    let lead = comments::node_leading(g, node);
    if !lead.has_tokens {
        return Err(no_tokens());
    }
    if !lead.has_comment {
        return Ok((lead.blank_lines, None, false));
    }
    let tok = cx.first_token(node).ok_or_else(no_tokens)?;
    Ok((
        comments::blank_lines_before(g, &tok),
        ignore::mark_in(g, &comments::leading_comments(g, &tok)),
        true,
    ))
}

/// Formats each slot; slots between `sudsy-ignore-start` and `sudsy-ignore-end` comments are
/// merged into one verbatim item.
pub(crate) fn render_slots(
    cx: &FormatCtx<'_>,
    slots: &[Slot],
    marks: bool,
) -> Result<Vec<Rendered>, FormatError> {
    let mut out = Vec::with_capacity(slots.len());
    let mut i = 0;
    while i < slots.len() {
        if slots[i].hole {
            out.push(Rendered {
                first: slots[i].node.clone(),
                last: slots[i].node.clone(),
                blank_before: 0,
                // (the comments in front of its separator stand in front of the hole)
                doc: slots[i]
                    .sep
                    .as_ref()
                    .map_or(Doc::Nil, |t| cx.leading_comments_doc(t)),
                verbatim: false,
                sep: slots[i].sep.clone(),
                hole: true,
                tail: None,
            });
            i += 1;
            continue;
        }
        if let Some(t) = slots[i].bare() {
            out.push(Rendered {
                first: slots[i].node.clone(),
                last: slots[i].last_node(),
                blank_before: comments::blank_lines_before(cx.grammar, t),
                doc: run_doc(cx, &slots[i], false)?,
                verbatim: false,
                sep: slots[i].sep.clone(),
                hole: false,
                tail: slots[i].last_token(cx),
            });
            i += 1;
            continue;
        }
        let (blank_before, mark, commented) = leading_info(cx, &slots[i].node)?;
        let mark = mark.filter(|_| marks);
        if mark == Some(IgnoreMark::Start) {
            let j = ignore::region_end(slots.len(), i, |n| {
                Ok::<_, FormatError>(leading_info(cx, &slots[n].node)?.1)
            })? - 1;
            let tok = cx.first_token(&slots[i].node).ok_or_else(no_tokens)?;
            let last = slots[j].last_token(cx).ok_or_else(no_tokens)?;
            out.push(Rendered {
                first: slots[i].node.clone(),
                last: slots[j].node.clone(),
                blank_before,
                doc: cx.verbatim_range(&tok, &last),
                verbatim: true,
                sep: slots[j].sep.clone(),
                hole: false,
                tail: Some(last),
            });
            i = j + 1;
        } else {
            let run = !slots[i].run().is_empty();
            out.push(Rendered {
                first: slots[i].node.clone(),
                last: slots[i].last_node(),
                blank_before,
                doc: if run {
                    run_doc(cx, &slots[i], commented)?
                } else {
                    cx.format_known(&slots[i].node, commented)?
                },
                verbatim: mark == Some(IgnoreMark::Ignore),
                sep: slots[i].sep.clone(),
                hole: false,
                tail: run.then(|| slots[i].last_token(cx)).flatten(),
            });
            i += 1;
        }
    }
    Ok(out)
}

/// The break between two items: `base` (a `line` or `hardline`), turned into a hard break by
/// `BreakBefore`/`BreakAfter`, followed by the blank lines the source had (at most
/// `max_blank`) or that a `BlankLineBefore` directive requires.
pub(crate) fn gap(
    cx: &FormatCtx<'_>,
    prev: &Rendered,
    next: &Rendered,
    base: Doc,
    max_blank: usize,
    force_blank: Option<BlankRule>,
    blank_hard: bool,
) -> Doc {
    let has_directives = !cx.directives.is_empty() && !prev.hole && !next.hole;
    let (prev_d, next_d) = (
        has_directives.then(|| cx.directives_of(&prev.first)),
        has_directives.then(|| cx.directives_of(&next.first)),
    );
    let required = next_d.and_then(|d| d.blank_lines_before()).or_else(|| {
        force_blank
            .is_some_and(|rule| rule(cx, &prev.last, &next.first))
            .then_some(1)
    });
    let forced = next_d.is_some_and(|d| d.has(&Directive::BreakBefore))
        || prev_d.is_some_and(|d| d.has(&Directive::BreakAfter))
        || required.is_some_and(|n| n > 0);
    let hard_base = matches!(base, Doc::HardLine);
    let blank = required.unwrap_or_else(|| next.blank_before.min(max_blank));
    // Blank lines the source had vanish when a list fits on one line; required ones do not.
    let blank_line = if hard_base || blank_hard || required.is_some() {
        doc::hardline()
    } else {
        doc::softline()
    };
    let first = if forced || (blank_hard && blank > 0) {
        doc::hardline()
    } else {
        base
    };
    if blank == 0 {
        return first;
    }
    let mut parts = vec![first];
    parts.extend((0..blank).map(|_| blank_line.clone()));
    doc::concat(parts)
}

/// Comments owned by the separator token after `item`, printed like trailing comments of the
/// item. Once a line comment is on the line, anything further must go on its own line, or it
/// would become part of that comment.
pub(crate) fn separator_comments(cx: &FormatCtx<'_>, item: &Rendered) -> Doc {
    let Some(sep) = item.sep.as_ref() else {
        return Doc::Nil;
    };
    let g = cx.grammar;
    let leading = if item.hole {
        Vec::new()
    } else {
        comments::leading_comments(g, sep)
    };
    let own: Vec<Comment> = leading
        .into_iter()
        .chain(comments::trailing_comments(g, sep))
        .collect();
    if own.is_empty() {
        return Doc::Nil;
    }
    let mut line_comment_open = item.last_token(cx).is_some_and(|last| {
        comments::trailing_comments(g, &last)
            .iter()
            .any(|c| c.kind == CommentKind::Line)
    });
    let mut parts = Vec::new();
    for c in own {
        if line_comment_open {
            parts.push(doc::hardline());
            parts.push(crate::context::verbatim_text(&c.text()));
        } else {
            parts.push(crate::context::trailing_comments_doc(
                &cx.comment_style,
                std::slice::from_ref(&c),
            ));
            line_comment_open = c.kind == CommentKind::Line;
        }
    }
    doc::concat(parts)
}

/// Comments in front of a closing delimiter, each on its own line inside the container. Blank
/// lines between them are kept, and one before the first when `blank_before`.
pub fn dangling_comments(cs: &[Comment], blank_before: bool) -> Doc {
    let mut parts = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        parts.push(doc::hardline());
        if (i == 0 && blank_before) || (i > 0 && cs[i - 1].breaks_after >= 2) {
            parts.push(doc::hardline());
        }
        parts.push(crate::context::comment_doc(c));
    }
    doc::concat(parts)
}
