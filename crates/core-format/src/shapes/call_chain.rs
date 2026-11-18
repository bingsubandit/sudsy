//! Member chains (`a.b().c()[0]`) read through roles: a node with the role `#call`, `#member-access`,
//! `#index` or a postfix `#unary` (`a!`) is a link, its receiver is the field `object` (or the
//! `callee` when that has no `object`, or the `operand`), and what else it holds is the link's
//! text. The layout comes from a [`ChainPolicy`], whose values are options of the language
//! (`member_chain_*`); what only the language can print or judge comes through [`ChainHooks`].

use super::chain::{MemberChain, member_chain as print_chain};
use crate::comments::{self, CommentKind};
use crate::doc::{self, Doc};
use crate::{Directive, FormatCtx, FormatError};
use core_config::{OptionValue, ResolvedOptions};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};
use std::cell::RefCell;

pub const MIN_CALLS: &str = "member_chain_min_calls";
pub const KEEP_ACCESSES: &str = "member_chain_keep_accesses";
pub const HEAD_CALLS: &str = "member_chain_head_calls";
pub const BREAK_ON_BLOCK: &str = "member_chain_break_on_block";
pub const SHORT_HEAD: &str = "member_chain_short_head";
pub const INDENT: &str = "member_chain_indent";
pub const STYLE: &str = "member_chain_style";
pub const FORCE_LINKS: &str = "member_chain_force_links";
#[path = "call_chain_wrapped.rs"]
mod wrapped;
/// The label of the document of a chain printed in the `grouped` style.
pub const CHAIN_LABEL: &str = "member-chain";

/// The algorithms of reference tools: `fluent` (C#, Java) groups the links at the dots;
/// `grouped` (JS) cuts them into a first group and call groups; `wrapped` (Kotlin) puts every dot
/// after the accesses of the head on a line of its own.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Fluent,
    Grouped,
    Wrapped,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeepAccesses {
    Never,
    Head,
    Always,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BreakOnBlock {
    Never,
    Inner,
    Any,
}

/// The values where reference tools differ; a language declares each as an option.
pub struct ChainPolicy {
    pub style: Style,
    /// A chain with fewer calls than this is never broken at the dots.
    pub min_calls: usize,
    /// The accesses before a call stay with it (`.A.B()` is one group): `always`, or in the first
    /// group only (`head`).
    pub keep_accesses: KeepAccesses,
    /// Calls that stay on the line of the head when they are among the first two groups
    /// (`.stream()`).
    pub head_calls: Vec<String>,
    /// Break at the dots when a call other than the last has a block in its arguments (`inner`),
    /// or when any call has one (`any`).
    pub break_on_block: BreakOnBlock,
    /// A head name of at most this many characters stays on the line of the first group.
    pub short_head: usize,
    /// Levels of indentation of the broken groups.
    pub indent: usize,
    /// A chain with at least this many dots always breaks (`wrapped`; 0: never).
    pub force_links: usize,
    /// A chain of at most this many characters on one line of the source with fewer dots than
    /// `force_links` is never broken (`wrapped`: a third of the line width; 0: none).
    pub flat_below: usize,
}

impl ChainPolicy {
    /// The policy of the `member_chain_*` options a language declares (the others keep a neutral
    /// value that its style does not read).
    pub fn of(options: &ResolvedOptions) -> Self {
        let text = |name: &str| match options.get(name) {
            Some(OptionValue::String(s)) => s.as_str(),
            _ => "",
        };
        let int = |name: &str, default: usize| match options.get(name) {
            Some(OptionValue::Int(n)) => usize::try_from(*n).unwrap_or(default),
            _ => default,
        };
        Self {
            style: match text(STYLE) {
                "grouped" => Style::Grouped,
                "wrapped" => Style::Wrapped,
                _ => Style::Fluent,
            },
            force_links: int(FORCE_LINKS, 0),
            flat_below: if text(STYLE) == "wrapped" {
                int("line_width", 0) / 3
            } else {
                0
            },
            min_calls: int(MIN_CALLS, 1),
            keep_accesses: match text(KEEP_ACCESSES) {
                "head" => KeepAccesses::Head,
                "always" => KeepAccesses::Always,
                _ => KeepAccesses::Never,
            },
            head_calls: match options.get(HEAD_CALLS) {
                Some(OptionValue::List(items)) => items
                    .iter()
                    .filter_map(|i| match i {
                        OptionValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            break_on_block: match text(BREAK_ON_BLOCK) {
                "inner" => BreakOnBlock::Inner,
                "any" => BreakOnBlock::Any,
                _ => BreakOnBlock::Never,
            },
            short_head: int(SHORT_HEAD, 0),
            indent: int(INDENT, 1),
        }
    }
}

/// What the language prints or judges itself; every method has a default.
pub trait ChainHooks {
    /// An element of a link (a node or a token); `in_head`: it is on the line of the head.
    fn element(&self, _e: &SyntaxElement, _in_head: bool) -> Option<Result<Doc, FormatError>> {
        None
    }
    /// A whole link of a node; the flag says that it ends the chain. The second value says that a
    /// comment or a blank line follows the link, which expands the chain.
    fn link(&self, _node: &SyntaxNode, _last: bool) -> Option<Result<(Doc, bool), FormatError>> {
        None
    }
    /// May the chain look through these parentheses (a `#transparent` node)?
    fn through(&self, _paren: &SyntaxNode) -> bool {
        false
    }
    /// Is this argument of a call simple (`grouped` style: a chain of more than two calls with a
    /// complex argument breaks)?
    fn simple_argument(&self, _arg: &SyntaxNode) -> bool {
        true
    }
    /// Is the chain a whole statement (`grouped` style: a short head name stays only then)?
    fn statement(&self, _chain: &SyntaxNode) -> bool {
        true
    }
}

/// Is the chain short, on one line and with few dots, so that no layout would change it
/// (`wrapped` style)? Its parts are then laid out one after the other, without looking at links.
pub fn is_flat(cx: &FormatCtx<'_>, node: &SyntaxNode, policy: &ChainPolicy) -> bool {
    let range = node.text_range();
    let (Some(first), Some(last)) = (cx.first_token(node), cx.last_token(node)) else {
        return false;
    };
    let code = core_syntax::TextRange::new(first.text_range().start(), last.text_range().end());
    usize::from(code.len()) <= policy.flat_below
        && !cx.has_line_break(node, code)
        && range.len() > 0.into()
        && node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| matches!(t.text(), "." | "?."))
            .count()
            < policy.force_links
}

/// A language with nothing of its own.
pub struct NoHooks;

impl ChainHooks for NoHooks {}

struct Link {
    node: SyntaxNode,
    elems: Vec<SyntaxElement>,
    /// Starts with `.` or `?.`.
    dotted: bool,
    call: bool,
    index: bool,
    /// An index with a literal in it (`a[0]`).
    literal: bool,
    /// A postfix operator (`a!`).
    postfix: bool,
    /// A `?` that only announces the access after it.
    marker: bool,
}

struct Walk<'a> {
    cx: &'a FormatCtx<'a>,
    hooks: &'a dyn ChainHooks,
    /// A call whose callee is no link is the head (`grouped`), else its callee is.
    call_heads: bool,
    links: RefCell<Vec<Link>>,
    /// The `(` of parentheses the chain looked through: their comments go in front of the head.
    dropped: RefCell<Vec<SyntaxToken>>,
}

impl<'a> Walk<'a> {
    fn tag(&self, n: &SyntaxNode, tag: &str) -> bool {
        self.cx.grammar.has_tag(n.kind(), tag)
    }

    fn significant(&self, n: &SyntaxNode) -> Vec<SyntaxElement> {
        n.children_with_tokens()
            .filter(|e| !self.cx.grammar.is_trivia(e.kind()))
            .collect()
    }

    /// `new Foo(...)` is a call by role but starts a chain, it is not a link of one.
    fn is_new(&self, n: Option<&SyntaxNode>) -> bool {
        n.is_some_and(|n| {
            n.children_with_tokens()
                .any(|e| e.as_token().is_some_and(|t| t.text() == "new"))
        })
    }

    fn is_link(&self, n: &SyntaxNode) -> bool {
        (self.tag(n, "call") && !self.is_new(Some(n)))
            || self.tag(n, "member-access")
            || self.tag(n, "index")
            || self.tag(n, "unary")
    }

    /// The node itself, or what is inside its parentheses when the language lets the chain look
    /// through them.
    fn unwrap(&self, n: &SyntaxNode, record: bool) -> SyntaxNode {
        let mut n = n.clone();
        while self.tag(&n, "transparent") && self.hooks.through(&n) {
            let Some(inner) = n.first_child() else { break };
            if let Some(open) = n.first_token().filter(|_| record) {
                self.dropped.borrow_mut().push(open);
            }
            n = inner;
        }
        n
    }

    fn descend(&self, e: Option<SyntaxElement>) -> Option<SyntaxElement> {
        match e {
            Some(SyntaxElement::Node(n)) => {
                let inner = self.unwrap(&n, true);
                if self.is_link(&inner) {
                    self.walk(&inner)
                } else {
                    Some(SyntaxElement::Node(inner))
                }
            }
            other => other,
        }
    }

    /// Collects the links of the chain `node` ends with, in source order, and returns the head
    /// (`None` when the chain starts with `.x` or `[i]` after a `?`).
    fn walk(&self, node: &SyntaxNode) -> Option<SyntaxElement> {
        crate::deep(|| self.walk_in(node))
    }

    fn walk_in(&self, node: &SyntaxNode) -> Option<SyntaxElement> {
        let g = self.cx.grammar;
        let call = self.tag(node, "call");
        let all = self.significant(node);
        let field = |name: &str| g.slot_in(node.kind(), &all, name);
        let receiver = match field("object") {
            Some(o) => Some(o),
            None if call => field("callee"),
            None if self.tag(node, "unary") => field("operand"),
            None => None,
        };
        if call && self.call_heads {
            let plain = match &receiver {
                Some(SyntaxElement::Node(n)) => !self.is_link(&self.unwrap(n, false)),
                _ => true,
            };
            if plain {
                return Some(SyntaxElement::Node(node.clone()));
            }
        }
        let mut elems = all.clone();
        elems.retain(|e| Some(e) != receiver.as_ref());
        // A member whose name is itself a chain (`a?.b.c()`: the `?` and then the chain) continues it.
        // A member that is a call on a plain name (`a.b(c)` as `a`, `.`, `b(c)`) is the call link itself.
        let selector = match field("member") {
            Some(SyntaxElement::Node(m)) if !call && self.tag(&m, "call") => {
                let callee = self.cx.grammar.slot(&m, "callee");
                !callee
                    .and_then(|c| c.into_node())
                    .is_some_and(|c| self.is_link(&c))
            }
            _ => false,
        };
        let nested = match field("member") {
            Some(SyntaxElement::Node(m)) if !call && !selector && self.is_link(&m) => Some(m),
            _ => None,
        };
        if let Some(m) = &nested {
            elems.retain(|e| e.as_node() != Some(m));
        }
        let head = self.descend(receiver);
        let dotted = matches!(elems.first(), Some(SyntaxElement::Token(t)) if matches!(t.text(), "." | "?."));
        let index = self.tag(node, "index");
        let literal = index
            && field("arguments")
                .and_then(|a| a.into_node())
                .is_some_and(|a| self.tag(&a, "literal"));
        self.links.borrow_mut().push(Link {
            node: node.clone(),
            elems,
            dotted,
            call: call || selector,
            index,
            literal,
            postfix: self.tag(node, "unary"),
            marker: nested.is_some(),
        });
        if let Some(m) = nested {
            self.walk(&m);
        }
        head
    }

    fn element(&self, e: &SyntaxElement, in_head: bool) -> Result<Doc, FormatError> {
        if let Some(d) = self.hooks.element(e, in_head) {
            return d;
        }
        match e {
            SyntaxElement::Node(n) => self.cx.format(n),
            SyntaxElement::Token(t) => Ok(self.cx.token(t)),
        }
    }

    /// The text of a link, and whether a comment or a blank line follows it.
    fn link_doc(&self, link: &Link, last: bool, in_head: bool) -> Result<(Doc, bool), FormatError> {
        if let Some(d) = self.hooks.link(&link.node, last) {
            return d;
        }
        let parts = link
            .elems
            .iter()
            .map(|e| self.element(e, in_head))
            .collect::<Result<Vec<_>, _>>()?;
        let commented = link.elems.iter().filter_map(|e| e.as_token()).any(|t| {
            !comments::leading_comments(self.cx.grammar, t).is_empty()
                || !comments::trailing_comments(self.cx.grammar, t).is_empty()
        });
        Ok((doc::concat(parts), commented))
    }
}

/// Does a line comment sit between the parts of the chain? Then every call goes on its own
/// line, or two comments would end up on one line. One that ends the chain does not count: the
/// caller prints it after what follows the chain (a list separator).
fn has_line_comment(cx: &FormatCtx<'_>, links: &[Link]) -> bool {
    let g = cx.grammar;
    let is_line = |c: &comments::Comment| c.kind != CommentKind::Block;
    let line = |t: &SyntaxToken, end: bool| {
        let trailing = comments::trailing_comments(g, t);
        (!end && trailing.iter().any(is_line))
            || comments::leading_comments(g, t).iter().any(is_line)
    };
    let last = links.len().saturating_sub(1);
    links.iter().enumerate().any(|(i, l)| {
        let n = l.elems.len();
        l.elems.iter().enumerate().any(|(j, e)| {
            let end = i == last && j + 1 == n;
            match e {
                SyntaxElement::Token(t) => line(t, end),
                SyntaxElement::Node(x) => cx.last_token(x).is_some_and(|t| line(&t, end)),
            }
        })
    })
}

/// The name a call link calls, or an access link reads.
fn link_name(cx: &FormatCtx<'_>, link: &Link) -> Option<String> {
    let name = cx
        .grammar
        .slot(&link.node, "callee")
        .or_else(|| cx.grammar.slot(&link.node, "member"))?;
    Some(text_of(cx, &name))
}

/// The text of an element without its comments and whitespace.
fn text_of(cx: &FormatCtx<'_>, e: &SyntaxElement) -> String {
    match e {
        SyntaxElement::Node(n) => n
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !cx.grammar.is_trivia(t.kind()))
            .map(|t| t.text().to_string())
            .collect(),
        SyntaxElement::Token(t) => t.text().to_string(),
    }
}

fn is_word(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Is the head short enough, or a name that reads as a type, to stay on the first line with the
/// first call (`this.Foo()`, `Task.Run()`, `x.Foo()`)? A capitalized head that is itself called
/// (`Create().Foo()`) is a call, not a type name.
fn merges_first_group(
    cx: &FormatCtx<'_>,
    head: &SyntaxElement,
    called: bool,
    short_head: usize,
) -> bool {
    let text = text_of(cx, head);
    if !is_word(&text) {
        return false;
    }
    let keyword = match head {
        SyntaxElement::Token(t) => cx.grammar.has_tag(t.kind(), "keyword"),
        SyntaxElement::Node(n) => n
            .first_token()
            .is_some_and(|t| cx.grammar.has_tag(t.kind(), "keyword")),
    };
    keyword
        || matches!(text.as_str(), "this" | "base" | "super")
        || (!called && text.chars().next().is_some_and(char::is_uppercase))
        || text.chars().count() <= short_head
}

/// The chain that ends with `node`: on one line if it fits, else the head and then one group per
/// line.
pub fn member_chain(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    policy: &ChainPolicy,
    hooks: &dyn ChainHooks,
) -> Result<Doc, FormatError> {
    let w = Walk {
        cx,
        hooks,
        call_heads: policy.style == Style::Grouped,
        links: RefCell::new(Vec::new()),
        dropped: RefCell::new(Vec::new()),
    };
    let head = w.walk(node);
    let links = w.links.take();
    let Some(head) = head else {
        // `.name` or `[i]` after a `?`: the part of the chain that follows the head.
        let docs = links
            .iter()
            .map(|l| w.link_doc(l, false, false).map(|d| d.0))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(doc::concat(docs));
    };
    let mut lead: Vec<Doc> = w
        .dropped
        .take()
        .iter()
        .map(|t| cx.leading_comments_doc(t))
        .collect();
    let head_call = head.as_node().is_some_and(|n| w.tag(n, "call"));
    if policy.style == Style::Wrapped {
        return wrapped::wrapped(&w, node, &head, lead, &links, policy, hooks);
    }
    let directives = cx.directives_of(node);
    let break_all = directives.has(&Directive::BreakAll);
    let force = has_line_comment(cx, &links) || break_all;
    let keep_together = directives.has(&Directive::KeepTogether);
    if policy.style == Style::Grouped {
        lead.push(w.element(&head, false)?);
        let head_doc = doc::concat(lead);
        // (a `new` at the head is no call)
        let head_call = head_call && !w.is_new(head.as_node());
        return prettier(
            &w, node, &head, head_doc, head_call, &links, policy, break_all,
        );
    }
    // A head that is a call or `new` counts as a call.
    let calls = links.iter().filter(|l| l.call).count() + usize::from(head_call);

    // The links that stay with the head (`None`) and the groups that start at a dot.
    let mut group_of: Vec<Option<usize>> = Vec::with_capacity(links.len());
    let mut group_count = 0usize;
    for (i, link) in links.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| &links[p]);
        let starts = if link.marker {
            links.get(i + 1).is_some_and(|n| n.dotted)
        } else if link.dotted {
            match prev {
                Some(p) if p.marker => false,
                Some(p)
                    if policy.keep_accesses == KeepAccesses::Always
                        || (policy.keep_accesses == KeepAccesses::Head && group_count <= 1) =>
                {
                    !(p.dotted && !p.call)
                }
                _ => true,
            }
        } else {
            false
        };
        if starts {
            group_count += 1;
        }
        group_of.push(group_count.checked_sub(1));
    }
    let called = group_of.first().is_some_and(|g| g.is_none());
    if group_count > 0 && merges_first_group(cx, &head, called, policy.short_head) {
        for g in group_of.iter_mut() {
            *g = match *g {
                Some(0) | None => None,
                Some(n) => Some(n - 1),
            };
        }
        group_count -= 1;
    }
    // A call named in `head_calls` among the first two groups pulls the groups before it along.
    let pulled = (0..group_count.min(2)).rev().find(|&g| {
        links.iter().zip(&group_of).any(|(l, x)| {
            *x == Some(g)
                && l.call
                && link_name(cx, l).is_some_and(|n| policy.head_calls.contains(&n))
        })
    });
    if let Some(last) = pulled.filter(|_| !policy.head_calls.is_empty()) {
        for g in group_of.iter_mut() {
            *g = match *g {
                Some(n) if n <= last => None,
                Some(n) => Some(n - last - 1),
                None => None,
            };
        }
        group_count -= last + 1;
    }
    let broken_head = calls >= policy.min_calls;
    lead.push(w.element(&head, broken_head)?);
    let mut head_parts = vec![doc::concat(lead)];
    let mut groups: Vec<Vec<Doc>> = vec![Vec::new(); group_count];
    for (link, g) in links.iter().zip(&group_of) {
        let d = w.link_doc(link, false, broken_head && g.is_none())?.0;
        match g {
            Some(g) => groups[*g].push(d),
            None => head_parts.push(d),
        }
    }
    // `BreakAll` / `KeepTogether` on the chain's outermost node: always / never at the dots.
    Ok(print_chain(MemberChain {
        head: head_parts,
        groups,
        calls,
        min_calls: if keep_together {
            usize::MAX
        } else {
            policy.min_calls
        },
        force_break: force,
        break_on_inner_break: policy.break_on_block != BreakOnBlock::Never,
        inner_break_anywhere: policy.break_on_block == BreakOnBlock::Any,
        indent: policy.indent,
    }))
}

// ---------------------------------------------------------------------------- prettier

/// A name that reads as a factory or a namespace: capitalized, or only `$` and `_`.
/// Does a link after the first group start on a line of its own in the source?
fn source_expanded(cx: &FormatCtx<'_>, links: &[Link]) -> bool {
    links.iter().any(|l| {
        l.dotted
            && l.elems
                .first()
                .and_then(|e| e.as_token())
                .is_some_and(|dot| {
                    let mut before = dot.prev_token();
                    while let Some(t) = &before {
                        if !cx.grammar.is_trivia(t.kind()) {
                            break;
                        }
                        before = t.prev_token();
                    }
                    before.is_some_and(|prev| {
                        cx.has_line_break(
                            &l.node,
                            TextRange::new(prev.text_range().end(), dot.text_range().start()),
                        )
                    })
                })
    })
}

fn is_factory(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        || (!name.is_empty() && name.chars().all(|c| c == '$' || c == '_'))
}

/// Prettier's `printMemberChain`: the first group is the head, the calls and indexes that follow
/// it and, when it is no call, its accesses but the last; every further group is accesses and
/// then calls. A chain of at most two (three, with a merged head) groups that has no forced
/// break stays on one line.
#[allow(clippy::too_many_arguments)]
fn prettier(
    w: &Walk<'_>,
    node: &SyntaxNode,
    head: &SyntaxElement,
    head_doc: Doc,
    head_call: bool,
    links: &[Link],
    policy: &ChainPolicy,
    break_all: bool,
) -> Result<Doc, FormatError> {
    let cx = w.cx;
    let n = links.len();
    // The text of each link and whether a comment or a blank line follows it.
    let mut docs: Vec<Doc> = Vec::with_capacity(n);
    let mut commented: Vec<bool> = Vec::with_capacity(n);
    for (i, l) in links.iter().enumerate() {
        let (d, c) = w.link_doc(l, i + 1 == n, false)?;
        docs.push(d);
        commented.push(c);
    }
    let head_commented = head_call
        && head
            .as_node()
            .and_then(|h| cx.last_token(h))
            .is_some_and(|t| comments::has_trailing_comment(cx.grammar, &t));
    // `memberish`: an access or an index.
    let memberish = |i: usize| !links[i].call && !links[i].postfix;
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < n && (links[i].call || links[i].postfix || (links[i].index && links[i].literal)) {
        cur.push(i);
        i += 1;
    }
    if !head_call {
        while i + 1 < n && memberish(i) && memberish(i + 1) {
            cur.push(i);
            i += 1;
        }
    }
    let first_group = std::mem::take(&mut cur);
    let mut seen_call = false;
    while i < n {
        if seen_call && memberish(i) {
            if links[i].index && links[i].literal && !commented[i] {
                cur.push(i);
                i += 1;
                continue;
            }
            groups.push(std::mem::take(&mut cur));
            seen_call = false;
        }
        if links[i].call {
            seen_call = true;
        }
        cur.push(i);
        i += 1;
    }
    if !cur.is_empty() {
        groups.push(cur);
    }
    // `groups[0]` is the first group.
    groups.insert(0, first_group);

    let in_statement = w.hooks.statement(node);
    let has_computed = groups
        .get(1)
        .and_then(|g| g.first())
        .is_some_and(|&i| links[i].index && links[i].literal);
    let should_not_wrap = || {
        if groups[0].is_empty() {
            let text = text_of(cx, head);
            let keyword = text == "this";
            let plain = head.as_node().is_some_and(|h| w.tag(h, "reference"));
            return keyword
                || (plain
                    && (is_factory(&text)
                        || (in_statement && text.chars().count() <= policy.short_head)
                        || has_computed));
        }
        let last = *groups[0].last().unwrap_or(&0);
        memberish(last)
            && !links[last].index
            && link_name(cx, &links[last]).is_some_and(|name| is_factory(&name) || has_computed)
    };
    let should_merge = groups.len() >= 2 && !groups[1].is_empty() && should_not_wrap();

    let print_group = |g: &Vec<usize>| doc::concat(g.iter().map(|&i| docs[i].clone()).collect());
    let printed: Vec<Doc> = groups.iter().map(print_group).collect();
    let printed: Vec<Doc> = printed
        .into_iter()
        .enumerate()
        .map(|(g, d)| {
            if g == 0 {
                doc::concat(vec![head_doc.clone(), d])
            } else {
                d
            }
        })
        .collect();
    let one_line = doc::concat(printed.clone());
    let cutoff = if should_merge { 3 } else { 2 };
    let flat: Vec<usize> = groups.iter().flatten().copied().collect();
    let any_commented = head_commented
        || flat[..flat.len().saturating_sub(1)]
            .iter()
            .any(|&i| commented[i])
        || groups
            .get(cutoff)
            .and_then(|g| g.first())
            .is_some_and(|&i| commented[i]);
    if groups.len() <= cutoff
        && !any_commented
        && !printed[cutoff.min(printed.len())..]
            .iter()
            .any(Doc::will_break)
    {
        return Ok(doc::group(one_line));
    }
    let rest_from = if should_merge { 2 } else { 1 };
    let expanded = doc::concat(vec![
        printed[0].clone(),
        if should_merge {
            printed[1].clone()
        } else {
            Doc::Nil
        },
        if groups.len() > rest_from {
            let mut rest = doc::concat(vec![
                doc::hardline(),
                doc::join(&doc::hardline(), printed[rest_from..].iter().cloned()),
            ]);
            for _ in 0..policy.indent {
                rest = doc::indent(rest);
            }
            rest
        } else {
            Doc::Nil
        },
    ]);
    if any_commented || break_all {
        return Ok(doc::group(expanded));
    }
    // A call at the head counts as one.
    let calls: Vec<&SyntaxNode> = links
        .iter()
        .filter(|l| l.call)
        .map(|l| &l.node)
        .chain(head.as_node().filter(|_| head_call))
        .collect();
    let args_of = |c: &SyntaxNode| {
        cx.grammar
            .slots(c, "arguments")
            .filter_map(|a| a.into_node())
            .collect::<Vec<_>>()
    };
    let complex_args = calls.len() > 2
        && calls
            .iter()
            .any(|c| !args_of(c).iter().all(|a| w.hooks.simple_argument(a)));
    let last_group_breaks_and_calls_have_functions = || {
        links.last().is_some_and(|l| l.call)
            && printed.last().is_some_and(Doc::will_break)
            && calls[..calls.len().saturating_sub(1)].iter().any(|c| {
                args_of(c)
                    .iter()
                    .any(|a| w.tag(&w.unwrap(a, false), "function"))
            })
    };
    // A chain of at most two calls that is expanded in the source stays so when its one-line form
    // fits only because an argument keeps a line break of the source: Prettier's own output of
    // such a chain hugs the argument at the next format and expands again at the one after that.
    // Longer chains keep Prettier's layout.
    let one_line = if calls.len() <= 2 && source_expanded(cx, &links[groups[0].len()..]) {
        one_line.unforced()
    } else {
        one_line
    };
    let result = if complex_args
        || printed[..printed.len() - 1].iter().any(Doc::will_break)
        || last_group_breaks_and_calls_have_functions()
    {
        doc::group(expanded)
    } else {
        doc::concat(vec![
            if one_line.will_break() {
                doc::break_parent()
            } else {
                Doc::Nil
            },
            doc::conditional_group(vec![one_line, expanded]),
        ])
    };
    Ok(doc::label(CHAIN_LABEL, result))
}
