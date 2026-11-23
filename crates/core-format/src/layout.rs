//! Node layouts as data: the `[layout]` rows of a language's rules file name a shape for a kind
//! of node and the details of that shape, so the language holds only a table (see
//! [`crate::rules`]). A row is `KINDS[&parent=KINDS|&parent!=KINDS] : shape key=value ...`.
//!
//! Shapes: `verbatim`; `document [max_blank=option]` (the file; blank lines between statements from
//! `[blank]`, at most `max_blank_lines` kept with the key); `section entries=K` (a header and its
//! entries one per line);
//! `scope body=K` (`namespace A;` and the statements after it); `braced open=K` (a declaration
//! with its members in braces); `with_body body=K params=K` (a function: signature, then the
//! body); `block` (statements between braces); `case colon=K` (a head up to the colon, then
//! statements); `colon_body colon=K clauses=K` (the alternative syntax: `:` ... `end`); `list`
//! (a parenthesized list, one item per line when the source breaks it); `array` (a list kept as
//! written, with a trailing comma when its closing bracket is on a line of its own);
//! `braced_list open=K` (`match (x) { ... }`); `heritage colon=K list=K ctor=K params=K
//! min=option:N hug_min=option:N` (supertypes one per line); `chain spaced=K` (a member chain);
//! `cases open=K` (a `when` body, a blank between cases when one spans lines); `flow` (the tokens
//! and nodes with the spacing rows); `line_start` (the text as written, at column 0: a preprocessor
//! directive); `items` (the child nodes one per line at the level of the node).
//!
//! Keys: `empty=expand` (an empty block is two lines, else `{}`), `gap=option:NAME|space` (what
//! stands before a brace: a line break, or a blank when the option is `same_line`),
//! `trailing=option:NAME=VALUE` (a trailing comma when the option has the value),
//! `no_trailing_after=K` (none after a last item that starts with that token).
//!
//! The kind sets of the language's options (`kinds:` of an option in `lang.yaml`, [`Binding`]) say
//! which constructs an option governs; the shapes decide: `fit` (a block of that parent stays on
//! one line when it fits: a bool option that is on, or `all`/`inline`), `no_fit_under` and
//! `no_fit_with` (a body over several lines stays so under such a grandparent, or beside such a
//! child), `flat` (the items of the block are not indented when the option is `none`), `join` and
//! `head` (`compound`), `outdent` (columns of an int option), `outdent_unless` (one indent unless
//! the bool option is on), `flush_unless` (column 0, one indent when it is on), `tight` (no blanks after the
//! `#` of a directive when the option is `none`) and `attach` (words of a `words` option: the `{`
//! of such a construct goes on its own line).
//!
//! concept: node-layout-rows
//! what: Layout rows name a shape and its details for a kind of node, so a language holds only a table.
//! tools: none (each tool layout as options)
//! reads: rules:layout

use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::rules::Binding;
use crate::seq::{break_after, el, first, kids, last};
use crate::shapes::flow::Flow;
use crate::shapes::{self, BlockStyle, ListLayout, ListStyle, TrailingComma};
use core_config::OptId;
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
    "entries",
];

pub(crate) struct Row {
    kinds: Vec<SyntaxKind>,
    parent: Vec<SyntaxKind>,
    parent_not: bool,
    shape: String,
    text: Vec<(String, String)>,
    kind_args: Vec<(String, Vec<SyntaxKind>)>,
    /// The keys whose value is `option:NAME[=VALUES]`: the key, the option and the values.
    option_args: Vec<(String, OptId, String)>,
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
        let (mut text, mut kind_args, mut option_args) = (Vec::new(), Vec::new(), Vec::new());
        for w in words {
            let (k, v) = w.split_once('=').unwrap_or((w, ""));
            if KIND_KEYS.contains(&k) {
                kind_args.push((k.to_string(), kinds_of(v)));
            } else {
                if let Some(o) = v.strip_prefix("option:") {
                    let (name, values) = o.split_once('=').unwrap_or((o, ""));
                    option_args.push((
                        k.to_string(),
                        core_config::opt::must(name),
                        values.to_string(),
                    ));
                }
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
            option_args,
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

    /// The option and the values of a key `option:NAME[=VALUES]`.
    fn option(&self, key: &str) -> Option<(OptId, &str)> {
        self.option_args
            .iter()
            .find(|(k, ..)| k == key)
            .map(|(_, id, values)| (*id, values.as_str()))
    }

    /// `option:NAME=VALUE`: does the option have the value?
    fn option_is(&self, cx: &FormatCtx<'_>, key: &str) -> bool {
        self.option(key)
            .filter(|(_, values)| !values.is_empty())
            .is_some_and(|(id, values)| {
                values.split('|').any(|v| match cx.options.get(id) {
                    Some(core_config::OptionValue::Bool(b)) => v == b.to_string(),
                    _ => cx.options.is(id, v),
                })
            })
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
    /// The kind sets of the language's options.
    bindings: Vec<Binding>,
    /// For each node kind (by index) the rows that name it, in order: a node of a kind no row
    /// names is found in one step.
    by_kind: Vec<Vec<usize>>,
}

/// Is there a line break between the first and the last code token of these elements?
fn line_break_inside(els: impl Iterator<Item = SyntaxElement>) -> bool {
    let (mut code, mut pending) = (false, false);
    for t in els.filter_map(SyntaxElement::into_token) {
        let text = t.text();
        if text.trim().is_empty() {
            pending |= code && text.contains('\n');
        } else {
            if pending || text.contains('\n') {
                return true;
            }
            code = true;
        }
    }
    false
}

/// What stands before a `{`: a line break, or a blank when the row's option says `same_line`.
fn brace_gap(cx: &FormatCtx<'_>, row: &Row) -> Doc {
    match row.option("gap") {
        Some((id, _)) if cx.options.str(id) != "same_line" => doc::hardline(),
        _ => doc::lit(" "),
    }
}

/// The language's `max_blank_lines` (all the blank lines of the source without the option).
fn max_blank(cx: &FormatCtx<'_>) -> usize {
    cx.max_blank_lines
}

fn block_style(cx: &FormatCtx<'_>, row: &Row) -> BlockStyle {
    BlockStyle {
        max_blank_lines: max_blank(cx),
        keep_trailing_blank: true,
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
        max_blank(cx),
        Some(|cx, p, n| cx.blank_between(p, n)),
        false,
    )
}

impl Layout {
    pub(crate) fn parse(g: &Grammar, lines: &[&str], bindings: Vec<Binding>) -> Layout {
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
        Layout {
            rows,
            bindings,
            by_kind,
        }
    }

    /// The kind set of `role` that holds `kind` (the option it belongs to and the set).
    fn bound(&self, role: &str, kind: SyntaxKind) -> Option<&Binding> {
        (self.bindings.iter()).find(|b| b.role == role && b.kinds.contains(&kind))
    }

    /// The kinds of the sets of `role` that belong to `option`.
    fn bound_kinds(&self, option: OptId, role: &str) -> Vec<SyntaxKind> {
        (self.bindings.iter())
            .filter(|b| b.role == role && b.option == option)
            .flat_map(|b| b.kinds.iter().copied())
            .collect()
    }

    /// The option of `role` that governs a node of this kind, when it has the value `none`.
    fn is_none(&self, cx: &FormatCtx<'_>, role: &str, kind: SyntaxKind) -> bool {
        (self.bound(role, kind)).is_some_and(|b| cx.options.is(b.option, "none"))
    }

    /// `fit`: a bool option that is on, or one of `all` and `inline`.
    fn fits(&self, cx: &FormatCtx<'_>, kind: SyntaxKind) -> bool {
        self.bound("fit", kind)
            .is_some_and(|b| match cx.options.get(b.option) {
                Some(core_config::OptionValue::Bool(on)) => *on,
                _ => cx.options.is(b.option, "all") || cx.options.is(b.option, "inline"),
            })
    }

    /// How far the node moves left: `outdent` (columns of an option), `outdent_unless` (one indent
    /// unless the bool option is on), `flush_unless` (to column 0, one indent when it is on).
    fn outdent(&self, cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<doc::Outdent> {
        if self.bindings.is_empty() {
            return None;
        }
        let kind = node.kind();
        let off = |b: &Binding| !cx.options.flag(b.option, true);
        if let Some(b) = self.bound("outdent", kind) {
            return Some(doc::Outdent::Columns(cx.options.uint(b.option, 0)));
        }
        if self.bound("outdent_unless", kind).is_some_and(off) {
            return Some(doc::Outdent::Columns(
                cx.options.uint(core_config::shared::INDENT_SIZE, 2),
            ));
        }
        self.bound("flush_unless", kind)
            .map(|b| doc::Outdent::Levels(if off(b) { usize::MAX } else { 1 }))
    }

    /// `break_after`: a declaration with a header (`template <...>`, the `head` kind of its option)
    /// and its declaration; the option says if the header ends its line (`yes`, `no`,
    /// `multiline`).
    fn header_break(&self, cx: &FormatCtx<'_>, node: &SyntaxNode, flow: &Flow<'_>) -> Option<R> {
        let b = self.bound("break_after", node.kind())?;
        let heads = self.bound_kinds(b.option, "head");
        let els = kids(cx, node);
        let head = els
            .iter()
            .position(|e| e.as_node().is_some_and(|n| heads.contains(&n.kind())))?;
        let mode = cx.options.str(b.option);
        let commented = crate::seq::last(cx, &els[head])
            .is_some_and(|t| comments::has_trailing_comment(cx.grammar, &t));
        Some(crate::header_break::header_break(
            node,
            &els,
            head,
            mode,
            commented,
            &|run| flow.seq(cx, run),
        ))
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
        // The comments in front of an outdented node go with it, unless a blank line separates
        // them: then they stay at the indentation of its neighbours.
        let outdent = self.outdent(cx, node);
        let lead = if outdent.is_some() {
            cx.first_token(node).filter(|t| {
                comments::leading_comments(cx.grammar, t)
                    .last()
                    .is_some_and(|c| c.breaks_after > 1)
            })
        } else {
            None
        };
        if let Some(t) = &lead {
            cx.hold_leading(t);
        }
        let doc = match row.shape.as_str() {
            "verbatim" => Ok(shapes::verbatim(cx, node)),
            "flat" => flow.flat(cx, node),
            "document" if file.verbatim(node) => {
                cx.note_copied(node.text_range());
                Ok(doc::verbatim(node.text().to_string()))
            }
            "document" => shapes::document(
                cx,
                node,
                if row
                    .arg("max_blank")
                    .is_some_and(|v| v.starts_with("option"))
                {
                    max_blank(cx)
                } else {
                    usize::MAX
                },
                Some(|cx, p, n| cx.blank_between(p, n)),
            )
            .map(|d| {
                // The blank lines at the start of the file stay.
                if !cx
                    .options
                    .flag(core_config::opt::KEEP_LEADING_BLANK_LINES, false)
                {
                    return d;
                }
                let blanks: usize = (node.descendants_with_tokens())
                    .filter_map(SyntaxElement::into_token)
                    .map_while(|t| {
                        t.text()
                            .trim()
                            .is_empty()
                            .then(|| t.text().matches('\n').count())
                    })
                    .sum();
                match blanks {
                    0 => d,
                    n => doc::concat((0..n).map(|_| doc::hardline()).chain([d]).collect()),
                }
            }),
            "key_value" => shapes::key_value(
                cx,
                node,
                &shapes::KeyValueStyle {
                    space_before_separator: row.arg("space_before") == Some("yes"),
                    space_after_separator: true,
                    // (an option of the language, when it has one)
                    align_comments: matches!(
                        cx.options.get(core_config::opt::ALIGN_COMMENTS),
                        Some(core_config::OptionValue::Bool(true))
                    ),
                },
            ),
            "section" => shapes::section(cx, node, row.kinds_arg("entries")),
            "scope" => scope(cx, node, row, flow).unwrap_or_else(generic),
            "braced" => braced(cx, node, row, flow).unwrap_or_else(generic),
            "with_body" => with_body(cx, node, row, flow),
            "block" => {
                let mut style = block_style(cx, row);
                let parent = node.parent().map(|p| p.kind());
                style.flat = parent.is_some_and(|p| self.is_none(cx, "flat", p));
                style.fit_single = parent.is_some_and(|p| self.fits(cx, p));
                // (`empty=as_written`: an empty body over two lines stays so)
                let as_written = row.arg("empty") == Some("as_written");
                if style.fit_single || as_written {
                    let multiline = line_break_inside(node.descendants_with_tokens());
                    if style.fit_single && multiline {
                        // A body over several lines stays so after a signature over several lines,
                        // a template header or a constructor's initializers.
                        let option = (node.parent())
                            .and_then(|p| self.bound("fit", p.kind()))
                            .map(|b| b.option);
                        let kinds =
                            |role| option.map_or_else(Vec::new, |o| self.bound_kinds(o, role));
                        let with = kinds("no_fit_with");
                        let under = kinds("no_fit_under");
                        let keeps = node.parent().is_some_and(|p| {
                            let start = node.text_range().start();
                            let header = p
                                .descendants_with_tokens()
                                .take_while(|e| e.text_range().start() < start);
                            line_break_inside(header)
                                || p.children().any(|c| with.contains(&c.kind()))
                                || (p.parent()).is_some_and(|g| under.contains(&g.kind()))
                        });
                        style.fit_single &= !keeps;
                    }
                    style.expand_empty |= multiline && as_written;
                }
                shapes::block(cx, node, &style).or_else(|_| generic())
            }
            "compound" => compound(cx, node, self, flow).unwrap_or_else(generic),
            "case" => case(cx, node, row, flow).unwrap_or_else(generic),
            "colon_body" => colon_body(cx, node, row, flow).unwrap_or_else(generic),
            "list" => list(cx, node, row).unwrap_or_else(generic),
            "array" => array(cx, node, row, level),
            "braced_list" => braced_list(cx, node, row, flow).unwrap_or_else(generic),
            "heritage" => heritage(cx, node, row, flow).unwrap_or_else(generic),
            "chain" => chain(cx, node, row).unwrap_or_else(generic),
            "cases" => cases(cx, node, row, flow).unwrap_or_else(generic),
            "flow" => self.header_break(cx, node, flow).unwrap_or_else(generic),
            // (a token between the nodes, a `,` after an initializer, has no line of its own: the
            // lines keep their indentation, their blanks collapse)
            "items" if kids(cx, node).iter().any(|e| e.as_token().is_some()) => {
                Ok(doc::line_start(crate::relay::relay(
                    node.text().to_string().trim(),
                    crate::relay::Align::None,
                    0,
                    None,
                )))
            }
            "items" => statements(cx, &node.children().collect::<Vec<_>>()),
            "line_start" => {
                let relay = self.bound("relay", node.kind()).map(|b| {
                    (
                        crate::relay::Align::parse(cx.options.str(b.option)),
                        cx.options.uint(core_config::opt::LINE_WIDTH, 80),
                    )
                });
                Ok(line_start(
                    cx,
                    node,
                    self.is_none(cx, "tight", node.kind()),
                    relay,
                ))
            }
            other => panic!("layout: unknown shape `{other}`"),
        };
        Some(match outdent {
            Some(by) => doc.map(|d| {
                let d = Doc::Outdent(by, Box::new(d));
                match &lead {
                    Some(t) => doc::concat(vec![cx.held_leading(t), d]),
                    None => d,
                }
            }),
            None => doc,
        })
    }
}

/// The node's text from its first token on, at column 0, after its leading comments (a
/// preprocessor directive).
fn line_start(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    tight_hash: bool,
    relay: Option<(crate::relay::Align, usize)>,
) -> Doc {
    let style = &cx.comment_style;
    let gap = crate::relay::CommentGap {
        gap: style.gap,
        aligned: style.align_lines,
    };
    let text = node.text().to_string();
    let tight = |t: &str| {
        let t = match t.strip_prefix('#') {
            Some(rest) if tight_hash => {
                std::borrow::Cow::Owned(format!("#{}", rest.trim_start_matches([' ', '\t'])))
            }
            _ => std::borrow::Cow::Borrowed(t),
        };
        match relay {
            Some((align, width)) => crate::relay::relay(&t, align, width, Some(gap)),
            None => t.into_owned(),
        }
    };
    let Some(first) = cx.first_token(node) else {
        return doc::line_start(tight(text.trim()));
    };
    let from = usize::from(first.text_range().start() - node.text_range().start());
    doc::concat(vec![
        cx.leading_comments_doc(&first),
        doc::line_start(tight(text[from..].trim())),
    ])
}

/// A head (`if (c)`), its body and the `else` that follows: a block stays on the line of the
/// head, another statement goes on the next line unless the option of `join=option:NAME` lets
/// it stay (`all`, `without_else` when there is no `else`, or true).
fn compound(cx: &FormatCtx<'_>, node: &SyntaxNode, layout: &Layout, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let option = layout.bound("join", node.kind()).map(|b| b.option);
    let heads = option.map_or_else(Vec::new, |o| layout.bound_kinds(o, "head"));
    let at = els
        .iter()
        .position(|e| matches!(e, SyntaxElement::Node(n) if !heads.contains(&n.kind())))?;
    let has_else = match &els[at + 1..] {
        [] => false,
        [SyntaxElement::Token(_), SyntaxElement::Node(_)] => true,
        _ => return None,
    };
    let join = match option.map(|id| (id, cx.options.get(id))) {
        Some((_, Some(core_config::OptionValue::Bool(true)))) => shapes::Join::Fits,
        Some((id, Some(core_config::OptionValue::String(_)))) => match cx.options.str(id) {
            "all" => shapes::Join::Fits,
            "without_else" if !has_else => shapes::Join::Fits,
            _ => shapes::Join::Never,
        },
        _ => shapes::Join::Never,
    };
    // (`while (x);`; a `;` with a comment before it is a body of its own)
    let empty = |e: &SyntaxElement| {
        let semi = match e {
            SyntaxElement::Token(t) => t.text() == ";",
            SyntaxElement::Node(n) => n.text().to_string().trim() == ";",
        };
        semi && first(cx, e).is_none_or(|t| comments::leading_comments(cx.grammar, &t).is_empty())
    };
    // (a line comment after the head or the `else` ends its line: the body goes below)
    let ends_line = |before: &SyntaxElement| {
        crate::seq::last(cx, before).is_some_and(|t| {
            comments::trailing_comments(cx.grammar, &t)
                .iter()
                .any(|c| c.kind == comments::CommentKind::Line)
        })
    };
    let body = |e: &SyntaxElement, before: &SyntaxElement, inline: bool| -> R {
        let printed = el(cx, e)?;
        Ok(if empty(e) {
            printed
        } else {
            let apart = || {
                let (a, b) = (crate::seq::last(cx, before)?, first(cx, e)?);
                flow.attach(cx, &a, &b).map(|attached| !attached)
            };
            if cx.grammar.has_tag(e.kind(), "block") && (ends_line(before) || apart() == Some(true))
            {
                doc::concat(vec![doc::hardline(), printed])
            } else {
                let join = if ends_line(before) {
                    shapes::Join::Never
                } else {
                    join
                };
                shapes::compound_body(cx, e, printed, inline, join)
            }
        })
    };
    Some((|| {
        let mut parts = vec![
            flow.seq(cx, &els[..at])?,
            body(&els[at], &els[at - 1], false)?,
        ];
        if has_else {
            let SyntaxElement::Token(kw) = &els[at + 1] else {
                unreachable!("checked above")
            };
            parts.push(shapes::compound_continuation(cx, &els[at], kw));
            parts.push(el(cx, &els[at + 1])?);
            let chained = els[at + 2].kind() == node.kind();
            parts.push(body(&els[at + 2], &els[at + 1], chained)?);
        }
        Ok(doc::concat(parts))
    })())
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
            ..block_style(cx, row)
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
        // A block as the only body opens on the line of the label.
        if let [block] = &items[..]
            && cx.grammar.has_tag(block.kind(), "block")
        {
            return Ok(doc::concat(vec![
                head,
                doc::lit(" "),
                statements(cx, &items)?,
            ]));
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
                doc::indent(shapes::dangling_comments_in(
                    &cx.comment_style,
                    &dangling,
                    false,
                )),
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
    row.option(key)
        .map_or(usize::MAX, |(id, _)| cx.options.int(id) as usize)
}

/// `head : A, B body`: the supertypes (`list=K`) one per line from `min=option:NAME` of them on;
/// after a parameter list (`ctor=K`, then `params=K`) of `hug_min=option:NAME` that breaks, the
/// first stays on the line of the `)`.
fn heritage(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row, flow: &Flow<'_>) -> Option<R> {
    let els = kids(cx, node);
    let own = match ctor_own_line(cx, &els, row) {
        Some(Ok(own)) => Some(own),
        Some(Err(e)) => return Some(Err(e)),
        None => None,
    };
    let supertypes = els
        .iter()
        .position(|e| row.kinds_arg("colon").contains(&e.kind()))
        .and_then(|colon| {
            let list = els.get(colon + 1).and_then(|e| e.as_node())?;
            row.kinds_arg("list")
                .contains(&list.kind())
                .then(|| (colon, list.clone()))
        });
    let Some((colon, list)) = supertypes else {
        // (no supertypes: what follows the constructor stays on its line)
        let (at, mut parts) = own?;
        return Some(flow.seq(cx, &els[..at]).and_then(|head| {
            if at + 1 < els.len() {
                parts.extend([doc::lit(" "), flow.seq(cx, &els[at + 1..])?]);
            }
            Ok(doc::concat(vec![head, doc::indent(doc::concat(parts))]))
        }));
    };
    let list = &list;
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
        shapes::HeritageLayout::Fit
    } else if params > 0 && params >= option_int(cx, row, "hug_min") {
        shapes::HeritageLayout::Hugged
    } else {
        shapes::HeritageLayout::Wrapped
    };
    let items = entries
        .iter()
        .map(|e| el(cx, e))
        .collect::<Result<Vec<_>, _>>();
    // (a constructor that starts with a word of `wrapped_body_indent_words` puts the body of a
    // wrapped supertype list one level further in)
    let deeper = layout == shapes::HeritageLayout::Wrapped
        && own.is_none()
        && els[..colon]
            .iter()
            .filter_map(|e| e.as_node())
            .filter(|n| row.kinds_arg("ctor").contains(&n.kind()))
            .flat_map(|c| c.children_with_tokens())
            .filter_map(|e| e.into_token())
            .any(|t| {
                cx.options
                    .list_has(core_config::opt::WRAPPED_BODY_INDENT_WORDS, t.text())
            });
    let tail = (colon + 2 < els.len()).then(|| {
        flow.seq(cx, &els[colon + 2..])
            .map(|t| if deeper { doc::indent(t) } else { t })
    });
    Some((|| {
        let mut head_breaks = own.is_some();
        let mut at = None;
        let mut head = None;
        if let Some((i, parts)) = own {
            // (the constructor below the name: it is the head, and the whole clause is one level in)
            at = Some(i);
            head = Some(doc::concat(parts));
        } else {
            for e in &els[..colon] {
                if row.kinds_arg("ctor").contains(&e.kind()) {
                    head_breaks = el(cx, e)?.will_break();
                }
            }
        }
        let clause = shapes::heritage(
            match head {
                Some(h) => h,
                None => flow.seq(cx, &els[..colon])?,
            },
            head_breaks,
            el(cx, &els[colon])?,
            items?,
            layout,
            tail.transpose()?,
        );
        Ok(match at {
            Some(i) => doc::concat(vec![flow.seq(cx, &els[..i])?, doc::indent(clause)]),
            None => clause,
        })
    })())
}

/// A constructor (`ctor=K`) with a modifier of the kinds of `ctor_own_line_kinds` (annotations) goes below the name, one level in, with its annotations on lines of their own;
/// what follows it (the supertypes, the body) stays under that level.
fn ctor_own_line(
    cx: &FormatCtx<'_>,
    els: &[SyntaxElement],
    row: &Row,
) -> Option<Result<(usize, Vec<Doc>), FormatError>> {
    let g = cx.grammar;
    let at = els
        .iter()
        .position(|e| row.kinds_arg("ctor").contains(&e.kind()))?;
    let ctor = els[at].as_node()?;
    // (a constructor the source has below the name stays there)
    let mut prev = cx.first_token(ctor).and_then(|t| t.prev_token());
    while let Some(p) = prev.as_ref().filter(|p| g.is_trivia(p.kind())) {
        prev = p.prev_token();
    }
    let below = prev.is_some_and(|t| comments::break_after(g, &t));
    (below
        || ctor
            .children()
            .filter(|m| g.has_tag(m.kind(), "modifiers"))
            .flat_map(|m| m.children())
            .any(|c| {
                cx.options
                    .list_has(core_config::opt::CTOR_OWN_LINE_KINDS, g.name(c.kind()))
            }))
    .then_some(())?;
    Some((|| {
        let mut parts = vec![doc::hardline()];
        for e in kids(cx, ctor) {
            match e.as_node().filter(|m| g.has_tag(m.kind(), "modifiers")) {
                Some(m) => {
                    for x in kids(cx, m) {
                        parts.push(el(cx, &x)?);
                        let note = g.has_tag(x.kind(), "annotation");
                        parts.push(if note { doc::hardline() } else { doc::lit(" ") });
                    }
                }
                None => parts.push(el(cx, &e)?),
            }
        }
        Ok((at, parts))
    })())
}

/// A member chain `a.b().c()` laid out by its outermost node (`spaced=K`: a link of that kind
/// follows its call after a blank).
fn chain(cx: &FormatCtx<'_>, node: &SyntaxNode, row: &Row) -> Option<R> {
    let policy = cx.chain_policy();
    let inner = node
        .parent()
        .is_some_and(|p| row.kinds.contains(&p.kind()) && p.first_child().as_ref() == Some(node));
    if inner || shapes::flat_chain(cx, node, policy) {
        return None;
    }
    Some(shapes::call_chain(
        cx,
        node,
        policy,
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
