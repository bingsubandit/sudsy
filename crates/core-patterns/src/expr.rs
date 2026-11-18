//! The `rule:` of a pattern rule file: atoms that all must hold for a node (`pattern`, `kind`,
//! `regex`, and the relations `inside`, `not-inside`, `has`, `not-has`) and the combinators
//! `all`, `any` and `not`.

use crate::matcher::{Capture, Captures, Pattern, SnippetParser};
use crate::metavar::Meta;
use core_rules::tree::TraversalCache;
use core_rules::{Check, Opt, OptionDecl, Re, RuleOptions};
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode};
use serde::Deserialize;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// One name or a list of them.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Names {
    One(String),
    Many(Vec<String>),
}

impl Names {
    pub fn list(&self) -> Vec<String> {
        match self {
            Names::One(n) => vec![n.clone()],
            Names::Many(l) => l.clone(),
        }
    }
}

/// The rule as written in the file.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub pattern: Option<String>,
    pub kind: Option<String>,
    pub regex: Option<String>,
    /// The file path (with `/`) matches this regex.
    pub path: Option<String>,
    /// The node is the Nth (from 1) of the children of its parent that have its kind.
    pub nth: Option<usize>,
    pub inside: Option<Box<Spec>>,
    #[serde(rename = "not-inside")]
    pub not_inside: Option<Box<Spec>>,
    pub has: Option<Box<Spec>>,
    #[serde(rename = "not-has")]
    pub not_has: Option<Box<Spec>>,
    /// The direct parent / a direct child holds the rule (`inside`/`has` look at every level).
    pub parent: Option<Box<Spec>>,
    pub child: Option<Box<Spec>>,
    /// The bool option of this name is true.
    pub option: Option<String>,
    /// The text of the node (a token: its text) is an item of this `list` option.
    #[serde(rename = "in-option")]
    pub in_option: Option<String>,
    /// A `#catch` whose body raises the caught error again: a `#throw` with no `value`, or a `value`
    /// or `cause` that is the catch's `binding` (nested `try`, functions and classes are not read).
    pub rethrows: Option<bool>,
    /// The text of the node matches the `regex` option of this name.
    #[serde(rename = "regex-option")]
    pub regex_option: Option<String>,
    pub all: Option<Vec<Spec>>,
    pub any: Option<Vec<Spec>>,
    pub not: Option<Box<Spec>>,
    /// The node has one of these roles (tags of `core_syntax::roles`); a rule written on roles runs
    /// on every grammar that has them.
    pub role: Option<Names>,
    /// The `operator` field of the node is an operator of one of these kinds
    /// (`Grammar::operator_kind`: `equality`, `comparison`, ...).
    pub operator: Option<Names>,
    /// Each named role field of the node holds this rule (an element: a token matches by
    /// `regex`, `role`, `any`, `all`, `not`).
    pub fields: Option<BTreeMap<String, Spec>>,
    /// Some child in each named role field holds this rule (`fields` reads the first).
    pub items: Option<BTreeMap<String, Spec>>,
    /// The named role field holds exactly one child.
    pub single: Option<String>,
    /// The two named fields have the same tokens.
    pub same: Option<Vec<String>>,
    /// The node holds no node and no comment before its closing bracket.
    pub empty: Option<bool>,
    /// The node, looked through parentheses and single-child wrappers, holds this rule.
    pub inner: Option<Box<Spec>>,
}

/// What an expression is evaluated against.
pub struct Ctx<'a> {
    pub grammar: &'a Grammar,
    pub path: &'a Path,
    pub text: &'a str,
    pub options: &'a RuleOptions,
    /// Matching steps left for this evaluation (see [`STEP_BUDGET`]); 0 means it ran out and
    /// the result is not to be trusted.
    pub budget: Cell<u32>,
    /// Shared with the other rules run on the same node.
    pub ranges: &'a TraversalCache,
}

impl Ctx<'_> {
    /// Spends one step; false when none are left.
    fn spend(&self) -> bool {
        let left = self.budget.get().saturating_sub(1);
        self.budget.set(left);
        left > 0
    }
}

/// Steps one rule may spend on one node (its `inside`/`has` walks included): about 10 ms.
pub const STEP_BUDGET: u32 = 1_000_000;

#[derive(Debug)]
pub enum Expr {
    Pattern(Pattern),
    Kind(SyntaxKind),
    /// With the names of its capture groups.
    Regex(Re, Vec<String>),
    Path(Re),
    Nth(usize),
    Inside(Box<Expr>),
    Has(Box<Expr>),
    Parent(Box<Expr>),
    Child(Box<Expr>),
    Option(String),
    InOption(String),
    Rethrows,
    RegexOption(String, RegexCache),
    All(Vec<Expr>),
    Any(Vec<Expr>),
    Not(Box<Expr>),
    Role(Vec<String>, Vec<SyntaxKind>),
    Operator(Vec<String>),
    Fields(Vec<(String, Expr)>),
    Items(Vec<(String, Expr)>),
    Single(String),
    Same(String, String),
    Empty,
    Inner(Box<Expr>),
}

/// Regexes compiled once for each source text they take.
#[derive(Default)]
pub struct RegexCache(Mutex<HashMap<String, Option<Arc<Re>>>>);

impl std::fmt::Debug for RegexCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RegexCache")
    }
}

impl RegexCache {
    pub fn is_match(&self, source: &str, text: &str) -> bool {
        let re = {
            let mut cache = self.0.lock().unwrap();
            match cache.get(source) {
                Some(re) => re.clone(),
                None => {
                    let re = Re::new(source).ok().map(Arc::new);
                    cache.insert(source.to_string(), re.clone());
                    re
                }
            }
        };
        re.is_some_and(|re| re.is_match(text))
    }
}

/// The node under parentheses and wrappers that hold nothing else (a Kotlin `Condition`).
pub(crate) fn unwrap(node: &SyntaxNode, g: &Grammar) -> SyntaxNode {
    let mut n = node.clone();
    loop {
        let mut kids = n.children();
        let (Some(only), None) = (kids.next(), kids.next()) else {
            return n;
        };
        let plain = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .all(|t| g.is_trivia(t.kind()) || matches!(t.text(), "(" | ")"));
        if !plain {
            return n;
        }
        n = only;
    }
}

/// No node inside and no comment before the last token (a closing bracket).
fn is_empty(node: &SyntaxNode, g: &Grammar) -> bool {
    if node.children().any(|c| !g.has_tag(c.kind(), "trivia")) {
        return false;
    }
    let end = node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()))
        .last()
        .map(|t| t.text_range().start());
    !node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| {
            g.has_tag(t.kind(), "comment") && end.is_none_or(|end| t.text_range().start() < end)
        })
}

/// The tokens of an element that are not trivia, read lazily.
fn words<'a>(
    e: &SyntaxElement,
    g: &'a Grammar,
) -> impl Iterator<Item = core_syntax::SyntaxToken> + use<'a> {
    let all: Box<dyn Iterator<Item = SyntaxElement>> = match e {
        SyntaxElement::Token(t) => Box::new(std::iter::once(SyntaxElement::Token(t.clone()))),
        SyntaxElement::Node(n) => Box::new(n.descendants_with_tokens()),
    };
    all.filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()) && !t.text().is_empty())
}

/// Do both elements hold the same tokens (and at least one)? Stops at the first difference.
fn same_words(a: &SyntaxElement, b: &SyntaxElement, g: &Grammar) -> bool {
    let (mut a, mut b) = (words(a, g), words(b, g));
    let mut any = false;
    loop {
        match (a.next(), b.next()) {
            (None, None) => return any,
            (Some(x), Some(y)) if x.text() == y.text() => any = true,
            _ => return false,
        }
    }
}

impl Expr {
    pub fn compile(
        spec: &Spec,
        parser: &dyn SnippetParser,
        options: &[OptionDecl],
    ) -> Result<Expr, String> {
        let mut all = Vec::new();
        let sub = |s: &Spec| Expr::compile(s, parser, options);
        if let Some(text) = &spec.pattern {
            all.push(Expr::Pattern(Pattern::compile(text, parser)?));
        }
        if let Some(name) = &spec.kind {
            let kind = parser
                .grammar()
                .kind_by_name(name)
                .filter(|k| !k.is_token())
                .ok_or_else(|| format!("`{name}` is not a node kind of the language"))?;
            all.push(Expr::Kind(kind));
        }
        if spec.regex.as_deref() == Some("") {
            // Every text matches: no regex to build (`fields: { x: { regex: '' } }` asks only that
            // the slot is there).
            all.push(Expr::All(Vec::new()));
        } else if let Some(text) = &spec.regex {
            let re = Re::new(text).map_err(|e| format!("regex `{text}`: {e}"))?;
            let groups = re.groups();
            all.push(Expr::Regex(re, groups));
        }
        if let Some(text) = &spec.path {
            all.push(Expr::Path(
                Re::new(text).map_err(|e| format!("path regex `{text}`: {e}"))?,
            ));
        }
        if let Some(n) = spec.nth {
            if n == 0 {
                return Err("nth counts from 1".into());
            }
            all.push(Expr::Nth(n));
        }
        if let Some(s) = &spec.inside {
            all.push(Expr::Inside(Box::new(sub(s)?)));
        }
        if let Some(s) = &spec.not_inside {
            all.push(Expr::Not(Box::new(Expr::Inside(Box::new(sub(s)?)))));
        }
        if let Some(s) = &spec.has {
            all.push(Expr::Has(Box::new(sub(s)?)));
        }
        if let Some(s) = &spec.not_has {
            all.push(Expr::Not(Box::new(Expr::Has(Box::new(sub(s)?)))));
        }
        if let Some(s) = &spec.parent {
            all.push(Expr::Parent(Box::new(sub(s)?)));
        }
        if let Some(s) = &spec.child {
            all.push(Expr::Child(Box::new(sub(s)?)));
        }
        if let Some(name) = &spec.option {
            let bool_option = |o: &&OptionDecl| o.name == name && matches!(o.default, Opt::Bool(_));
            if !options.iter().any(|o| bool_option(&o)) {
                return Err(format!("option `{name}` is not a declared `bool` option"));
            }
            all.push(Expr::Option(name.clone()));
        }
        if let Some(name) = &spec.in_option {
            if !options
                .iter()
                .any(|o| o.name == name && matches!(o.default, Opt::List(_)))
            {
                return Err(format!("option `{name}` is not a declared `list` option"));
            }
            all.push(Expr::InOption(name.clone()));
        }
        if spec.rethrows == Some(true) {
            all.push(Expr::Rethrows);
        }
        if let Some(name) = &spec.regex_option {
            if !options
                .iter()
                .any(|o| o.name == name && o.check == Check::Regex)
            {
                return Err(format!("option `{name}` is not a declared `regex` option"));
            }
            all.push(Expr::RegexOption(name.clone(), RegexCache::default()));
        }
        if let Some(list) = &spec.all {
            all.push(Expr::All(list.iter().map(sub).collect::<Result<_, _>>()?));
        }
        if let Some(list) = &spec.any {
            all.push(Expr::Any(list.iter().map(sub).collect::<Result<_, _>>()?));
        }
        if let Some(s) = &spec.not {
            all.push(Expr::Not(Box::new(sub(s)?)));
        }
        if let Some(names) = &spec.role {
            let names = names.list();
            let mut kinds = Vec::new();
            for n in &names {
                if !core_syntax::roles::is_tag(n) {
                    return Err(format!("`{n}` is not a role"));
                }
                kinds.extend_from_slice(parser.grammar().nodes_with_tag(n));
            }
            kinds.sort();
            kinds.dedup();
            all.push(Expr::Role(names, kinds));
        }
        if let Some(names) = &spec.operator {
            all.push(Expr::Operator(names.list()));
        }
        if let Some(fields) = &spec.fields {
            let list = fields
                .iter()
                .map(|(k, s)| Ok((k.clone(), sub(s)?)))
                .collect::<Result<_, String>>()?;
            all.push(Expr::Fields(list));
        }
        if let Some(fields) = &spec.items {
            let list = fields
                .iter()
                .map(|(k, s)| Ok((k.clone(), sub(s)?)))
                .collect::<Result<_, String>>()?;
            all.push(Expr::Items(list));
        }
        if let Some(field) = &spec.single {
            all.push(Expr::Single(field.clone()));
        }
        if let Some(pair) = &spec.same {
            let [a, b] = pair.as_slice() else {
                return Err("`same` names two fields".into());
            };
            all.push(Expr::Same(a.clone(), b.clone()));
        }
        if spec.empty == Some(true) {
            all.push(Expr::Empty);
        }
        if let Some(s) = &spec.inner {
            all.push(Expr::Inner(Box::new(sub(s)?)));
        }
        match all.len() {
            0 => Err(
                "a rule needs at least one of pattern, kind, regex, path, nth, inside, not-inside, \
                      has, not-has, parent, child, option, in-option, all, any, not, role, operator, fields, \
                      items, single, same, empty, inner, regex-option, rethrows"
                    .into(),
            ),
            1 => Ok(all.remove(0)),
            _ => Ok(Expr::All(all)),
        }
    }

    /// The captures when the expression holds for `node`.
    pub fn matches(&self, node: &SyntaxNode, cx: &Ctx<'_>) -> Option<Captures> {
        match self {
            Expr::Pattern(p) => p.matches(node, cx.grammar, &cx.budget),
            Expr::Kind(k) => (node.kind() == *k).then(Captures::new),
            Expr::Regex(re, groups) => {
                let range = cx.ranges.significant_range(node, cx.grammar);
                let text = &cx.text[range];
                let ascii = cx.ranges.ascii(cx.text);
                if groups.is_empty() {
                    return re.is_match_in(text, ascii).then(Captures::new);
                }
                let spans = re.groups_in(text, ascii)?;
                Some(Capture::groups(groups, spans, range.start()).collect())
            }
            Expr::Path(re) => {
                let path = cx.path.to_string_lossy().replace('\\', "/");
                re.is_match(&path).then(Captures::new)
            }
            Expr::Nth(n) => {
                let mut same = node
                    .parent()?
                    .children()
                    .filter(|c| c.kind() == node.kind());
                (same.nth(n - 1).as_ref() == Some(node)).then(Captures::new)
            }
            Expr::Inside(e) => node
                .ancestors()
                .skip(1)
                .find_map(|a| if cx.spend() { e.matches(&a, cx) } else { None }),
            Expr::Has(e) => node
                .descendants()
                .skip(1)
                .any(|d| cx.spend() && e.matches(&d, cx).is_some())
                .then(Captures::new),
            Expr::Parent(e) => e.matches(&node.parent()?, cx),
            Expr::Child(e) => node
                .children()
                .any(|c| cx.spend() && e.matches(&c, cx).is_some())
                .then(Captures::new),
            Expr::Option(name) => cx.options.bool(name).then(Captures::new),
            Expr::InOption(name) => {
                let range = cx.ranges.significant_range(node, cx.grammar);
                cx.options
                    .list_has(name, &cx.text[range])
                    .then(Captures::new)
            }
            Expr::Rethrows => {
                let g = cx.grammar;
                let text = |e: SyntaxElement| match e {
                    SyntaxElement::Token(t) => t.text().to_string(),
                    SyntaxElement::Node(n) => {
                        cx.text[cx.ranges.significant_range(&n, g)].to_string()
                    }
                };
                let bound = g.slot(node, "binding").map(text);
                let mut stack: Vec<SyntaxNode> = node.children().collect();
                while let Some(n) = stack.pop() {
                    if g.has_tag(n.kind(), "throw") {
                        let rethrown = match g.slot(&n, "value") {
                            None => true,
                            Some(v) => bound.as_ref().is_some_and(|b| {
                                text(v) == *b || g.slot(&n, "cause").is_some_and(|c| text(c) == *b)
                            }),
                        };
                        if rethrown {
                            return Some(Captures::new());
                        }
                    }
                    if !["try", "function", "type-declaration"]
                        .iter()
                        .any(|r| g.has_tag(n.kind(), r))
                    {
                        stack.extend(n.children());
                    }
                }
                None
            }
            Expr::RegexOption(name, cache) => {
                let range = cx.ranges.significant_range(node, cx.grammar);
                cache
                    .is_match(cx.options.string(name), &cx.text[range])
                    .then(Captures::new)
            }
            Expr::All(list) => {
                let mut captures = Captures::new();
                for e in list {
                    for (name, capture) in e.matches(node, cx)? {
                        captures.entry(name).or_insert(capture);
                    }
                }
                Some(captures)
            }
            Expr::Any(list) => list.iter().find_map(|e| e.matches(node, cx)),
            Expr::Not(e) => e.matches(node, cx).is_none().then(Captures::new),
            Expr::Role(_, kinds) => kinds.contains(&node.kind()).then(Captures::new),
            Expr::Operator(kinds) => {
                let token = match cx.grammar.slot(node, "operator")? {
                    SyntaxElement::Token(t) => t,
                    SyntaxElement::Node(n) => n.first_token()?,
                };
                let kind = cx.grammar.operator_kind(token.text())?;
                kinds.iter().any(|k| k == kind).then(Captures::new)
            }
            Expr::Fields(list) => {
                for (label, e) in list {
                    let slot = cx.grammar.slot(node, label)?;
                    e.matches_element(&slot, cx)?;
                }
                Some(Captures::new())
            }
            Expr::Items(list) => {
                for (label, e) in list {
                    cx.grammar
                        .slots(node, label)
                        .find(|item| e.matches_element(item, cx).is_some())?;
                }
                Some(Captures::new())
            }
            Expr::Single(field) => (cx.grammar.slots(node, field).count() == 1).then(Captures::new),
            Expr::Same(a, b) => {
                let a = cx.grammar.slot(node, a)?;
                let b = cx.grammar.slot(node, b)?;
                // Two sides that start with different characters (but for a comment) are not the
                // same tokens: no need to walk them.
                let first = |e: &SyntaxElement| cx.text[e.text_range()].trim_start().bytes().next();
                if let (Some(x), Some(y)) = (first(&a), first(&b))
                    && x != y
                    && !matches!(x, b'#' | b'/')
                    && !matches!(y, b'#' | b'/')
                {
                    return None;
                }
                same_words(&a, &b, cx.grammar).then(Captures::new)
            }
            Expr::Empty => is_empty(node, cx.grammar).then(Captures::new),
            Expr::Inner(e) => e.matches(&unwrap(node, cx.grammar), cx),
        }
    }

    /// Like [`Expr::matches`] for a child that may be a token: a token satisfies `regex` (its
    /// text), `role` (a token tagged with it) and the combinators, nothing else.
    pub fn matches_element(&self, e: &SyntaxElement, cx: &Ctx<'_>) -> Option<Captures> {
        let SyntaxElement::Token(t) = e else {
            return self.matches(e.as_node()?, cx);
        };
        match self {
            Expr::Regex(re, _) => re.is_match(t.text()).then(Captures::new),
            Expr::InOption(name) => cx.options.list_has(name, t.text()).then(Captures::new),
            Expr::Role(names, _) => names
                .iter()
                .any(|n| cx.grammar.has_tag(t.kind(), n))
                .then(Captures::new),
            Expr::All(list) => list.iter().try_fold(Captures::new(), |mut c, x| {
                c.extend(x.matches_element(e, cx)?);
                Some(c)
            }),
            Expr::Any(list) => list.iter().find_map(|x| x.matches_element(e, cx)),
            Expr::Not(x) => x.matches_element(e, cx).is_none().then(Captures::new),
            _ => None,
        }
    }

    /// Can evaluating this cost more than linear in the node's size: a `$$$` run (all splits are
    /// tried) or an `inside`/`has` walk?
    pub fn superlinear(&self) -> bool {
        match self {
            Expr::Pattern(p) => p.metas().iter().any(|m| m.multi),
            Expr::Inside(_) | Expr::Has(_) => true,
            Expr::All(list) | Expr::Any(list) => list.iter().any(Expr::superlinear),
            Expr::Not(e) | Expr::Parent(e) | Expr::Child(e) | Expr::Inner(e) => e.superlinear(),
            Expr::Fields(list) | Expr::Items(list) => list.iter().any(|(_, e)| e.superlinear()),
            Expr::Kind(_)
            | Expr::Regex(..)
            | Expr::Path(_)
            | Expr::Nth(_)
            | Expr::Option(_)
            | Expr::InOption(_)
            | Expr::RegexOption(..)
            | Expr::Rethrows
            | Expr::Role(..)
            | Expr::Operator(_)
            | Expr::Same(..)
            | Expr::Single(_)
            | Expr::Empty => false,
        }
    }

    /// The node kinds a match can start at, when the expression pins them down (the engine then
    /// calls the rule for those kinds only).
    pub fn kinds(&self) -> Option<Vec<SyntaxKind>> {
        match self {
            Expr::Pattern(p) => Some(vec![p.kind()]),
            Expr::Kind(k) => Some(vec![*k]),
            Expr::Role(_, kinds) => Some(kinds.clone()),
            Expr::All(list) => list.iter().find_map(Expr::kinds),
            Expr::Any(list) => {
                let mut kinds = Vec::new();
                for e in list {
                    kinds.extend(e.kinds()?);
                }
                Some(kinds)
            }
            _ => None,
        }
    }

    /// Every metavariable a match can capture: those of the patterns outside `has`, `child` and
    /// `not`, whose captures are dropped, and the groups of a `regex` (`$1`).
    pub fn metas(&self, out: &mut Vec<Meta>) {
        match self {
            Expr::Pattern(p) => out.extend(p.metas().iter().cloned()),
            Expr::Regex(_, groups) => {
                out.extend(groups.iter().filter(|n| !n.is_empty()).map(|name| Meta {
                    multi: false,
                    name: name.clone(),
                    transform: None,
                }))
            }
            Expr::All(list) | Expr::Any(list) => list.iter().for_each(|e| e.metas(out)),
            Expr::Inside(e) | Expr::Parent(e) => e.metas(out),
            Expr::Kind(_)
            | Expr::Path(_)
            | Expr::Nth(_)
            | Expr::Option(_)
            | Expr::InOption(_)
            | Expr::RegexOption(..)
            | Expr::Rethrows
            | Expr::Has(_)
            | Expr::Child(_)
            | Expr::Not(_)
            | Expr::Role(..)
            | Expr::Operator(_)
            | Expr::Fields(_)
            | Expr::Items(_)
            | Expr::Single(_)
            | Expr::Same(..)
            | Expr::Empty
            | Expr::Inner(_) => {}
        }
    }
}
