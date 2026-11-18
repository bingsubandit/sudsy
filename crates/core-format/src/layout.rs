//! Node layouts as data: the `[layout]` rows of a language's rules file name a shape for a kind
//! of node and the details of that shape, so the language holds only a table (see
//! [`crate::rules`]). A row is `KINDS[&parent=KINDS|&parent!=KINDS] : shape key=value ...`.
//!
//! Shapes: `verbatim`; `document` (the file; blank lines between statements from `[blank]`);
//! `scope body=K` (`namespace A;` and the statements after it); `braced open=K` (a declaration
//! with its members in braces); `with_body body=K params=K` (a function: signature, then the
//! body); `block` (statements between braces); `case colon=K` (a head up to the colon, then
//! statements); `colon_body colon=K clauses=K` (the alternative syntax: `:` ... `end`); `list`
//! (a parenthesized list, one item per line when the source breaks it); `array` (a list kept as
//! written, with a trailing comma when its closing bracket is on a line of its own);
//! `braced_list open=K` (`match (x) { ... }`); `heritage colon=K list=K ctor=K params=K
//! min=option:N hug_min=option:N` (supertypes one per line); `chain spaced=K` (a member chain);
//! `cases open=K` (a `when` body, a blank between cases when one spans lines).
//!
//! Keys: `empty=expand` (an empty block is two lines, else `{}`), `gap=option:NAME|space` (what
//! stands before a brace: a line break, or a blank when the option is `same_line`),
//! `trailing=option:NAME=VALUE` (a trailing comma when the option has the value),
//! `no_trailing_after=K` (none after a last item that starts with that token).

use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::shapes::flow::{Flow, break_after, el, first, kids, last};
use crate::shapes::{self, BlockStyle, ListLayout, ListStyle, TrailingComma};
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode};

type R = Result<Doc, FormatError>;

/// Keys whose values are kind names.
const KIND_KEYS: &[&str] = &[
    "open",
    "body",
    "params",
    "colon",
    "clauses",
    "no_trailing_after",
    "list",
    "ctor",
    "spaced",
];

pub(crate) struct Row {
    kinds: Vec<SyntaxKind>,
    parent: Vec<SyntaxKind>,
    parent_not: bool,
    shape: String,
    text: Vec<(String, String)>,
    kind_args: Vec<(String, Vec<SyntaxKind>)>,
}

impl Row {
    fn parse(g: &Grammar, line: &str) -> Row {
        let (subject, shape) = line
            .split_once(" : ")
            .unwrap_or_else(|| panic!("layout: `{line}`"));
        let kinds_of = |names: &str| -> Vec<SyntaxKind> {
            names
                .split('|')
                .map(|n| {
                    g.kind_by_name(n.trim())
                        .unwrap_or_else(|| panic!("layout: kind `{n}`"))
                })
                .collect()
        };
        let mut parts = subject.split('&');
        let kinds = kinds_of(parts.next().unwrap_or(""));
        let (mut parent, mut parent_not) = (Vec::new(), false);
        for p in parts {
            if let Some(v) = p.strip_prefix("parent!=") {
                (parent, parent_not) = (kinds_of(v), true);
            } else if let Some(v) = p.strip_prefix("parent=") {
                parent = kinds_of(v);
            }
        }
        let mut words = shape.split_whitespace();
        let shape = words.next().unwrap_or("").to_string();
        let (mut text, mut kind_args) = (Vec::new(), Vec::new());
        for w in words {
            let (k, v) = w.split_once('=').unwrap_or((w, ""));
            if KIND_KEYS.contains(&k) {
                kind_args.push((k.to_string(), kinds_of(v)));
            } else {
                text.push((k.to_string(), v.to_string()));
            }
        }
        Row {
            kinds,
            parent,
            parent_not,
            shape,
            text,
            kind_args,
        }
    }

    fn matches(&self, node: &SyntaxNode) -> bool {
        self.kinds.contains(&node.kind())
            && (self.parent.is_empty()
                || node
                    .parent()
                    .is_some_and(|p| self.parent.contains(&p.kind()))
                    != self.parent_not)
    }

    fn arg(&self, key: &str) -> Option<&str> {
        self.text
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn kinds_arg(&self, key: &str) -> &[SyntaxKind] {
        self.kind_args
            .iter()
            .find(|(k, _)| k == key)
            .map_or(&[], |(_, v)| v.as_slice())
    }

    /// `option:NAME=VALUE`: does the option have the value?
    fn option_is(&self, cx: &FormatCtx<'_>, key: &str) -> bool {
        self.arg(key)
            .and_then(|v| v.strip_prefix("option:"))
            .and_then(|v| v.split_once('='))
            .is_some_and(|(name, value)| cx.options.str(name) == value)
    }
}

/// File-level facts: when the whole file stays as written.
pub(crate) struct File {
    /// A file of one line.
    pub(crate) one_line: bool,
    /// A file with a token of one of these kinds (text outside the language's tags).
    pub(crate) any: Vec<SyntaxKind>,
}

impl File {
    pub(crate) fn is_one_line(&self, node: &SyntaxNode) -> bool {
        self.one_line
            && !node
                .descendants_with_tokens()
                .filter_map(SyntaxElement::into_token)
                .any(|t| t.text().contains('\n'))
    }

    fn verbatim(&self, node: &SyntaxNode) -> bool {
        self.is_one_line(node)
            || node
                .descendants_with_tokens()
                .any(|e| self.any.contains(&e.kind()))
    }
}

/// The rows of a language.
pub(crate) struct Layout {
    rows: Vec<Row>,
    /// For each node kind (by index) the rows that name it, in order: a node of a kind no row
    /// names is found in one step.
    by_kind: Vec<Vec<usize>>,
}

/// What stands before a `{`: a line break, or a blank when the row's option says `same_line`.
fn brace_gap(cx: &FormatCtx<'_>, row: &Row) -> Doc {
    match row.arg("gap").and_then(|g| g.strip_prefix("option:")) {
        Some(name) if cx.options.str(name) != "same_line" => doc::hardline(),
        _ => doc::lit(" "),
    }
}

fn block_style(row: &Row) -> BlockStyle {
    BlockStyle {
        max_blank_lines: usize::MAX,
        expand_empty: row.arg("empty") == Some("expand"),
        keep_leading_blank: true,
        ..BlockStyle::default()
    }
}

/// Does the source have a line break at the top level of the list between its delimiters?
fn own_break(cx: &FormatCtx<'_>, list: &SyntaxNode) -> bool {
    kids(cx, list).windows(2).any(|w| break_after(cx, &w[0]))
}

/// Is a `{ }` of these elements empty: nothing between the delimiters, not even a comment?
fn empty_braces(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> bool {
    els.len() == 2
        && els
            .last()
            .and_then(|e| first(cx, e))
            .is_some_and(|close| comments::leading_comments(cx.grammar, &close).is_empty())
}

fn statements(cx: &FormatCtx<'_>, items: &[SyntaxNode]) -> R {
    shapes::statement_sequence_with(
        cx,
        items,
        usize::MAX,
        Some(|cx, p, n| cx.blank_between(p, n)),
        false,
    )
}

impl Layout {
    pub(crate) fn parse(g: &Grammar, lines: &[&str]) -> Layout {
        let rows: Vec<Row> = lines.iter().map(|l| Row::parse(g, l)).collect();
        let mut by_kind: Vec<Vec<usize>> = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            for k in &r.kinds {
                let at = k.index() as usize;
                if by_kind.len() <= at {
                    by_kind.resize(at + 1, Vec::new());
                }
                by_kind[at].push(i);
            }
        }
        Layout { rows, by_kind }
    }

    /// The node laid out by the first row that matches it, `None` when no row does.
    pub(crate) fn format(
        &self,
        cx: &FormatCtx<'_>,
        node: &SyntaxNode,
        flows: (&Flow<'_>, &Flow<'_>),
        file: &File,
    ) -> Option<R> {
        let row = self
            .by_kind
            .get(node.kind().index() as usize)?
            .iter()
            .map(|&i| &self.rows[i])
            .find(|r| r.matches(node))?;
        let (flow, level) = flows;
        let generic = || flow.seq(cx, &kids(cx, node));
        Some(match row.shape.as_str() {
            "verbatim" => Ok(shapes::verbatim(cx, node)),
            "document" if file.verbatim(node) => Ok(doc::verbatim(node.text().to_string())),
            "document" => shapes::document(
                cx,
                node,
                usize::MAX,
                Some(|cx, p, n| cx.blank_between(p, n)),
            ),
            "scope" => scope(cx, node, row, flow).unwrap_or_else(generic),
            "braced" => braced(cx, node, row, flow).unwrap_or_else(generic),
            "with_body" => with_body(cx, node, row, flow),
            "block" => shapes::block(cx, node, &block_style(row)).or_else(|_| generic()),
            "case" => case(cx, node, row, flow).unwrap_or_else(generic),
            "colon_body" => colon_body(cx, node, row, flow).unwrap_or_else(generic),
            "list" => list(cx, node, row).unwrap_or_else(generic),
            "array" => array(cx, node, row, level),
            "braced_list" => braced_list(cx, node, row, flow).unwrap_or_else(generic),
            "heritage" => heritage(cx, node, row, flow).unwrap_or_else(generic),
            "chain" => chain(cx, node, row).unwrap_or_else(generic),
            "cases" => cases(cx, node, row, flow).unwrap_or_else(generic),
            other => panic!("layout: unknown shape `{other}`"),
        })
    }
}

/// `namespace A;` and the statements after it (the body node holds them and the `;`).
fn scope(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let Some(SyntaxElement::Node(body)) = els
        .last()
        .filter(|e| row.kinds_arg("body").contains(&e.kind()))
    else {
        return None;
    };
    let semi = body.children_with_tokens().find_map(|e| e.into_token());
    let items: Vec<SyntaxNode> = body.children().collect();
    Some((|| {
        let mut parts = vec![flow.seq(cx, &els[..els.len() - 1])?];
        if let Some(semi) = semi {
            parts.push(cx.token(&semi));
        }
        if !items.is_empty() {
            parts.push(doc::hardline());
            parts.push(doc::hardline());
            parts.push(statements(cx, &items)?);
        }
        Ok(doc::concat(parts))
    })())
}

/// The header, then the braces and the members.
fn braced(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let open = els
        .iter()
        .position(|e| row.kinds_arg("open").contains(&e.kind()))?;
    let (SyntaxElement::Token(open_tok), Some(SyntaxElement::Token(close_tok))) =
        (&els[open], els.last())
    else {
        return None;
    };
    Some((|| {
        let head = flow.seq(cx, &els[..open])?;
        let gap = if row.arg("gap") == Some("space") || empty_braces(cx, &els[open..]) {
            doc::lit(" ")
        } else {
            brace_gap(cx, row)
        };
        let style = BlockStyle {
            keep_leading_blank: false,
            ..block_style(row)
        };
        let body = shapes::block_between(cx, node, open_tok, close_tok, &style)?;
        Ok(doc::concat(vec![head, gap, body]))
    })())
}

/// A function: the signature, then the body on the line of the `)` when the parameters are on
/// lines of their own, else after the gap.
fn with_body(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> R {
    let els = kids(cx, node);
    let Some(SyntaxElement::Node(body)) = els
        .last()
        .filter(|e| row.kinds_arg("body").contains(&e.kind()))
    else {
        return flow.seq(cx, &els);
    };
    let head = flow.seq(cx, &els[..els.len() - 1])?;
    let multi = node
        .children()
        .find(|c| row.kinds_arg("params").contains(&c.kind()))
        .is_some_and(|l| own_break(cx, &l));
    let gap = if multi || empty_braces(cx, &kids(cx, body)) {
        doc::lit(" ")
    } else {
        brace_gap(cx, row)
    };
    Ok(doc::concat(vec![head, gap, cx.format(body)?]))
}

/// `case x:` and the statements below it.
fn case(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let colon = els
        .iter()
        .position(|e| row.kinds_arg("colon").contains(&e.kind()))?;
    let items: Vec<SyntaxNode> = els[colon + 1..]
        .iter()
        .filter_map(|e| e.as_node().cloned())
        .collect();
    if items.len() != els.len() - colon - 1 {
        return None;
    }
    Some((|| {
        let head = flow.seq(cx, &els[..=colon])?;
        if items.is_empty() {
            return Ok(head);
        }
        Ok(doc::concat(vec![
            head,
            doc::indent(doc::concat(vec![doc::hardline(), statements(cx, &items)?])),
        ]))
    })())
}

/// The alternative syntax: the colon, the statements indented, the clauses and the closing
/// keyword at the level of the head.
fn colon_body(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let colon = els
        .iter()
        .position(|e| row.kinds_arg("colon").contains(&e.kind()))?;
    let clause = |e: &SyntaxElement| row.kinds_arg("clauses").contains(&e.kind());
    let rest = &els[colon + 1..];
    let items: Vec<SyntaxNode> = rest
        .iter()
        .take_while(|e| e.as_node().is_some() && !clause(e))
        .filter_map(|e| e.as_node().cloned())
        .collect();
    Some((|| {
        let mut parts = vec![flow.seq(cx, &els[..=colon])?];
        if !items.is_empty() {
            parts.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                statements(cx, &items)?,
            ])));
        }
        let after = &rest[items.len()..];
        let tail = after
            .iter()
            .position(|e| e.as_token().is_some())
            .unwrap_or(after.len());
        for e in &after[..tail] {
            parts.push(doc::hardline());
            parts.push(el(cx, e)?);
        }
        if tail < after.len() {
            parts.push(doc::hardline());
            parts.push(flow.seq(cx, &after[tail..])?);
        }
        Ok(doc::concat(parts))
    })())
}

fn trailing(cx: &FormatCtx<'_>, row: &Row, when: bool) -> TrailingComma {
    if when && row.option_is(cx, "trailing") {
        TrailingComma::WhenBroken
    } else {
        TrailingComma::Never
    }
}

/// Arguments and parameters: all on one line, or, when the source breaks between them, one per
/// line with a trailing comma.
fn list(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row) -> Option<R> {
    node.children().next()?;
    let broken = own_break(cx, node);
    let stops = node.children().last().is_some_and(|c| {
        c.first_token()
            .is_some_and(|t| row.kinds_arg("no_trailing_after").contains(&t.kind()))
    });
    let style = ListStyle {
        layout: if broken {
            ListLayout::Break
        } else {
            ListLayout::Flat
        },
        trailing_comma: trailing(cx, row, broken && !stops),
        max_blank_lines: usize::MAX,
        ..ListStyle::default()
    };
    shapes::delimited_list(cx, node, &style).ok().map(Ok)
}

/// `[a, b]` as written, with a trailing comma when the closing bracket is on a line of its own.
fn array(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, level: &Flow<'_>) -> R {
    let els = kids(cx, node);
    let n = els.len();
    if let [open, SyntaxElement::Token(close)] = &els[..] {
        let dangling = comments::leading_comments(cx.grammar, close);
        if !dangling.is_empty() {
            return Ok(doc::concat(vec![
                el(cx, open)?,
                doc::indent(shapes::dangling_comments(&dangling, false)),
                doc::hardline(),
                cx.token_text(close),
            ]));
        }
    }
    let closes = n >= 2 && els[n - 1].as_token().is_some() && break_after(cx, &els[n - 2]);
    let commented = els
        .get(n.wrapping_sub(2))
        .and_then(|e| last(cx, e))
        .is_some_and(|t| comments::has_trailing_comment(cx.grammar, &t));
    let missing = closes
        && !commented
        && n > 2
        && els[n - 2]
            .as_token()
            .is_none_or(|t| !cx.grammar.has_tag(t.kind(), "separator"))
        && row.option_is(cx, "trailing");
    if !missing {
        return level.seq(cx, &els);
    }
    Ok(doc::concat(vec![
        level.seq(cx, &els[..n - 1])?,
        doc::lit(","),
        doc::hardline(),
        el(cx, &els[n - 1])?,
    ]))
}

/// `match (x) { a => b, }`: the arms one per line when the source breaks after the brace.
fn braced_list(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let open = els
        .iter()
        .position(|e| row.kinds_arg("open").contains(&e.kind()))?;
    let (SyntaxElement::Token(open_tok), Some(SyntaxElement::Token(close_tok))) =
        (&els[open], els.last())
    else {
        return None;
    };
    if !break_after(cx, &els[open]) {
        return None;
    }
    let style = ListStyle {
        layout: ListLayout::Break,
        trailing_comma: trailing(cx, row, true),
        max_blank_lines: usize::MAX,
        ..ListStyle::default()
    };
    Some((|| {
        let list = shapes::delimited_list_between(cx, node, open_tok, close_tok, &style)?;
        Ok(doc::concat(vec![
            flow.seq(cx, &els[..open])?,
            doc::lit(" "),
            list,
        ]))
    })())
}

/// An integer option named by the row's `key=option:NAME`.
fn option_int(cx: &FormatCtx<'_>, row: &Row, key: &str) -> usize {
    row.arg(key)
        .and_then(|v| v.strip_prefix("option:"))
        .map_or(usize::MAX, |name| cx.options.int(name) as usize)
}

/// `head : A, B body`: the supertypes (`list=K`) one per line from `min=option:NAME` of them on;
/// after a parameter list (`ctor=K`, then `params=K`) of `hug_min=option:NAME` that breaks, the
/// first stays on the line of the `)`.
fn heritage(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let colon = els
        .iter()
        .position(|e| row.kinds_arg("colon").contains(&e.kind()))?;
    let list = els
        .get(colon + 1)
        .and_then(|e| e.as_node())
        .filter(|n| row.kinds_arg("list").contains(&n.kind()))?;
    let entries: Vec<SyntaxElement> = kids(cx, list)
        .into_iter()
        .filter(|e| !cx.grammar.has_tag(e.kind(), "separator"))
        .collect();
    let params = els[..colon]
        .iter()
        .filter_map(|e| e.as_node())
        .find(|n| row.kinds_arg("ctor").contains(&n.kind()))
        .and_then(|c| {
            c.children()
                .find(|c| row.kinds_arg("params").contains(&c.kind()))
        })
        .map_or(0, |l| {
            l.children().filter(|c| !c.text_range().is_empty()).count()
        });
    let layout = if entries.len() < option_int(cx, row, "min") {
        shapes::HeritageLayout::Inline
    } else if params > 0 && params >= option_int(cx, row, "hug_min") {
        shapes::HeritageLayout::Hugged
    } else {
        shapes::HeritageLayout::Wrapped
    };
    let items = entries
        .iter()
        .map(|e| el(cx, e))
        .collect::<Result<Vec<_>, _>>();
    let tail = (colon + 2 < els.len()).then(|| flow.seq(cx, &els[colon + 2..]));
    Some((|| {
        Ok(shapes::heritage(
            flow.seq(cx, &els[..colon])?,
            el(cx, &els[colon])?,
            items?,
            layout,
            tail.transpose()?,
        ))
    })())
}

/// A member chain `a.b().c()` laid out by its outermost node (`spaced=K`: a link of that kind
/// follows its call after a blank).
fn chain(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row) -> Option<R> {
    let policy = shapes::ChainPolicy::of(cx.options);
    let inner = node
        .parent()
        .is_some_and(|p| row.kinds.contains(&p.kind()) && p.first_child().as_ref() == Some(node));
    if inner || shapes::flat_chain(cx, node, &policy) {
        return None;
    }
    Some(shapes::call_chain(
        cx,
        node,
        &policy,
        &SpacedLinks(cx, row.kinds_arg("spaced")),
    ))
}

struct SpacedLinks<'a, 'b>(&'a FormatCtx<'b>, &'a [SyntaxKind]);

impl shapes::ChainHooks for SpacedLinks<'_, '_> {
    fn element(&self, e: &SyntaxElement, _: bool) -> Option<R> {
        let n = e.as_node().filter(|n| self.1.contains(&n.kind()))?;
        Some(
            self.0
                .format(n)
                .map(|d| doc::concat(vec![doc::lit(" "), d])),
        )
    }
}

/// `when (x) { ... }`: the head, then the braces with a blank between cases when one spans lines.
fn cases(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let at = els
        .iter()
        .position(|e| row.kinds_arg("open").contains(&e.kind()))?;
    let (SyntaxElement::Token(open), Some(SyntaxElement::Token(close))) = (&els[at], els.last())
    else {
        return None;
    };
    let body = shapes::block_between(cx, node, open, close, &shapes::spread_cases_style(cx, node));
    Some((|| {
        Ok(doc::concat(vec![
            flow.seq(cx, &els[..at])?,
            doc::lit(" "),
            body?,
        ]))
    })())
}
