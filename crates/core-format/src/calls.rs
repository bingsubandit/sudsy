//! The `[call]`, `[simple]` and `[hug]` sections of a language's rules file: what a call's
//! arguments look like for the layout that hugs, keeps on a line or breaks them
//! ([`crate::shapes::call_arguments`]).
//!
//! `[call]`: `set NAME = text|text` names a list of callee texts (a dotted name without blanks);
//! a row `CALLEE / ARGS : EFFECT` says that a call whose callee and arguments match is printed
//! with the effect. `CALLEE` is `&`-joined conditions: `callee=@SET` or `callee=a|b`,
//! `node=Kind|Kind`, `parent=Kind|Kind`. `ARGS` is a comma list of argument shapes, one per
//! argument (a trailing `+` repeats the last shape one or more times): `*`, or `Kind|#tag|...`
//! followed by `&` conditions: `block` (the function's `body` is a `#block`), `params<=N`,
//! `params=N`, `callee=@SET` and `arg=SHAPE` (a call with exactly that one argument),
//! `own-line` or `own-line=Kind` (a multi-line literal, or the last child of that kind, that starts on the line of the code before it) and
//! `comments=none` (no comment next to the node) or `comments=none-inside` (no comment next to
//! any of its tokens). Effects: `flat` (the arguments stay on the call's line), `test` (the same,
//! and the parameters of a function argument stay on its line) and `hook` (the arguments stay on
//! the line, as part of the argument list).
//!
//! `[simple]` (Prettier's "simple call argument"): `leaf`, `regex = Kind:N` (a leaf of a pattern
//! of at most N characters), `single_line` (a literal with expressions that has no line break),
//! `object`, `array`, `items` (every child simple, one level deeper), `members` (every child
//! simple), `operand` (the first child simple), `unary = Kind: op op` (the last child simple),
//! `call` (callee and arguments simple, only at the top). `[hug]`: `composition = yes` (a function composition breaks its arguments), `no_composition` (parent kinds), `short` (always short),
//! `short_binary`, `cast = Kind : TypeArgumentsKind` (short when the first child is and the last
//! has no type arguments), `not_last = Kind,Kind` (a pair of arguments the last of which does not
//! hug), `numeric` and `signed_numeric` (the items of a concise array), `concise = Kind`.

use crate::FormatCtx;
use crate::comments;
use crate::rules::{section, settings};
use core_syntax::roles::{code_text, un};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};

/// What a matching `[call]` row asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Flat,
    Test,
    Hook,
}

impl Effect {
    /// The arguments are the layout's, not a member chain's.
    pub fn is_inline(self) -> bool {
        self != Effect::Hook
    }
}

#[derive(Clone, Debug)]
enum Pick {
    Kind(SyntaxKind),
    Tag(String),
}

#[derive(Clone, Debug)]
enum Names {
    Set(usize),
    Text(Vec<String>),
}

#[derive(Clone, Debug)]
enum Cond {
    Block,
    Params(bool, usize),
    Callee(Names),
    Arg(Box<Shape>),
    OwnLine(Option<SyntaxKind>),
    Comments(bool),
}

#[derive(Clone, Debug, Default)]
struct Shape {
    any: bool,
    picks: Vec<Pick>,
    conds: Vec<Cond>,
}

#[derive(Debug)]
struct Pattern {
    node: Vec<SyntaxKind>,
    parent: Vec<SyntaxKind>,
    callee: Option<Names>,
    args: Vec<Shape>,
    repeat: bool,
    effect: Effect,
}

#[derive(Default)]
struct Simple {
    leaf: Vec<Pick>,
    regex: Vec<(SyntaxKind, usize)>,
    single_line: Vec<SyntaxKind>,
    object: Vec<SyntaxKind>,
    array: Vec<SyntaxKind>,
    items: Vec<SyntaxKind>,
    members: Vec<SyntaxKind>,
    operand: Vec<SyntaxKind>,
    unary: Vec<(SyntaxKind, Vec<String>)>,
    call: Vec<SyntaxKind>,
    short: Vec<SyntaxKind>,
    short_binary: Vec<SyntaxKind>,
    cast: Vec<(SyntaxKind, SyntaxKind)>,
    not_last: Vec<(SyntaxKind, SyntaxKind)>,
    numeric: Vec<SyntaxKind>,
    signed_numeric: Vec<SyntaxKind>,
    concise: Vec<SyntaxKind>,
    no_composition: Vec<SyntaxKind>,
    composition: bool,
}

/// The rows of `[call]`, `[simple]` and `[hug]`.
#[derive(Default)]
pub struct CallRules {
    sets: Vec<Vec<String>>,
    patterns: Vec<Pattern>,
    simple: Simple,
}

fn kind(g: &Grammar, n: &str) -> SyntaxKind {
    g.kind_by_name(n.trim())
        .unwrap_or_else(|| panic!("call rules: kind `{n}`"))
}

fn kinds(g: &Grammar, v: &str) -> Vec<SyntaxKind> {
    v.split('|').map(|n| kind(g, n)).collect()
}

fn picks(g: &Grammar, v: &str) -> Vec<Pick> {
    v.split('|')
        .map(|n| match n.trim().strip_prefix('#') {
            Some(t) => Pick::Tag(t.to_owned()),
            None => Pick::Kind(kind(g, n)),
        })
        .collect()
}

fn pairs(g: &Grammar, v: &str, sep: char) -> Vec<(SyntaxKind, SyntaxKind)> {
    v.split('|')
        .filter_map(|p| p.split_once(sep))
        .map(|(a, b)| (kind(g, a), kind(g, b)))
        .collect()
}

impl CallRules {
    pub fn parse(g: &Grammar, text: &str) -> CallRules {
        let mut rules = CallRules::default();
        let mut names: Vec<String> = Vec::new();
        for l in section(text, "call") {
            if let Some(rest) = l.strip_prefix("set ") {
                let (name, list) = rest
                    .split_once('=')
                    .unwrap_or_else(|| panic!("call: `{l}`"));
                names.push(name.trim().to_owned());
                rules
                    .sets
                    .push(list.split('|').map(|s| s.trim().to_owned()).collect());
            }
        }
        for l in section(text, "call").filter(|l| !l.starts_with("set ")) {
            rules.patterns.push(rules.pattern_row(g, &names, l));
        }
        let mut all = settings(text, "simple");
        all.extend(settings(text, "hug"));
        let get = |key: &str| all.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        let list = |key: &str| get(key).map(|v| kinds(g, v)).unwrap_or_default();
        let s = &mut rules.simple;
        s.leaf = get("leaf").map(|v| picks(g, v)).unwrap_or_default();
        s.regex = get("regex")
            .into_iter()
            .flat_map(|v| v.split('|'))
            .filter_map(|p| p.split_once(':'))
            .map(|(n, max)| (kind(g, n), max.trim().parse().expect("regex length")))
            .collect();
        s.single_line = list("single_line");
        s.object = list("object");
        s.array = list("array");
        s.items = list("items");
        s.members = list("members");
        s.operand = list("operand");
        s.unary = get("unary")
            .into_iter()
            .flat_map(|v| v.split('|'))
            .filter_map(|p| p.split_once(':'))
            .map(|(n, ops)| {
                (
                    kind(g, n),
                    ops.split_whitespace().map(str::to_owned).collect(),
                )
            })
            .collect();
        s.call = list("call");
        s.short = list("short");
        s.short_binary = list("short_binary");
        s.cast = get("cast")
            .into_iter()
            .flat_map(|v| v.split('|'))
            .filter_map(|p| p.split_once(':'))
            .map(|(n, t)| (kind(g, n), kind(g, t)))
            .collect();
        s.not_last = get("not_last")
            .map(|v| pairs(g, v, ','))
            .unwrap_or_default();
        s.numeric = list("numeric");
        s.signed_numeric = list("signed_numeric");
        s.concise = list("concise");
        s.no_composition = list("no_composition");
        s.composition = get("composition") == Some("yes");
        rules
    }

    fn names(&self, declared: &[String], v: &str) -> Names {
        match v.strip_prefix('@') {
            Some(n) => Names::Set(
                declared
                    .iter()
                    .position(|d| d == n)
                    .unwrap_or_else(|| panic!("call: set `{n}`")),
            ),
            None => Names::Text(v.split('|').map(|s| s.trim().to_owned()).collect()),
        }
    }

    fn pattern_row(&self, g: &Grammar, declared: &[String], l: &str) -> Pattern {
        let (head, effect) = l.rsplit_once(':').unwrap_or_else(|| panic!("call: `{l}`"));
        let (callee_part, args_part) = head
            .split_once(" / ")
            .unwrap_or_else(|| panic!("call: `{l}`"));
        let mut p = Pattern {
            node: Vec::new(),
            parent: Vec::new(),
            callee: None,
            args: Vec::new(),
            repeat: false,
            effect: match effect.trim() {
                "flat" => Effect::Flat,
                "test" => Effect::Test,
                "hook" => Effect::Hook,
                e => panic!("call: effect `{e}`"),
            },
        };
        for c in callee_part.split('&').filter(|c| *c != "*") {
            match c.trim().split_once('=') {
                Some(("callee", v)) => p.callee = Some(self.names(declared, v)),
                Some(("node", v)) => p.node = kinds(g, v),
                Some(("parent", v)) => p.parent = kinds(g, v),
                _ => panic!("call: `{c}`"),
            }
        }
        // (a comma in `arg=` conditions is not a separator: shapes nest only one level)
        let mut shapes: Vec<&str> = args_part.split(',').map(str::trim).collect();
        if let Some(last) = shapes.last_mut()
            && let Some(s) = last.strip_suffix('+')
        {
            *last = s;
            p.repeat = true;
        }
        p.args = shapes
            .into_iter()
            .map(|s| self.shape(g, declared, s))
            .collect();
        p
    }

    fn shape(&self, g: &Grammar, declared: &[String], s: &str) -> Shape {
        let mut parts = s.split('&');
        let head = parts.next().unwrap_or("*").trim();
        let mut shape = Shape {
            any: head == "*",
            ..Shape::default()
        };
        if !shape.any {
            shape.picks = picks(g, head);
        }
        for c in parts {
            shape.conds.push(match c.trim() {
                "block" => Cond::Block,
                "own-line" => Cond::OwnLine(None),
                "comments=none" => Cond::Comments(false),
                "comments=none-inside" => Cond::Comments(true),
                c => match c.split_once('=') {
                    Some(("own-line", k)) => Cond::OwnLine(Some(kind(g, k))),
                    Some(("params", n)) => Cond::Params(false, n.parse().expect("params")),
                    Some(("callee", v)) => Cond::Callee(self.names(declared, v)),
                    Some(("arg", v)) => Cond::Arg(Box::new(self.shape(g, declared, v))),
                    _ => match c.strip_prefix("params<=") {
                        Some(n) => Cond::Params(true, n.parse().expect("params")),
                        None => panic!("call: condition `{c}`"),
                    },
                },
            });
        }
        shape
    }

    fn name_matches(&self, names: &Names, text: Option<&str>) -> bool {
        let Some(text) = text else { return false };
        match names {
            Names::Set(i) => self.sets[*i].iter().any(|n| n == text),
            Names::Text(v) => v.iter().any(|n| n == text),
        }
    }

    /// The effect of the first `[call]` row that matches the call `node` with callee `callee`
    /// and arguments `args`.
    pub fn pattern(
        &self,
        cx: &FormatCtx<'_>,
        node: &SyntaxNode,
        callee: Option<&SyntaxNode>,
        args: &[SyntaxNode],
    ) -> Option<Effect> {
        if self.patterns.is_empty() || args.is_empty() {
            return None;
        }
        // (the text of the callee is made when a row needs it, once)
        let text: std::cell::OnceCell<Option<String>> = std::cell::OnceCell::new();
        let callee_text = || {
            text.get_or_init(|| callee.and_then(|c| callee_text(cx, c)))
                .as_deref()
        };
        self.patterns
            .iter()
            .find(|p| {
                (p.node.is_empty() || p.node.contains(&node.kind()))
                    && self.args_match(cx, p, args)
                    && (p.parent.is_empty()
                        || node.parent().is_some_and(|x| p.parent.contains(&x.kind())))
                    && p.callee
                        .as_ref()
                        .is_none_or(|n| self.name_matches(n, callee_text()))
                    && !core_syntax::roles::has_own_token(node, "?.")
            })
            .map(|p| p.effect)
    }

    fn args_match(&self, cx: &FormatCtx<'_>, p: &Pattern, args: &[SyntaxNode]) -> bool {
        let n = p.args.len();
        if p.repeat {
            if args.len() < n {
                return false;
            }
        } else if args.len() != n {
            return false;
        }
        args.iter().enumerate().all(|(i, a)| {
            let s = &p.args[i.min(n - 1)];
            self.shape_matches(cx, s, a)
        })
    }

    fn shape_matches(&self, cx: &FormatCtx<'_>, s: &Shape, arg: &SyntaxNode) -> bool {
        let n = un(arg);
        let g = cx.grammar;
        (s.any
            || s.picks.iter().any(|p| match p {
                Pick::Kind(k) => n.kind() == *k,
                Pick::Tag(t) => g.has_tag(n.kind(), t),
            }))
            && s.conds.iter().all(|c| match c {
                Cond::Block => g
                    .slot(&n, "body")
                    .is_some_and(|b| g.has_tag(b.kind(), "block")),
                Cond::Params(le, count) => {
                    let have = g.slot(&n, "parameters").map_or(0, |p| {
                        p.as_node().map_or(0, |p| {
                            p.children()
                                .filter(|c| g.has_tag(c.kind(), "parameter"))
                                .count()
                        })
                    });
                    if *le { have <= *count } else { have == *count }
                }
                Cond::Callee(names) => {
                    let callee = g.slot(&n, "callee").and_then(|c| c.into_node());
                    self.name_matches(
                        names,
                        callee.map(|c| callee_text(cx, &c)).as_ref().and_then(|c| c.as_deref()),
                    )
                }
                Cond::Arg(shape) => {
                    let mut args = g.slots(&n, "arguments").filter_map(|e| e.into_node());
                    matches!((args.next(), args.next()), (Some(a), None) if self.shape_matches(cx, shape, &a))
                }
                Cond::OwnLine(literal) => match literal {
                    None => template_on_own_line(cx, &n, &n),
                    Some(k) => n
                        .last_child()
                        .filter(|l| l.kind() == *k)
                        .is_some_and(|l| template_on_own_line(cx, &n, &l)),
                },
                Cond::Comments(inside) => {
                    !has_comment(cx, &n)
                        && (!inside
                            || !n
                                .descendants_with_tokens()
                                .filter_map(|e| e.into_token())
                                .any(|t| token_has_comment(cx, &t)))
                }
            })
    }

    /// Prettier's `isSimpleCallArgument`.
    pub fn is_simple(&self, cx: &FormatCtx<'_>, n: &SyntaxNode, depth: u32) -> bool {
        let s = &self.simple;
        let (g, n) = (cx.grammar, un(n));
        let k = n.kind();
        let simple_at = |c: &SyntaxNode, d: u32| self.is_simple(cx, c, d);
        if let Some((_, max)) = s.regex.iter().find(|(r, _)| *r == k) {
            return n
                .text()
                .to_string()
                .rfind('/')
                .is_some_and(|end| end <= max + 1);
        }
        if s.leaf.iter().any(|p| match p {
            Pick::Kind(l) => *l == k,
            Pick::Tag(t) => g.has_tag(k, t),
        }) {
            return true;
        }
        if s.single_line.contains(&k) {
            return !n.text().to_string().contains('\n')
                && n.children().all(|c| simple_at(&c, depth + 1));
        }
        if s.object.contains(&k) {
            return n.children().all(|p| {
                g.has_tag(p.kind(), "key-value")
                    && !p
                        .children_with_tokens()
                        .any(|e| e.as_token().is_some_and(|t| t.text() == "["))
                    && (p.children().count() == 1
                        || p.last_child().is_some_and(|v| simple_at(&v, depth + 1)))
            });
        }
        if s.array.contains(&k) {
            return n
                .children()
                .all(|c| g.has_tag(c.kind(), "hole") || simple_at(&c, depth + 1));
        }
        if s.items.contains(&k) {
            return n.children().all(|c| simple_at(&c, depth + 1));
        }
        if s.members.contains(&k) {
            return n.children().all(|c| simple_at(&c, depth));
        }
        if s.operand.contains(&k) {
            return n.first_child().is_some_and(|o| simple_at(&o, depth));
        }
        if let Some((_, ops)) = s.unary.iter().find(|(u, _)| *u == k) {
            let op = g
                .slot(&n, "operator")
                .and_then(|e| e.into_token())
                .map_or(String::new(), |t| t.text().to_owned());
            return ops.contains(&op) && n.last_child().is_some_and(|a| simple_at(&a, depth));
        }
        if s.call.contains(&k) {
            let callee = g.slot(&n, "callee").and_then(|c| c.into_node());
            // A call inside a call argument is not simple.
            return depth == 0
                && callee.is_some_and(|c| simple_at(&c, depth))
                && g.slots(&n, "arguments")
                    .filter_map(|e| e.into_node())
                    .all(|a| simple_at(&a, depth + 1));
        }
        false
    }

    /// Prettier's `isHopefullyShortCallArgument`: an argument that is cheap to print before a
    /// hugged first argument.
    pub fn is_short(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        let s = &self.simple;
        let n = un(n);
        let k = n.kind();
        if s.short_binary.contains(&k) {
            return n.children().all(|c| self.is_short(cx, &c));
        }
        if s.short.contains(&k) {
            return true;
        }
        if let Some((_, targs)) = s.cast.iter().find(|(c, _)| *c == k) {
            return n.first_child().is_some_and(|e| self.is_short(cx, &e))
                && n.last_child()
                    .is_some_and(|t| t.children().all(|c| c.kind() != *targs));
        }
        self.is_simple(cx, &n, 0)
    }

    /// Does a call with several function arguments (or one that has a function argument) break
    /// its arguments (`composition = yes`)?
    pub fn composition(&self) -> bool {
        self.simple.composition
    }

    /// Arguments of a call under a node of this kind are never a function composition.
    pub fn no_composition(&self, parent: SyntaxKind) -> bool {
        self.simple.no_composition.contains(&parent)
    }

    /// An argument pair `(a, b)` whose last argument is not hugged.
    pub fn not_last(&self, first: SyntaxKind, last: SyntaxKind) -> bool {
        self.simple.not_last.contains(&(first, last))
    }

    /// An array of two or more numbers, which prints as a fill.
    pub fn is_concise_array(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        let s = &self.simple;
        if !s.concise.contains(&n.kind()) {
            return false;
        }
        let items: Vec<SyntaxNode> = n.children().collect();
        items.len() > 1 && items.iter().all(|i| self.concise_item(cx, i))
    }

    /// A number (signed, without comments inside) with no line comment after it or its comma.
    fn concise_item(&self, cx: &FormatCtx<'_>, i: &SyntaxNode) -> bool {
        let s = &self.simple;
        let line_comment = |t: &core_syntax::SyntaxToken| {
            comments::trailing_comments(cx.grammar, t)
                .iter()
                .any(|c| c.kind == comments::CommentKind::Line)
        };
        let number = s.numeric.contains(&i.kind())
            || (s.signed_numeric.contains(&i.kind())
                && i.last_child()
                    .is_some_and(|c| s.numeric.contains(&c.kind()) && !has_comment(cx, &c))
                && !cx.first_token(i).is_some_and(|t| token_has_comment(cx, &t)));
        number
            && !cx.last_token(i).is_some_and(|t| {
                line_comment(&t)
                    || t.next_token().is_some_and(|c| {
                        cx.grammar.has_tag(c.kind(), "separator") && line_comment(&c)
                    })
            })
    }
}

/// The text of a callee that is a name or a dotted name (`it.only`); `None` for any other callee.
fn callee_text(cx: &FormatCtx<'_>, callee: &SyntaxNode) -> Option<String> {
    let c = un(callee);
    let g = cx.grammar;
    (g.has_tag(c.kind(), "reference") || g.has_tag(c.kind(), "member-access"))
        .then(|| code_text(&c))
}

fn token_has_comment(cx: &FormatCtx<'_>, t: &core_syntax::SyntaxToken) -> bool {
    !comments::leading_comments(cx.grammar, t).is_empty()
        || !comments::trailing_comments(cx.grammar, t).is_empty()
}

/// A comment in front of the first token of `n` or after its last.
pub fn has_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.first_token(n)
        .is_some_and(|t| !comments::leading_comments(cx.grammar, &t).is_empty())
        || cx
            .last_token(n)
            .is_some_and(|t| comments::has_trailing_comment(cx.grammar, &t))
}

/// A node whose literal `lit` (the node, or its template literal when it is tagged) has a line
/// break and starts on the line of the code before it.
pub fn template_on_own_line(cx: &FormatCtx<'_>, n: &SyntaxNode, lit: &SyntaxNode) -> bool {
    let (Some(first), Some(last)) = (cx.first_token(n), cx.last_token(n)) else {
        return false;
    };
    // (a node may own trivia after its last token: a line break there is not in the literal)
    let from = cx.first_token(lit).unwrap_or_else(|| first.clone());
    if !cx.has_line_break(lit, from.text_range().cover(last.text_range())) {
        return false;
    }
    let mut prev = first.prev_token();
    while let Some(t) = prev {
        if cx.grammar.has_tag(t.kind(), "newline") {
            return false;
        }
        if !cx.grammar.is_trivia(t.kind()) {
            break;
        }
        prev = t.prev_token();
    }
    true
}
