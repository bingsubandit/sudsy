//! Call arguments: the parenthesized list of a call, with the layouts Prettier calls hugging.
//!
//! [`plan`] reads the arguments' kinds and decides, from grammar tags and the `[call]` rows and the language's `CallSets`
//! ([`crate::calls`]), whether the list must break (a function composition) and which argument
//! hugs: the last one or the first one, kept on the call's line while its own body breaks.
//! [`Args::layout`] then builds the document: the conditional group of flat, hugged and
//! all-broken-out candidates, or the plain group of the whole list.
//!
//! Tags: `#huggable-always` (a function with a block body), `#huggable` (an object or array: hugs
//! when it has items), `#huggable-function` (a
//! function or lambda: hugs when its body is a `#block`, a `#huggable-body` (object, array, JSX), an
//! arrow function that hugs, or a `#huggable-tail` (a call or conditional) unless the function is
//! part of an arrow chain; not when its return type is a `#named-type` and the body is an empty
//! block), `#hug-wrapper` (a cast: hugs when its `#expression` child does), `#argument` (a node that
//! holds one argument expression: the last child node).
//!
//! concept: call-arguments-hug
//! what: Call arguments flat, hugged (first or last) or broken out, decided by grammar tags, `[call]` rows and call kind sets.
//! tools: Prettier call-arguments (last-arg hug, test calls, composition, React hooks), google-java-format argument lists
//! reads: rules:call, tag:huggable

use super::{ListStyle, container, delimited_list};
use crate::FormatCtx;
use crate::calls::{CallRules, has_comment};
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::rules::RuleFormatter;
use crate::seq;
use core_config::ResolvedOptions;
use core_syntax::SyntaxNode;
use core_syntax::roles::{outer, parent, un};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HugLast {
    Never,
    /// Only an argument that is alone.
    Sole,
    /// Only an argument that is alone and a function with no parameters or one without
    /// parentheses (`x => ...`, `() => ...`).
    SoleBare,
    /// The last of any number of arguments.
    Last,
}

#[derive(Clone, Copy, Debug)]
pub struct CallStyle {
    /// A function that is the first of two arguments hugs (`setTimeout(() => {...}, 500)`).
    pub first: bool,
    pub last: HugLast,
    /// The body of a lambda with an expression body that is the only argument hangs below its
    /// head (`Foo(x =>` / `    body` / `)`) when the call does not fit.
    pub body: bool,
}

impl CallStyle {
    /// The options `hug_first_argument` and `hug_last_argument` (a language without them never
    /// hugs).
    pub fn of(options: &ResolvedOptions) -> CallStyle {
        CallStyle {
            first: options
                .get(core_config::opt::HUG_FIRST_ARGUMENT)
                .is_some_and(|_| options.bool(core_config::opt::HUG_FIRST_ARGUMENT)),
            body: false,
            last: match options
                .get(core_config::opt::HUG_LAST_ARGUMENT)
                .map(|_| options.str(core_config::opt::HUG_LAST_ARGUMENT))
            {
                Some("last") => HugLast::Last,
                Some("sole") => HugLast::Sole,
                Some("sole_bare") => HugLast::SoleBare,
                _ => HugLast::Never,
            },
        }
    }
}

/// What [`plan`] found.
#[derive(Clone, Copy, Debug, Default)]
pub struct Plan {
    /// The list breaks whatever the width: a function composition (`compose(a => b, c => d)`).
    pub broken: bool,
    /// The argument that hugs: its index and whether it is the first of the two.
    pub hug: Option<(usize, bool)>,
}

/// The expression of an argument: a node tagged `#argument` holds it as its last child node.
pub fn expression_of(cx: &FormatCtx<'_>, arg: &SyntaxNode) -> SyntaxNode {
    if cx.grammar.has_tag(arg.kind(), "argument") {
        arg.last_child().unwrap_or_else(|| arg.clone())
    } else {
        un(arg)
    }
}

/// Can the node `n` be printed with its body expanded while it stays on the call's line?
pub fn can_hug(cx: &FormatCtx<'_>, n: &SyntaxNode, arrow_chain: bool) -> bool {
    let g = cx.grammar;
    let k = n.kind();
    if g.has_tag(k, "huggable-always") {
        return true;
    }
    if g.has_tag(k, "huggable") {
        // (more than its two delimiters: an item may be a bare token)
        return n
            .children_with_tokens()
            .filter(|e| !g.is_trivia(e.kind()))
            .nth(2)
            .is_some();
    }
    if g.has_tag(k, "hug-wrapper") {
        return n
            .children()
            .find(|c| g.has_tag(c.kind(), "expression"))
            .is_some_and(|c| can_hug(cx, &un(&c), false));
    }
    if !g.has_tag(k, "huggable-function") {
        return false;
    }
    let Some(body) = n.last_child() else {
        return false;
    };
    let body = un(&body);
    let bk = body.kind();
    let block = g.has_tag(bk, "block");
    let returns_named = g
        .slot(n, "return_type")
        .and_then(|t| t.into_node())
        .and_then(|t| t.first_child())
        .is_some_and(|t| g.has_tag(t.kind(), "named-type"));
    if returns_named && !(block && body.children().next().is_some()) {
        return false;
    }
    if block || g.has_tag(bk, "huggable-body") {
        true
    } else if g.has_tag(bk, "huggable-function") {
        can_hug(cx, &body, true)
    } else if g.has_tag(bk, "hug-wrapper") {
        can_hug(cx, &body, false)
    } else if g.has_tag(bk, "tail-wrapper") {
        let inner = body.first_child().map(|c| un(&c));
        !arrow_chain && inner.is_some_and(|c| g.has_tag(c.kind(), "huggable-tail"))
    } else {
        g.has_tag(bk, "huggable-tail") && !arrow_chain
    }
}

/// Does `arg`, the last of `count` arguments, hug the call by `style` (its kind only: comments
/// are the caller's business)?
pub fn hugs_last(cx: &FormatCtx<'_>, style: &CallStyle, arg: &SyntaxNode, count: usize) -> bool {
    match style.last {
        HugLast::Never => false,
        HugLast::Sole => count == 1 && can_hug(cx, &expression_of(cx, arg), false),
        HugLast::SoleBare => {
            let e = expression_of(cx, arg);
            count == 1 && can_hug(cx, &e, false) && has_bare_parameters(cx, &e)
        }
        HugLast::Last => can_hug(cx, &expression_of(cx, arg), false),
    }
}

/// Does the function `f` take no parameters, or one without parentheses? A function with no
/// `parameters` slot does not.
fn has_bare_parameters(cx: &FormatCtx<'_>, f: &SyntaxNode) -> bool {
    let g = cx.grammar;
    match g.slot(f, "parameters") {
        Some(core_syntax::SyntaxElement::Node(p)) => {
            !g.has_tag(p.kind(), "delimited-list") || p.children().next().is_none()
        }
        _ => false,
    }
}

/// A function composition: more than one function argument, or an argument that is a call with a
/// function argument.
fn is_composition(cx: &FormatCtx<'_>, args: &[SyntaxNode]) -> bool {
    let g = cx.grammar;
    if args.len() <= 1 {
        return false;
    }
    let is_fn = |n: &SyntaxNode| g.has_tag(n.kind(), "function");
    let mut count = 0;
    for n in args {
        if is_fn(n) {
            count += 1;
            if count > 1 {
                return true;
            }
        } else if g.has_tag(n.kind(), "call")
            && g.slots(n, "arguments")
                .filter_map(|e| e.into_node())
                .any(|a| is_fn(&un(&a)))
        {
            return true;
        }
    }
    false
}

fn should_group_last(
    cx: &FormatCtx<'_>,
    rules: &CallRules,
    args: &[SyntaxNode],
    style: &CallStyle,
) -> bool {
    let Some(last) = args.last() else {
        return false;
    };
    let penultimate = args.len().checked_sub(2).map(|i| &args[i]);
    match style.last {
        HugLast::Never => return false,
        HugLast::Sole if args.len() != 1 => return false,
        HugLast::SoleBare if args.len() != 1 || !has_bare_parameters(cx, last) => return false,
        _ => {}
    }
    !has_comment(cx, last)
        && can_hug(cx, last, false)
        && penultimate.is_none_or(|p| p.kind() != last.kind())
        && (args.len() != 2
            || penultimate.is_none_or(|p| !rules.not_last(cx.grammar, p.kind(), last.kind())))
        && !(args.len() > 1 && rules.is_concise_array(cx, last))
}

fn could_group_first(cx: &FormatCtx<'_>, rules: &CallRules, args: &[SyntaxNode]) -> bool {
    let g = cx.grammar;
    let [first, second] = args else {
        return false;
    };
    !has_comment(cx, first)
        && g.has_tag(first.kind(), "function")
        && g.slot(first, "body")
            .is_some_and(|b| g.has_tag(b.kind(), "block"))
        && !g.has_tag(second.kind(), "function")
        && !g.has_tag(second.kind(), "conditional")
        && rules.is_short(cx, second)
        && !can_hug(cx, second, false)
}

/// The layout the arguments `args` (each without the parentheses around it) of the call `node`
/// call for.
pub fn plan(
    cx: &FormatCtx<'_>,
    rules: &CallRules,
    node: &SyntaxNode,
    args: &[SyntaxNode],
    style: &CallStyle,
) -> Plan {
    let broken = rules.composition()
        && is_composition(cx, args)
        && !parent(node).is_some_and(|p| rules.no_composition(cx.grammar, p.kind()));
    if broken {
        return Plan { broken, hug: None };
    }
    let hug = if style.first && could_group_first(cx, rules, args) {
        Some((0, true))
    } else if should_group_last(cx, rules, args, style) {
        Some((args.len() - 1, false))
    } else {
        None
    };
    Plan { broken, hug }
}

/// How the hugged argument is printed.
pub enum Expanded {
    /// As it is.
    Same,
    /// As it is, and only on the call's line as a whole: a function whose parameters would break
    /// does not hug.
    Whole,
    /// In the layout the caller made for it.
    Doc(Doc),
    /// It cannot hug (its parameters would have to break): every argument on a line of its own.
    Bailout,
}

/// One printed argument.
pub struct Item {
    pub doc: Doc,
    /// The comma after it (not for the last).
    pub sep: Doc,
    /// A blank line stands between this argument and the next in the source.
    pub blank_after: bool,
}

/// The printed arguments and delimiters of a call.
pub struct Args {
    open: Doc,
    close: Doc,
    docs: Vec<Doc>,
    seps: Vec<Doc>,
    printed: Vec<Doc>,
    any_blank: bool,
    trailing: Doc,
    dangling: Doc,
    hug_close: bool,
}

impl Args {
    /// `trailing`: what follows the last argument when the list breaks (a comma);
    /// `dangling`: comments in front of the `)`.
    pub fn new(open: Doc, close: Doc, items: Vec<Item>, trailing: Doc, dangling: Doc) -> Args {
        let n = items.len();
        let mut a = Args {
            open,
            close,
            docs: Vec::with_capacity(n),
            seps: Vec::with_capacity(n),
            printed: Vec::with_capacity(n),
            any_blank: false,
            trailing,
            dangling,
            hug_close: false,
        };
        for (i, it) in items.into_iter().enumerate() {
            let doc = doc::shared(it.doc);
            let mut parts = vec![doc.clone()];
            if i + 1 < n {
                if it.blank_after {
                    a.any_blank = true;
                    parts.extend([it.sep.clone(), doc::hardline(), doc::hardline()]);
                } else {
                    parts.extend([it.sep.clone(), doc::line()]);
                }
            }
            a.docs.push(doc);
            a.seps.push(it.sep);
            a.printed.push(doc::concat(parts));
        }
        a
    }

    /// The `)` follows the last argument when the list breaks.
    pub fn hug_close(mut self, on: bool) -> Args {
        self.hug_close = on;
        self
    }

    /// Does any argument contain a forced line break?
    pub fn will_break(&self) -> bool {
        self.printed.iter().any(Doc::will_break)
    }

    /// Does a comment in front of the `)` stand in the list?
    pub fn has_dangling(&self) -> bool {
        !matches!(self.dangling, Doc::Nil)
    }

    /// `open`, the indented printed arguments (after `lead`), `close`; `tail` stands in front of
    /// the `)` unless it hugs the last argument.
    fn wrapped(&self, lead: Doc, tail: Doc) -> Doc {
        doc::concat(vec![
            self.open.clone(),
            doc::indent(doc::concat(vec![
                lead,
                doc::concat(self.printed.clone()),
                self.trailing.clone(),
                self.dangling.clone(),
            ])),
            if self.hug_close { Doc::Nil } else { tail },
            self.close.clone(),
        ])
    }

    fn broken_out(&self) -> Doc {
        doc::group_broken(self.wrapped(doc::line(), doc::line()))
    }

    /// The candidates with the argument `at` hugged, tried flat: the separators are spaces.
    fn candidate(&self, at: usize, expanded: &Doc, broken: bool) -> Doc {
        let mut parts = vec![self.open.clone()];
        for i in 0..self.docs.len() {
            parts.push(if i != at {
                self.docs[i].clone()
            } else if broken {
                doc::group_broken(expanded.clone())
            } else {
                expanded.clone()
            });
            if i + 1 < self.docs.len() {
                parts.extend([self.seps[i].clone(), doc::lit(" ")]);
            }
        }
        parts.push(self.close.clone());
        doc::concat(parts)
    }

    /// Every argument on a line of its own whatever the width: blank lines or comments in the
    /// list, a composition, or a hugged argument next to one that breaks.
    fn forced(&self, plan: &Plan) -> bool {
        self.any_blank
            || self.has_dangling()
            || plan.broken
            || plan.hug.is_some_and(|(_, first)| {
                let others = if first {
                    &self.printed[1..]
                } else {
                    &self.printed[..self.printed.len() - 1]
                };
                others.iter().any(Doc::will_break)
            })
    }

    /// The argument that hugs, when the list is not broken out: its index and whether it is
    /// the first.
    pub fn hug_target(&self, plan: &Plan) -> Option<(usize, bool)> {
        plan.hug.filter(|_| !self.forced(plan))
    }

    /// The document of the list. `unbroken`: the plain layout is not wrapped in a group of its
    /// own (the call it belongs to is a curried one, whose group is the outer call's).
    pub fn layout(&self, plan: &Plan, expanded: Expanded, unbroken: bool) -> Doc {
        if self.docs.is_empty() {
            return doc::concat(vec![self.open.clone(), self.close.clone()]);
        }
        if self.forced(plan) {
            return self.broken_out();
        }
        if let Some((at, _)) = plan.hug {
            let whole = matches!(expanded, Expanded::Whole);
            let expanded = match expanded {
                Expanded::Same | Expanded::Whole => self.docs[at].clone(),
                Expanded::Doc(d) => doc::shared(d),
                Expanded::Bailout => return self.broken_out(),
            };
            let (flat, hugged) = (
                self.candidate(at, &expanded, false),
                self.candidate(at, &expanded, true),
            );
            let mut states = vec![flat];
            if !whole {
                states.push(hugged);
            }
            states.push(self.broken_out());
            return doc::concat(vec![
                if self.will_break() {
                    doc::break_parent()
                } else {
                    Doc::Nil
                },
                doc::conditional_group(states),
            ]);
        }
        let contents = self.wrapped(doc::softline(), doc::softline());
        if unbroken {
            contents
        } else if self.will_break() {
            doc::group_broken(contents)
        } else {
            doc::group(contents)
        }
    }
}

/// `a(b)(c)(d)`: the inner call has more arguments than the outer one.
pub fn is_long_curried(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let g = cx.grammar;
    let Some(p) = parent(node) else {
        return false;
    };
    let count = |n: &SyntaxNode| n.children().count();
    g.has_tag(node.kind(), "call")
        && p.kind() == node.kind()
        && outer(node).parent().as_ref() == Some(&p)
        && p.first_child().as_ref() == Some(&outer(node))
        && count(&p) > 1
        && count(node) > count(&p)
}

/// A call's `( args )` that is a node of its own (`ArgumentList`): the argument that hugs by
/// `style` and the tags stays on the line of the call, every other list is `list`.
pub fn call_arguments(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    rows: &RuleFormatter,
    style: &CallStyle,
    list: &ListStyle,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let plain = || delimited_list(cx, node, list);
    if style.last == HugLast::Never && !style.first && !style.body {
        return plain();
    }
    let c = container(cx, node)?;
    let exprs: Vec<SyntaxNode> = c.slots.iter().map(|s| expression_of(cx, &s.node)).collect();
    // Comments next to the delimiters, the separators or the arguments are the list's business.
    let clean = node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()))
        .all(|t| !comments::token_has_comment(g, &t))
        && c.slots.iter().all(|s| {
            cx.first_token(&s.node)
                .is_some_and(|t| comments::leading_comments(g, &t).is_empty())
        });
    if !clean {
        return plain();
    }
    // (an expression body wins over the last-argument hug)
    if let Some(d) = body_hug(cx, rows, &c, &exprs, style)? {
        return Ok(d);
    }
    let plan = plan(cx, rows.calls(g), node, &exprs, style);
    if plan.hug.is_none() {
        return plain();
    }
    let mut items = Vec::with_capacity(exprs.len());
    for (i, s) in c.slots.iter().enumerate() {
        items.push(Item {
            doc: cx.format(&s.node)?,
            sep: if i + 1 < exprs.len() {
                doc::lit(",")
            } else {
                Doc::Nil
            },
            blank_after: false,
        });
    }
    let args = Args::new(
        cx.token(&c.open),
        cx.token(&c.close),
        items,
        Doc::Nil,
        Doc::Nil,
    )
    .hug_close(list.hug_close);
    let function = plan
        .hug
        .is_some_and(|(at, _)| g.has_tag(exprs[at].kind(), "huggable-function"));
    let expanded = if function {
        Expanded::Whole
    } else {
        Expanded::Same
    };
    Ok(args.layout(&plan, expanded, false))
}

/// A lambda with an expression body that is the only argument: the head stays on the call's
/// line and the body hangs below it when the call does not fit.
fn body_hug(
    cx: &FormatCtx<'_>,
    rows: &RuleFormatter,
    c: &super::Container,
    exprs: &[SyntaxNode],
    style: &CallStyle,
) -> Result<Option<Doc>, FormatError> {
    let g = cx.grammar;
    let [lambda] = exprs else { return Ok(None) };
    if !style.body || !g.has_tag(lambda.kind(), "huggable-function") {
        return Ok(None);
    }
    if style.last == HugLast::SoleBare && !has_bare_parameters(cx, lambda) {
        return Ok(None);
    }
    let [slot] = &c.slots[..] else {
        return Ok(None);
    };
    // (the lambda is the whole argument)
    if slot.node.children().count() != 1 && slot.node != *lambda {
        return Ok(None);
    }
    let els = seq::children(cx, lambda);
    let Some((core_syntax::SyntaxElement::Node(body), head)) = els.split_last() else {
        return Ok(None);
    };
    if g.has_tag(body.kind(), "block") {
        return Ok(None);
    }
    let head = doc::shared(seq::plain(cx, rows.space(), head, |e| seq::el(cx, e))?);
    let body = doc::shared(cx.format(body)?);
    let (open, close) = (cx.token(&c.open), cx.token(&c.close));
    let wrap = |inner: Doc, tail: Doc| doc::concat(vec![open.clone(), inner, tail, close.clone()]);
    let inline = doc::concat(vec![head.clone(), doc::lit(" "), body.clone()]);
    let hung = doc::indent(doc::concat(vec![doc::hardline(), body.clone()]));
    let wide = doc::indent(doc::concat(vec![doc::softline(), inline.clone()]));
    Ok(Some(doc::conditional_group(vec![
        wrap(inline, Doc::Nil),
        wrap(doc::concat(vec![head, hung]), doc::hardline()),
        doc::group(wrap(wide, doc::softline())),
    ])))
}
