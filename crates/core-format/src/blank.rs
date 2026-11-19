//! `[blank]` rows: between which neighbouring statements or members a blank line is required
//! although the source has none. A row is `PREV / NEXT [: key=value ...]`; `PREV` and `NEXT` are
//! `*`, `kind=A|B` or `kind!=A|B`, and a row needs all its keys to hold (any row may decide).
//! Keys test the next node, or the previous one with the prefix `prev.`: `parent=A|B` (the next
//! node's parent), `same_kind=yes|no` (both of one kind), `has=A|B` and `lacks=A|B` (a child of
//! those kinds), `commented=yes|no` (a comment in front), `doc=yes|no` (a `/**` comment in
//! front), `comment_blank=yes|no` (a blank line between the comments in front or after them),
//! `annotated=any|args` (an annotation, or an annotation with arguments, among its modifiers) and `group=K`
//! (only when the second token of the two is of kind `K` for one of them).
//!
//! A language with the options `blank_lines_between_members` (`preserve`, `separate`, `always`),
//! `blank_lines_after_block` and `blank_line_before_comment` (`preserve`, `require`) gets the
//! decision from [`by_options`] instead of rows. It reads the tags `#type-declaration`, `#namespace`, `#function` (with a
//! `body` or `expression_body` slot), `#enum-constant`, `#import`,
//! `#annotation` and `#directive`, and the options `blank_compact_kinds` (kinds
//! that need no blank line between two of them), `blank_after_block_kinds`, `blank_directives_skip` and
//! `blank_directives_attach`: prefixes of the directives in front of the next node that leave the
//! blank line to the source, and of those that stay with the node before.
//!
//! concept: blank-lines-between-members
//! what: Rows say between which neighbouring statements or members a blank line is required.
//! tools: ktlint blank-line-before-declaration, google-java-format member blank lines, Prettier blank-line preservation, dotnet-format blank line options
//! reads: rules:blank, tag:directive, option:blank_lines_between_members

use crate::FormatCtx;
use crate::comments;
use crate::seq::{first, kids};
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode};

struct KindPat {
    kinds: Vec<SyntaxKind>,
    not: bool,
}

fn kinds_of(g: &Grammar, names: &str) -> Vec<SyntaxKind> {
    names
        .split('|')
        .filter(|n| !n.is_empty())
        .map(|n| {
            g.kind_by_name(n)
                .unwrap_or_else(|| panic!("blank: kind `{n}`"))
        })
        .collect()
}

impl KindPat {
    fn parse(g: &Grammar, text: &str) -> KindPat {
        let text = text.trim();
        let (not, names) = match text.strip_prefix("kind!=") {
            Some(n) => (true, n),
            None => (false, text.strip_prefix("kind=").unwrap_or("")),
        };
        KindPat {
            kinds: kinds_of(g, names),
            not,
        }
    }

    fn matches(&self, kind: SyntaxKind) -> bool {
        self.kinds.is_empty() || self.kinds.contains(&kind) != self.not
    }
}

enum Test {
    Parent(Vec<SyntaxKind>),
    SameKind,
    Has(Vec<SyntaxKind>),
    Commented,
    Doc,
    CommentBlank,
    /// An annotation among the modifiers; `true`: one with arguments.
    Annotated(bool),
    /// The second token of the node is of this kind.
    Second(SyntaxKind),
}

struct Cond {
    prev: bool,
    test: Test,
    want: bool,
}

struct Row {
    prev: KindPat,
    next: KindPat,
    conds: Vec<Cond>,
}

pub(crate) struct Blank(Vec<Row>);

impl Blank {
    pub(crate) fn parse<'a>(g: &Grammar, lines: impl Iterator<Item = &'a str>) -> Blank {
        Blank(
            lines
                .map(|l| {
                    let (pair, opts) = l.split_once(':').unwrap_or((l, ""));
                    let (prev, next) = pair
                        .split_once('/')
                        .unwrap_or_else(|| panic!("blank: `{l}`"));
                    let conds = opts
                        .split_whitespace()
                        .map(|w| {
                            let (key, value) = w.split_once('=').unwrap_or((w, ""));
                            let (prev, key) = match key.strip_prefix("prev.") {
                                Some(k) => (true, k),
                                None => (false, key),
                            };
                            let (test, want) = match key {
                                "parent" => (Test::Parent(kinds_of(g, value)), true),
                                "has" => (Test::Has(kinds_of(g, value)), true),
                                "lacks" => (Test::Has(kinds_of(g, value)), false),
                                "group" => (Test::Second(kinds_of(g, value)[0]), true),
                                "annotated" => (Test::Annotated(value == "args"), true),
                                _ => {
                                    let test = match key {
                                        "same_kind" => Test::SameKind,
                                        "commented" => Test::Commented,
                                        "doc" => Test::Doc,
                                        "comment_blank" => Test::CommentBlank,
                                        "annotation_args" => Test::Annotated(true),
                                        _ => panic!("blank: key `{key}`"),
                                    };
                                    (test, value == "yes")
                                }
                            };
                            Cond { prev, test, want }
                        })
                        .collect();
                    Row {
                        prev: KindPat::parse(g, prev),
                        next: KindPat::parse(g, next),
                        conds,
                    }
                })
                .collect(),
        )
    }

    pub(crate) fn between(&self, cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
        self.0.iter().any(|r| {
            r.prev.matches(prev.kind())
                && r.next.matches(next.kind())
                && r.conds.iter().all(|c| {
                    let node = if c.prev { prev } else { next };
                    match &c.test {
                        // A `group` row holds when exactly one of the two is of the kind.
                        Test::Second(k) => second_is(cx, prev, *k) != second_is(cx, next, *k),
                        t => holds(cx, t, prev, next, node) == c.want,
                    }
                })
        })
    }
}

fn second_is(cx: &FormatCtx<'_>, node: &SyntaxNode, kind: SyntaxKind) -> bool {
    kids(cx, node)
        .get(1)
        .and_then(|e| first(cx, e))
        .is_some_and(|t| t.kind() == kind)
}

fn leading(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<comments::Comment> {
    cx.first_token(node)
        .map(|t| comments::leading_comments(cx.grammar, &t))
        .unwrap_or_default()
}

fn holds(
    cx: &FormatCtx<'_>,
    test: &Test,
    prev: &SyntaxNode,
    next: &SyntaxNode,
    node: &SyntaxNode,
) -> bool {
    match test {
        Test::Parent(kinds) => next.parent().is_some_and(|p| kinds.contains(&p.kind())),
        Test::SameKind => prev.kind() == next.kind(),
        Test::Has(kinds) => node.children().any(|c| kinds.contains(&c.kind())),
        Test::Commented => !leading(cx, node).is_empty(),
        Test::Doc => crate::shapes::has_doc_comment(cx, node),
        Test::CommentBlank => leading(cx, node).iter().any(|c| c.breaks_after >= 2),
        Test::Annotated(args) => crate::shapes::annotated(cx, node, *args),
        Test::Second(_) => false,
    }
}

/// The blank line rule of the sequences of a body when the options ask for one: it asks the
/// language's [`NodeFormatter::blank_between`](crate::NodeFormatter::blank_between).
pub fn rule(cx: &FormatCtx<'_>) -> Option<crate::shapes::BlankRule> {
    let o = cx.options;
    let enabled = matches!(o.text("blank_lines_between_members"), "separate" | "always")
        || o.is("blank_lines_after_block", "require")
        || o.is("blank_line_before_comment", "require");
    enabled.then_some(|cx, p, n| cx.blank_between(p, n))
}

/// Does a blank line have to stand between two neighbours although the source has none: after a
/// closing brace, before a `//` comment, between members (`separate`: when either has a body, an
/// annotation or a comment in front; `always`: any two but two of one `blank_compact_kinds` kind)? A
/// directive in front of `next` that `blank_directives_skip` names leaves the blank line to the
/// source, and one that `blank_directives_attach` names and that follows `prev` without a blank
/// line and has one after it belongs to `prev`.
pub fn by_options(cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
    let g = cx.grammar;
    let o = cx.options;
    let comments = leading(cx, next);
    let starts = |c: &comments::Comment, list: &[String]| {
        let text = c.text();
        list.iter()
            .any(|d| text.trim_start().starts_with(d.as_str()))
    };
    if comments
        .last()
        .is_some_and(|c| starts(c, &o.strings("blank_directives_attach")) && c.breaks_after >= 2)
        && cx
            .first_token(next)
            .is_some_and(|t| comments::blank_lines_before(g, &t) == 0)
    {
        return false;
    }
    let skip = o.strings("blank_directives_skip");
    if comments.iter().any(|c| starts(c, &skip)) {
        return false;
    }
    let always = o.is("blank_lines_between_members", "always");
    let tagged = |n: &SyntaxNode, t: &str| g.has_tag(n.kind(), t);
    let kind_in = |n: &SyntaxNode, key: &str| o.strings(key).iter().any(|k| g.name(n.kind()) == k);
    if cx.last_token(prev).is_some_and(|t| t.text() == "}")
        && (o.is("blank_lines_after_block", "require")
            || (always && kind_in(next, "blank_after_block_kinds")))
    {
        return true;
    }
    if o.is("blank_line_before_comment", "require")
        && comments
            .first()
            .is_some_and(|c| c.kind == comments::CommentKind::Line && !c.text().starts_with("////"))
    {
        return true;
    }
    let enum_member = tagged(next, "enum-constant");
    let in_members = next.parent().is_some_and(|p| {
        tagged(&p, "namespace") || tagged(&p, "type-declaration") || p.parent().is_none()
    });
    if !matches!(o.text("blank_lines_between_members"), "separate" | "always")
        || !(enum_member || in_members)
    {
        return false;
    }
    // Imports in a row stay in a row, comments above one of them or not.
    let imports = tagged(prev, "import") && tagged(next, "import");
    let is_directive = |c: &comments::Comment| matches!(&c.element, SyntaxElement::Node(n) if g.has_tag(n.kind(), "directive"));
    let decorated = comments.iter().any(|c| !is_directive(c))
        || next.children().any(|c| tagged(&c, "annotation"));
    let has_body = |n: &SyntaxNode| {
        tagged(n, "type-declaration")
            || tagged(n, "namespace")
            || (tagged(n, "function")
                && (g.slot(n, "body").is_some() || g.slot(n, "expression_body").is_some()))
    };
    (decorated && !imports)
        || (!enum_member
            && if always {
                !(kind_in(prev, "blank_compact_kinds") && prev.kind() == next.kind())
            } else {
                has_body(prev) || has_body(next)
            })
}
