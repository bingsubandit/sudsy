//! Token spacing as data: a language's rows say which blanks stand between two neighbouring
//! tokens (`Flow::want` and its companions), so a spacing change is a row, not a function.
//!
//! The text has sections `[space]`, `[joined]` and `[at_least_one]`; a line is
//! `PREV / NEXT : RESULT`. The first row of a section that matches
//! wins. `RESULT` is `yes` (one blank), `no` or `keep` (as the source has it); the other sections
//! have no result. A pattern is `*` or conditions joined by `&`: `key=a|b` or `key!=a|b` with
//! `key` one of `kind` and `parent` and `grand` and `next` (names of the grammar's kinds, of the
//! token, of its parent node, of that node's parent and of the token after it), `end_of` (the token ends a node of one of the kinds)
//! and `text`, `exact`, `sibling` and `before` (texts, case-insensitive except `exact`: of the
//! token, of any other token of its parent node, of any token of the parent node
//! before it) and `ends=word` (the token's text
//! ends like a word or a literal: a letter, digit, `_`, quote or `@`). `%amp` in a text stands for `&`. The result is the text after
//! the last `:` when that is `yes`, `no` or `keep`; the sides are split at the first ` / `. Lines
//! starting with `#` are comments. [`Pattern`] is also the matcher of `rewrite::RewriteRules`.
//!
//! concept: token-spacing-rows
//! what: Blanks between neighbouring tokens as ordered rows.
//! tools: google-java-format spacing, ktlint spacing rules, PHP-CS-Fixer spacing, sqlfluff LT01
//! reads: rules:space, rules:joined, rules:at_least_one

use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// A condition on a token: the token's value is (or, with `not`, is not) one of `any`.
struct Cond {
    key: Key,
    not: bool,
    kinds: Vec<SyntaxKind>,
    texts: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Kind,
    Parent,
    Grand,
    EndOf,
    Text,
    Sibling,
    Before,
    Next,
    Ends,
    Exact,
}

#[derive(Default)]
pub struct Pattern(Vec<Cond>);

impl Pattern {
    /// The kinds of a pattern that only names kinds (`kind=A|B`), `None` for any other.
    fn only_kinds(&self) -> Option<&[SyntaxKind]> {
        match self.0.as_slice() {
            [c] if matches!(c.key, Key::Kind) && !c.not => Some(&c.kinds),
            _ => None,
        }
    }

    pub fn parse(grammar: &Grammar, text: &str) -> Pattern {
        let conds = text.trim().split('&').filter(|c| *c != "*").map(|c| {
            let (key, not, vals) = match c.split_once("!=") {
                Some((k, v)) => (k, true, v),
                None => {
                    let (k, v) = c
                        .split_once('=')
                        .unwrap_or_else(|| panic!("spacing: `{c}`"));
                    (k, false, v)
                }
            };
            let key = match key.trim() {
                "kind" => Key::Kind,
                "parent" => Key::Parent,
                "grand" => Key::Grand,
                "end_of" => Key::EndOf,
                "text" => Key::Text,
                "sibling" => Key::Sibling,
                "before" => Key::Before,
                "next" => Key::Next,
                "ends" => Key::Ends,
                "exact" => Key::Exact,
                other => panic!("spacing: unknown key `{other}`"),
            };
            let vals = vals.split('|').map(str::trim);
            let (mut kinds, mut texts) = (Vec::new(), Vec::new());
            for v in vals {
                if matches!(key, Key::Exact) {
                    texts.push(v.to_string());
                } else if matches!(key, Key::Text | Key::Sibling | Key::Before | Key::Ends) {
                    texts.push(v.replace("%amp", "&").to_ascii_lowercase());
                } else {
                    kinds.push(
                        grammar
                            .kind_by_name(v)
                            .unwrap_or_else(|| panic!("spacing: unknown kind `{v}`")),
                    );
                }
            }
            Cond {
                key,
                not,
                kinds,
                texts,
            }
        });
        Pattern(conds.collect())
    }

    pub fn matches(&self, t: &SyntaxToken) -> bool {
        self.matches_in(&Ctx::new(t))
    }

    /// Like [`Pattern::matches`], with the `end_of` answers of a whole file found once
    /// ([`Pattern::end_kinds`], [`EndSet`]) instead of per token.
    pub fn matches_ends(&self, t: &SyntaxToken, ends: &EndSet) -> bool {
        let mut cx = Ctx::new(t);
        cx.ends = Some(ends);
        self.matches_in(&cx)
    }

    /// The kinds of the nodes a positive `parent=` condition names.
    pub fn parent_kinds(&self) -> Vec<SyntaxKind> {
        self.0
            .iter()
            .filter(|c| matches!(c.key, Key::Parent) && !c.not)
            .flat_map(|c| c.kinds.iter().copied())
            .collect()
    }

    /// The kinds of the nodes a positive `end_of` condition names (their last tokens match).
    pub fn end_kinds(&self) -> Vec<SyntaxKind> {
        self.0
            .iter()
            .filter(|c| matches!(c.key, Key::EndOf) && !c.not)
            .flat_map(|c| c.kinds.iter().copied())
            .collect()
    }

    /// Every token this pattern matches is also matched by `broad` (provable from the conditions).
    fn implies(&self, broad: &Pattern) -> bool {
        broad.0.iter().all(|b| {
            self.0.iter().any(|n| {
                if n.key != b.key {
                    return false;
                }
                let (nv, bv): (Vec<&str>, Vec<&str>) = if matches!(b.key, Key::Ends) {
                    return n.not == b.not;
                } else if b.kinds.is_empty() && n.kinds.is_empty() {
                    (
                        n.texts.iter().map(String::as_str).collect(),
                        b.texts.iter().map(String::as_str).collect(),
                    )
                } else {
                    return match (n.not, b.not) {
                        (false, false) => n.kinds.iter().all(|k| b.kinds.contains(k)),
                        (false, true) => n.kinds.iter().all(|k| !b.kinds.contains(k)),
                        (true, true) => b.kinds.iter().all(|k| n.kinds.contains(k)),
                        (true, false) => false,
                    };
                };
                match (n.not, b.not) {
                    (false, false) => nv.iter().all(|t| bv.contains(t)),
                    (false, true) => nv.iter().all(|t| !bv.contains(t)),
                    (true, true) => bv.iter().all(|t| nv.contains(t)),
                    (true, false) => false,
                }
            })
        })
    }

    /// The kinds a matching token's `kind=` condition allows, when it has one.
    pub(crate) fn kinds(&self) -> Option<&[SyntaxKind]> {
        self.0
            .iter()
            .find(|c| matches!(c.key, Key::Kind) && !c.not)
            .map(|c| &c.kinds[..])
    }

    /// The texts a matching token's `text=` or `exact=` condition allows, when it has one.
    fn texts(&self) -> Option<&[String]> {
        self.0
            .iter()
            .find(|c| matches!(c.key, Key::Text | Key::Exact) && !c.not)
            .map(|c| &c.texts[..])
    }

    fn matches_in(&self, cx: &Ctx<'_>) -> bool {
        let t = cx.token;
        self.0.iter().all(|c| {
            let hit =
                match c.key {
                    Key::Kind => c.kinds.contains(&cx.kind),
                    Key::Next => {
                        let mut cur = t.next_token();
                        while let Some(n) = &cur {
                            if !n.text().trim().is_empty() {
                                break;
                            }
                            cur = n.next_token();
                        }
                        cur.is_some_and(|n| c.kinds.contains(&n.kind()))
                    }
                    Key::Text => c.texts.iter().any(|x| t.text().eq_ignore_ascii_case(x)),
                    Key::Exact => c.texts.iter().any(|x| t.text() == x),
                    Key::Ends => t.text().chars().next_back().is_some_and(|c| {
                        c.is_alphanumeric() || matches!(c, '_' | '"' | '\'' | '@')
                    }),
                    Key::Sibling | Key::Before => {
                        let before = matches!(c.key, Key::Before);
                        let mut found = false;
                        let mut cur = if before {
                            t.prev_sibling_or_token()
                        } else {
                            t.parent().and_then(|p| p.first_child_or_token())
                        };
                        while let Some(e) = cur {
                            if let SyntaxElement::Token(o) = &e
                                && o != t
                                && c.texts.iter().any(|x| o.text().eq_ignore_ascii_case(x))
                            {
                                found = true;
                                break;
                            }
                            cur = if before {
                                e.prev_sibling_or_token()
                            } else {
                                e.next_sibling_or_token()
                            };
                        }
                        found
                    }
                    Key::EndOf if cx.ends.is_some() && !c.not => {
                        let ends = cx.ends.unwrap();
                        let at = cx.token.text_range().start();
                        ends.contains(&c.kinds, at)
                    }
                    Key::EndOf => t.parent().is_some_and(|mut n| {
                        loop {
                            if last_code(&n).is_none_or(|l| l != *t) {
                                return false;
                            }
                            if c.kinds.contains(&n.kind()) {
                                return true;
                            }
                            match n.parent() {
                                Some(p) => n = p,
                                None => return false,
                            }
                        }
                    }),
                    Key::Parent => cx.parent().is_some_and(|p| c.kinds.contains(&p)),
                    Key::Grand => cx.grand().is_some_and(|p| c.kinds.contains(&p)),
                };
            hit != c.not
        })
    }
}

/// A token with the kinds of its parent and grandparent, each looked up once however many
/// conditions ask.
struct Ctx<'a> {
    token: &'a SyntaxToken,
    kind: SyntaxKind,
    parent: std::cell::OnceCell<Option<core_syntax::SyntaxNode>>,
    grand: std::cell::OnceCell<Option<SyntaxKind>>,
    ends: Option<&'a EndSet>,
}

/// The last token of every node of some kinds, as `(kind, start of the token)`: what `end_of`
/// asks, answered with one walk of the tree instead of a climb per token.
#[derive(Default)]
pub struct EndSet {
    /// One bit per byte offset where some such node ends: a token that is not at one skips the
    /// hash lookup.
    any: Vec<u64>,
    set: std::collections::HashSet<(SyntaxKind, core_syntax::TextSize)>,
}

impl EndSet {
    /// Notes the end of `node`.
    pub fn add(&mut self, node: &core_syntax::SyntaxNode) {
        if let Some(l) = last_code(node) {
            let at = l.text_range().start();
            let bit = usize::from(at);
            if self.any.len() <= bit / 64 {
                self.any.resize(bit / 64 + 1, 0);
            }
            self.any[bit / 64] |= 1 << (bit % 64);
            self.set.insert((node.kind(), at));
        }
    }

    fn contains(&self, kinds: &[SyntaxKind], at: core_syntax::TextSize) -> bool {
        let bit = usize::from(at);
        self.any
            .get(bit / 64)
            .is_some_and(|w| w >> (bit % 64) & 1 == 1)
            && kinds.iter().any(|k| self.set.contains(&(*k, at)))
    }
}

impl<'a> Ctx<'a> {
    fn new(token: &'a SyntaxToken) -> Self {
        Ctx {
            token,
            kind: token.kind(),
            parent: std::cell::OnceCell::new(),
            grand: std::cell::OnceCell::new(),
            ends: None,
        }
    }

    fn node(&self) -> &Option<core_syntax::SyntaxNode> {
        self.parent.get_or_init(|| self.token.parent())
    }

    fn parent(&self) -> Option<SyntaxKind> {
        self.node().as_ref().map(|n| n.kind())
    }

    fn grand(&self) -> Option<SyntaxKind> {
        *self.grand.get_or_init(|| {
            self.node()
                .as_ref()
                .and_then(|n| n.parent())
                .map(|n| n.kind())
        })
    }
}

/// The last token of the node that is not blank.
pub(crate) fn last_code(n: &core_syntax::SyntaxNode) -> Option<SyntaxToken> {
    let mut cur = n.last_token();
    while let Some(t) = &cur {
        if !t.text().trim().is_empty() {
            break;
        }
        cur = t.prev_token();
    }
    cur
}

struct Row {
    prev: Pattern,
    next: Pattern,
    result: Option<bool>,
}

impl Row {
    fn matches(&self, a: &Ctx<'_>, b: &Ctx<'_>) -> bool {
        self.prev.matches_in(a) && self.next.matches_in(b)
    }
}

/// The rows of `[space]` by the kind and the text of the second token: for each kind and each
/// (lower-case) text some row names, and for the rest, the rows that name it or name none, in
/// order.
struct Index {
    kinds: Slots,
    texts: Vec<String>,
    /// `lists[kind * (texts + 1) + text]`; the last kind and text stand for the unnamed ones.
    lists: Vec<(u32, u32)>,
    /// The row numbers of all lists, one after the other.
    rows: Vec<u32>,
    /// Does the text of the second token matter for a kind (its lists differ by text)?
    by_text: Vec<bool>,
    /// The kinds some row names for the first token (at most 63), and for each row the set of
    /// them it allows (bit `n` for the kinds not named: only rows that name none).
    prev_kinds: Slots,
    prev_masks: Vec<u64>,
}

/// The position of a token kind among a sorted list of kinds, by a table over the kind's index.
struct Slots {
    names: Vec<SyntaxKind>,
    table: Vec<u16>,
}

impl Slots {
    fn new(mut names: Vec<SyntaxKind>) -> Slots {
        names.retain(|k| k.is_token());
        names.sort();
        names.dedup();
        let size = names
            .iter()
            .map(|k| k.index() as usize + 1)
            .max()
            .unwrap_or(0);
        let mut table = vec![u16::MAX; size];
        for (i, k) in names.iter().enumerate() {
            table[k.index() as usize] = i as u16;
        }
        Slots { names, table }
    }

    /// The position of the kind, or the number of names.
    fn get(&self, kind: SyntaxKind) -> usize {
        match self.names.first() {
            Some(f) if kind.is_token() && kind.lang() == f.lang() => self
                .table
                .get(kind.index() as usize)
                .map_or(self.names.len(), |n| match *n {
                    u16::MAX => self.names.len(),
                    n => n as usize,
                }),
            _ => self.names.len(),
        }
    }
}

/// Texts longer than this are never named by a row's index key.
const KEY_TEXT: usize = 16;

impl Index {
    fn build(rows: &[Row]) -> Index {
        let kinds = Slots::new(
            rows.iter()
                .filter_map(|r| r.next.kinds())
                .flatten()
                .copied()
                .collect(),
        );
        let mut texts: Vec<String> = rows
            .iter()
            .filter_map(|r| r.next.texts())
            .flatten()
            .map(|t| t.to_ascii_lowercase())
            .collect();
        texts.sort();
        texts.dedup();
        assert!(texts.iter().all(|t| t.len() <= KEY_TEXT), "spacing: text");
        let wanted = |r: &Row, k: Option<SyntaxKind>, t: Option<&str>| {
            let kind = match (r.next.kinds(), k) {
                (None, _) => true,
                (Some(ks), Some(k)) => ks.contains(&k),
                (Some(_), None) => false,
            };
            let text = match (r.next.texts(), t) {
                (None, _) => true,
                (Some(xs), Some(t)) => xs.iter().any(|x| x.eq_ignore_ascii_case(t)),
                (Some(_), None) => false,
            };
            kind && text
        };
        let (mut lists, mut all) = (Vec::new(), Vec::new());
        for k in kinds.names.iter().map(Some).chain([None]) {
            for t in texts.iter().map(|t| Some(t.as_str())).chain([None]) {
                let start = all.len() as u32;
                all.extend(
                    (0..rows.len() as u32).filter(|i| wanted(&rows[*i as usize], k.copied(), t)),
                );
                lists.push((start, all.len() as u32));
            }
        }
        let prev_kinds = Slots::new(
            rows.iter()
                .filter_map(|r| r.prev.kinds())
                .flatten()
                .copied()
                .collect(),
        );
        let named = prev_kinds.names.len() < 64;
        let prev_masks = rows
            .iter()
            .map(|r| match r.prev.kinds() {
                _ if !named => u64::MAX,
                None => u64::MAX,
                Some(ks) => prev_kinds
                    .names
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| ks.contains(k))
                    .fold(0, |m, (i, _)| m | 1 << i),
            })
            .collect();
        let width = texts.len() + 1;
        let by_text = (0..=kinds.names.len())
            .map(|k| {
                let list = |t: usize| {
                    let (a, b) = lists[k * width + t];
                    &all[a as usize..b as usize]
                };
                (0..texts.len()).any(|t| list(t) != list(texts.len()))
            })
            .collect();
        Index {
            kinds,
            texts,
            lists,
            rows: all,
            by_text,
            prev_kinds,
            prev_masks,
        }
    }

    /// The bit of a first token's kind in the row masks.
    fn slot(&self, prev: SyntaxKind) -> u32 {
        match self.prev_kinds.get(prev) {
            n if n == self.prev_kinds.names.len() => 63,
            n => n as u32,
        }
    }

    /// Can row `i` match a first token with this slot?
    fn allows(&self, i: u32, slot: u32) -> bool {
        self.prev_masks[i as usize] >> slot & 1 == 1
    }

    fn rows(&self, next: &SyntaxToken) -> &[u32] {
        let k = self.kinds.get(next.kind());
        let text = next.text();
        let t = if !self.by_text[k] {
            self.texts.len()
        } else if text.len() <= KEY_TEXT && text.is_ascii() {
            let mut buf = [0u8; KEY_TEXT];
            let low = &mut buf[..text.len()];
            low.copy_from_slice(text.as_bytes());
            low.make_ascii_lowercase();
            self.texts
                .binary_search_by(|x| x.as_bytes().cmp(&*low))
                .unwrap_or(self.texts.len())
        } else {
            self.texts.len()
        };
        let (start, end) = self.lists[k * (self.texts.len() + 1) + t];
        &self.rows[start as usize..end as usize]
    }
}

/// The rows of one language, compiled against its grammar.
pub struct SpaceRules {
    space: Vec<Row>,
    index: Index,
    joined: Vec<Row>,
    at_least_one: Vec<Row>,
    op_first: Vec<Pattern>,
    flat_after: Vec<SyntaxKind>,
    closers: Vec<SyntaxKind>,
    unhang: Vec<Pattern>,
    /// The `attach` kind sets of the options: the constructs whose `{` follows the option's words.
    attach: Vec<crate::rules::Binding>,
    /// The `attach_call` and `loop_call` kind sets: a block right after a node of such a kind (a call
    /// head, `list_for_each(x) {`) belongs to the construct of the `attach_call` word, or to the
    /// word of the `loop_call` set when the head's name is among the words of that option.
    attach_call: (Vec<crate::rules::Binding>, Vec<crate::rules::Binding>),
    /// The `attach_after` kinds: a `{` of no known construct behind such a token is on its line.
    attach_after: Vec<SyntaxKind>,
    /// The `attach_in` kinds: the nodes that are blocks of statements.
    attach_in: Vec<SyntaxKind>,
    /// The `attach_apart` kinds: a token inside such a node (a directive) ends its line.
    attach_apart: Vec<SyntaxKind>,
    /// The comment tokens: not part of the name of a call head.
    comments: Vec<SyntaxKind>,
    /// The `tight_after` and `tight_behind` kind sets: no blank after a token of the first when the
    /// token before it is of the second (`Args &&...rest`).
    tight: (Vec<SyntaxKind>, Vec<SyntaxKind>),
    /// The declarator marks (`*`, `&`) of the `pointer_alignment` kind sets.
    pointer: Pointer,
    /// The braces of the `braced_list_padding` kind sets.
    pad: Pad,
    /// The `cast_node`, `cast_group`, `cast_close`, `cast_keep` and `cast_tight` kind sets: a cast node, its type group, the token
    /// that closes it, the tokens that may be an operator behind it (the blank stays) and those that touch the `)` (`(a)*b`).
    cast: [Vec<SyntaxKind>; 5],
    /// The `binary_node`, `binary_clear`, `binary_cast`, `binary_spaced` and `binary_operand` kind sets (the last:
    /// the nodes whose last name is an operand, never a type): the nodes of binary
    /// operations, the parents that prove a name `*` name (`T *x`, `a * b`) is a product, the tokens that may close
    /// a cast type (`(T)*p`, `(T)&x`: a mark between it and a name stays as written) and the marks that are an
    /// operator, not a cast's, when the source has a blank behind them (`(a) & b`).
    binary: [Vec<SyntaxKind>; 5],
    /// The `hug_brace_after` (what a `{` follows when its list hugs: a type name, not `=`).
    hug: [Vec<SyntaxKind>; 8],
}

/// The kind sets of padded braces: `pad_list` (the nodes whose braces are padded), `pad_open` and
/// `pad_close` (their brace tokens), `pad_except` (a token that makes the list a block the parse took
/// for a list: its braces stay as written).
#[derive(Default)]
struct Pad {
    except: Vec<SyntaxKind>,
    lists: Vec<SyntaxKind>,
    open: Vec<SyntaxKind>,
    close: Vec<SyntaxKind>,
}

/// The kind sets of the declarator marks: `pointer` (the marks), `pointer_in` (their parents) and
/// `pointer_in_under` (the grandparents of marks in a parenthesized type), `pointer_in_loose` (grandparents of marks that need no `pointer_end` behind them: the declaration of a range `for` or a `catch`), `pointer_ref` (marks no other mark follows), `pointer_type` (what
/// stands before the first mark: a type), `pointer_name` (what a name behind the last mark is), `pointer_not_name` (parents of a `pointer_name` or `pointer_end` token that make it none),
/// `pointer_bare` (parents that hold types only, and what `pointer_in_under` names, where a mark needs a `pointer_end` token behind it
/// to be no multiplication), `pointer_shaped` (groups whose marks are declarators once an item of the group
/// is a declaration: two names side by side, `O out`, `T const x`).
#[derive(Default)]
struct Pointer {
    marks: Vec<SyntaxKind>,
    within: Vec<SyntaxKind>,
    under: Vec<SyntaxKind>,
    loose: Vec<SyntaxKind>,
    refs: Vec<SyntaxKind>,
    types: Vec<SyntaxKind>,
    names: Vec<SyntaxKind>,
    /// Parents of a `names` or `end` token that make it neither (the `[` of a lambda's captures).
    not_named: Vec<SyntaxKind>,
    bare: Vec<SyntaxKind>,
    end: Vec<SyntaxKind>,
    shaped: Vec<SyntaxKind>,
}

/// The previous token that is no blank.
fn prev_code(t: &SyntaxToken) -> Option<SyntaxToken> {
    let mut p = t.prev_token();
    while let Some(x) = &p {
        if !x.text().trim().is_empty() {
            break;
        }
        p = x.prev_token();
    }
    p
}

/// The next token that is no blank.
fn next_code(t: &SyntaxToken) -> Option<SyntaxToken> {
    let mut p = t.next_token();
    while let Some(x) = &p {
        if !x.text().trim().is_empty() {
            break;
        }
        p = x.next_token();
    }
    p
}

fn row(grammar: &Grammar, line: &str) -> Row {
    let (pair, result) = match line.rsplit_once(':') {
        Some((p, r)) if matches!(r.trim(), "yes" | "no" | "keep") => (p, r),
        _ => (line, ""),
    };
    let (prev, next) = pair
        .split_once(" / ")
        .unwrap_or_else(|| panic!("spacing: `{line}`"));
    Row {
        prev: Pattern::parse(grammar, prev),
        next: Pattern::parse(grammar, next),
        result: match result.trim() {
            "yes" => Some(true),
            "no" => Some(false),
            _ => None,
        },
    }
}

/// The rows of the spacing sections that an earlier row of the same section always matches first.
pub fn shadowed_rows(grammar: &Grammar, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for name in ["space", "joined", "at_least_one"] {
        let rows: Vec<(&str, Row)> = crate::rules::section(text, name)
            .map(|l| (l, row(grammar, l)))
            .collect();
        for (j, (line, r)) in rows.iter().enumerate() {
            if let Some((first, _)) = rows[..j]
                .iter()
                .find(|(_, e)| r.prev.implies(&e.prev) && r.next.implies(&e.next))
            {
                out.push(format!(
                    "[{name}] `{line}` is never reached: `{first}` matches first"
                ));
            }
        }
    }
    out
}

impl SpaceRules {
    /// The sections `[space]`, `[joined]`, `[at_least_one]`, `[op_first]` and the kind lists
    /// `flat_after` and `closers` of `[flow]`; `unhang` and `op_first` are the language's `RuleFacts`.
    pub fn parse(
        grammar: &Grammar,
        text: &str,
        facts: &crate::rules::RuleFacts,
        bindings: &[crate::rules::Binding],
    ) -> SpaceRules {
        let rows = |name| {
            crate::rules::section(text, name)
                .map(|l| row(grammar, l))
                .collect()
        };
        let flow = crate::rules::settings(text, "flow");
        let kinds = |key: &str| -> Vec<SyntaxKind> {
            flow.iter()
                .filter(|(k, _)| *k == key)
                .flat_map(|(_, v)| v.split('|'))
                .map(|n| {
                    grammar
                        .kind_by_name(n.trim())
                        .unwrap_or_else(|| panic!("spacing: unknown kind `{n}`"))
                })
                .collect()
        };
        let space: Vec<Row> = rows("space");
        let attach = (bindings.iter())
            .filter(|b| b.role == "attach")
            .cloned()
            .collect();
        let calls = |role: &str| -> Vec<crate::rules::Binding> {
            (bindings.iter())
                .filter(|b| b.role == role)
                .cloned()
                .collect()
        };
        let attach_call = (calls("attach_call"), calls("loop_call"));
        let set = |role: &str| -> Vec<SyntaxKind> {
            (bindings.iter())
                .filter(|b| b.role == role)
                .flat_map(|b| b.kinds.iter().copied())
                .collect()
        };
        SpaceRules {
            attach,
            attach_call,
            attach_after: set("attach_after"),
            attach_in: set("attach_in"),
            attach_apart: set("attach_apart"),
            comments: grammar.kinds_with_tag("comment").map(|d| d.kind).collect(),
            tight: (set("tight_after"), set("tight_behind")),
            pointer: Pointer {
                marks: set("pointer"),
                within: set("pointer_in"),
                under: set("pointer_in_under"),
                loose: set("pointer_in_loose"),
                refs: set("pointer_ref"),
                types: set("pointer_type"),
                names: set("pointer_name"),
                not_named: set("pointer_not_name"),
                bare: set("pointer_bare"),
                end: set("pointer_end"),
                shaped: set("pointer_shaped"),
            },
            binary: [
                "binary_node",
                "binary_clear",
                "binary_cast",
                "binary_spaced",
                "binary_operand",
            ]
            .map(set),
            cast: [
                "cast_node",
                "cast_group",
                "cast_close",
                "cast_keep",
                "cast_tight",
            ]
            .map(set),
            pad: Pad {
                except: set("pad_except"),
                lists: set("pad_list"),
                open: set("pad_open"),
                close: set("pad_close"),
            },
            hug: [
                "hug_brace_after",
                "hug_unless",
                "hug_brace_hang",
                "hug_comment_down",
                "comment_continue",
                "name_break",
                "name_break_after",
                "verbatim_split",
            ]
            .map(set),
            index: Index::build(&space),
            space,
            joined: rows("joined"),
            at_least_one: rows("at_least_one"),
            op_first: facts
                .op_first
                .iter()
                .map(|l| Pattern::parse(grammar, l))
                .collect(),
            flat_after: kinds("flat_after"),
            closers: kinds("closers"),
            unhang: facts
                .unhang
                .iter()
                .map(|n| Pattern::parse(grammar, n))
                .collect(),
        }
    }

    /// `Flow::flat_after`: a line break after this element starts no hanging part.
    pub fn flat_after(&self, kind: SyntaxKind) -> bool {
        self.flat_after.contains(&kind)
    }

    /// `Flow::closers`.
    pub fn closer(&self, kind: SyntaxKind) -> bool {
        self.closers.contains(&kind)
    }

    /// `Flow::unhang`.
    pub fn unhangs(&self, t: &SyntaxToken) -> bool {
        let kind = t.kind();
        self.unhang.iter().any(|p| match p.only_kinds() {
            Some(kinds) => kinds.contains(&kind),
            None => p.matches(t),
        })
    }

    /// No blank after `a`: its kind is in `tight_after` and the code token before it is in `tight_behind`.
    fn tight_gap(&self, a: &SyntaxToken) -> bool {
        if self.tight.0.is_empty() || !self.tight.0.contains(&a.kind()) {
            return false;
        }
        let mut prev = a.prev_token();
        while let Some(p) = &prev {
            if !p.text().trim().is_empty() {
                break;
            }
            prev = p.prev_token();
        }
        prev.is_some_and(|p| self.tight.1.contains(&p.kind()))
    }

    /// Does the language have declarator marks (`pointer` kind set)?
    pub fn has_pointer(&self) -> bool {
        !self.pointer.marks.is_empty()
    }

    /// Is `t` the `*` or `&` of a declarator (`T *x`, `T&`, `(char *)`, `vector<int *>`)? One whose
    /// parent is no declaration part, that follows no type or mark, or that a multiplication or a
    /// bitwise and can be (`N * 2` in template arguments, `operator*`) is none: its blanks stay.
    pub fn is_mark(&self, t: &SyntaxToken) -> bool {
        let p = &self.pointer;
        if !p.marks.contains(&t.kind()) {
            return false;
        }
        let Some(parent) = t.parent() else {
            return false;
        };
        // (in a parent that is no declaration part the end of a type behind the mark proves it)
        let known = p.within.contains(&parent.kind())
            || parent
                .parent()
                .is_some_and(|g| p.under.contains(&g.kind()) || p.loose.contains(&g.kind()))
            || (p.shaped.contains(&parent.kind()) && self.declares(&parent));
        let ends_type = || {
            let mut next = next_code(t);
            while let Some(x) = next.as_ref().filter(|x| p.marks.contains(&x.kind())) {
                next = next_code(x);
            }
            next.is_some_and(|x| p.end.contains(&x.kind()) && !self.not_named(&x))
        };
        if !known && !ends_type() {
            return false;
        }
        let mut prev = prev_code(t);
        while let Some(x) = prev.as_ref().filter(|x| p.marks.contains(&x.kind())) {
            prev = prev_code(x);
        }
        if !prev.is_some_and(|x| p.types.contains(&x.kind()) && x.text() != "operator") {
            return false;
        }
        if p.refs.contains(&t.kind()) && next_code(t).is_some_and(|x| p.marks.contains(&x.kind())) {
            return false; // `it && *it`: no pointer or reference follows a reference
        }
        if p.bare.contains(&parent.kind())
            || parent.parent().is_some_and(|g| p.under.contains(&g.kind()))
        {
            return ends_type();
        }
        true
    }

    /// Does an item of the group hold two names side by side (`O out`, `T const x`): is it a list of
    /// parameters? Its marks are then declarators.
    fn declares(&self, group: &SyntaxNode) -> bool {
        let p = &self.pointer;
        let mut prev: Option<SyntaxKind> = None;
        for t in group.children_with_tokens() {
            let Some(t) = t.as_token() else {
                prev = None;
                continue;
            };
            if t.text().trim().is_empty() {
                continue;
            }
            if prev.is_some_and(|a| p.names.contains(&a) && p.types.contains(&a))
                && p.names.contains(&t.kind())
            {
                return true;
            }
            prev = Some(t.kind());
        }
        false
    }

    /// Is `t` a name behind a declarator mark (a `pointer_name` token, but for the parents of
    /// `pointer_not_name`)?
    fn is_name(&self, t: &SyntaxToken) -> bool {
        self.pointer.names.contains(&t.kind()) && !self.not_named(t)
    }

    /// Is `t` in a parent of `pointer_not_name`?
    fn not_named(&self, t: &SyntaxToken) -> bool {
        (t.parent()).is_some_and(|n| self.pointer.not_named.contains(&n.kind()))
    }

    /// The blank between `a` and `b` when one of them is a declarator mark: `before` stands for the
    /// blank in front of the first mark, `after` for the one behind the last, where a name follows.
    pub fn pointer_gap(
        &self,
        a: &SyntaxToken,
        b: &SyntaxToken,
        before: bool,
        after: bool,
    ) -> Option<bool> {
        if !self.has_pointer() {
            return None;
        }
        if self.is_mark(b) {
            return Some(if self.is_mark(a) { false } else { before });
        }
        // (`T &...args` against `T&... args`: the pack's `...` after a mark takes the mark's side)
        (self.is_name(b) && (self.is_mark(a) || self.tight_gap(a))).then_some(after)
    }

    /// The blank around a `*`, `&` or `&&` of a binary operation: one, unless the operation may be a
    /// declaration the parse took for one (a type or mark before, a name, mark or end of a type behind, in a parent that does not
    /// prove an expression): then `None`, the source stays. `None` for other pairs.
    pub fn operator_gap(&self, a: &SyntaxToken, b: &SyntaxToken) -> Option<bool> {
        let p = &self.pointer;
        let binary = |t: &SyntaxToken| {
            p.marks.contains(&t.kind())
                && t.parent()
                    .is_some_and(|n| self.binary[0].contains(&n.kind()))
        };
        let mark = match (binary(a), binary(b)) {
            (true, false) => a,
            (false, true) => b,
            _ => return None,
        };
        let (left, right) = (prev_code(mark)?, next_code(mark)?);
        let clear = mark
            .parent()
            .and_then(|n| n.parent())
            .is_some_and(|n| self.binary[1].contains(&n.kind()));
        let (l, r) = (left.kind(), right.kind());
        // (the `)` of a group of only a type's tokens before a name ends a cast and the operator
        // touches it; the `)` of any other group, a call or `sizeof(T)`, ends an operand)
        if self.binary[2].contains(&l) && self.is_name(&right) {
            // (a blank behind the mark as written makes it an operator)
            let spaced = self.binary[3].contains(&mark.kind())
                && mark
                    .next_token()
                    .is_some_and(|x| x.text().trim().is_empty());
            return Some(spaced || !left.parent().is_some_and(|g| self.cast_like(&g)));
        }
        // (a name that ends a member access, `p->flags & MASK`, is an operand, not a type)
        let operand = left
            .parent()
            .is_some_and(|n| self.binary[4].contains(&n.kind()));
        let declaration = !operand
            && (p.types.contains(&l) || p.end.contains(&l) || p.marks.contains(&l))
            && (self.is_name(&right) || p.end.contains(&r) || p.marks.contains(&r));
        (clear || !declaration).then_some(true)
    }

    /// Is the group a cast's type: of a kind of `cast_group`, only the tokens of a type (a mark only at
    /// its end) and no name or call in front of it (`sizeof(T)`, `f(T)`)?
    fn cast_like(&self, group: &SyntaxNode) -> bool {
        let p = &self.pointer;
        if !self.cast[1].contains(&group.kind()) {
            return false;
        }
        let inner: Vec<_> = (group.children_with_tokens())
            .filter(|e| e.as_token().is_none_or(|t| !t.text().trim().is_empty()))
            .skip(1)
            .collect();
        let typed = inner.iter().enumerate().all(|(i, e)| match e.as_token() {
            Some(t) if p.marks.contains(&t.kind()) => inner.get(i + 1).is_none_or(|n| {
                n.as_token().is_some_and(|n| {
                    p.marks.contains(&n.kind()) || self.cast[2].contains(&n.kind())
                })
            }),
            Some(t) => self.cast[2].contains(&t.kind()) || p.types.contains(&t.kind()),
            None => false,
        });
        // (a group that starts its node, `return (T)&x`, is no argument of a name in front of it)
        let leads = group.parent().is_none_or(|n| {
            (n.children_with_tokens())
                .find(|e| e.as_token().is_none_or(|t| !t.text().trim().is_empty()))
                .and_then(|e| e.into_node())
                .is_some_and(|f| f == *group)
        });
        typed
            && (leads
                || (group.first_token())
                    .and_then(|t| prev_code(&t))
                    .is_none_or(|t| !p.types.contains(&t.kind())))
    }

    /// Does the language have casts (`cast_node` kind set)?
    pub fn has_cast(&self) -> bool {
        !self.cast[0].is_empty()
    }

    /// The blank between the closing token of a cast's type group and its operand: `spaced` stands for
    /// one (`(int) x`). `None` for other pairs.
    pub fn cast_gap(&self, a: &SyntaxToken, b: &SyntaxToken, spaced: bool) -> Option<bool> {
        if !self.cast[2].contains(&a.kind()) || self.cast[3].contains(&b.kind()) {
            return None;
        }
        let group = a.parent().filter(|g| self.cast[1].contains(&g.kind()))?;
        let node = group
            .parent()
            .filter(|n| self.cast[0].contains(&n.kind()))?;
        // (a group that is no type, `(a & MASK) * b`, is an operand: the operator has its blanks)
        let tight = self.cast[4].contains(&b.kind());
        let blank = if tight {
            !self.cast_like(&group)
        } else {
            spaced
        };
        node.text_range()
            .contains_range(b.text_range())
            .then_some(blank)
    }

    /// Is `kind` a node that, standing before a directive inside a declaration, makes the rest of the
    /// declaration's head stay as written (`verbatim_split`)?
    pub fn verbatim_split(&self, kind: SyntaxKind) -> bool {
        self.hug[7].contains(&kind)
    }

    /// Does the language have `name_break` kinds?
    pub fn has_name_break(&self) -> bool {
        !self.hug[5].is_empty()
    }

    /// Does the language have `hug_comment_down` kinds?
    pub fn has_hug_comment_down(&self) -> bool {
        !self.hug[3].is_empty()
    }

    /// Is the list `open` starts of a `hug_comment_down` kind (a comment after its `{` goes below it)?
    pub fn hug_comment_down(&self, open: &SyntaxToken) -> bool {
        open.parent()
            .is_some_and(|p| self.hug[3].contains(&p.kind()))
    }

    /// Is `node` a bracketed list (`(a, b)`, `[a, b]` or `{a, b}`), not a body of statements?
    pub fn in_hug_node(&self, node: &SyntaxNode) -> bool {
        let first = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| !t.text().trim().is_empty());
        matches!(first.as_ref().map(|t| t.text()), Some("(" | "[" | "{"))
            && !node
                .descendants_with_tokens()
                .any(|e| e.as_token().is_some_and(|t| t.text() == ";"))
    }

    /// Is `kind` one of the declarations whose line behind a trailing comment is a continuation
    /// (`comment_continue`)?
    pub fn comment_continue(&self, kind: SyntaxKind) -> bool {
        self.hug[4].contains(&kind)
    }

    /// Is `kind` a declaration whose name goes on a line of its own where its lined-up parameters
    /// would not fit behind the return type (`name_break`)?
    pub fn name_break(&self, kind: SyntaxKind) -> bool {
        self.hug[5].contains(&kind)
    }

    /// Can a name break stand behind a token of `kind` (the end of a return type, not a macro)?
    pub fn name_break_after(&self, kind: SyntaxKind) -> bool {
        self.hug[6].contains(&kind)
    }

    /// Is `node` of a `hug_unless` kind (a body that lays itself out over lines: the list does not line up)?
    pub fn hug_unless(&self, node: &SyntaxNode) -> bool {
        node.descendants().any(|d| self.hug[1].contains(&d.kind()))
            || node
                .descendants_with_tokens()
                .any(|e| e.as_token().is_some_and(|t| t.text() == "{"))
    }

    /// How the list `open` starts lines up: `Some(true)` its first item may go on the line of the
    /// bracket (a `(` or `[`, or a `{` behind a `hug_brace_after` token, a type name); `Some(false)`
    /// it hangs (a `{` behind a `hug_brace_hang` token, `=`); `None` the source decides.
    pub fn hug_after(&self, open: &SyntaxToken) -> Option<bool> {
        if open.text() != "{" {
            return Some(true);
        }
        let mut p = open.prev_token();
        while let Some(t) = p.as_ref().filter(|t| {
            t.text().trim().is_empty() || t.text().starts_with("//") || t.text().starts_with("/*")
        }) {
            p = t.prev_token();
        }
        let kind = p?.kind();
        if self.hug[0].contains(&kind) {
            Some(true)
        } else {
            self.hug[2].contains(&kind).then_some(false)
        }
    }

    /// Does the language pad braces (`pad_list` kind set)?
    pub fn has_pad(&self) -> bool {
        !self.pad.lists.is_empty()
    }

    /// The blank inside the braces of a padded list: `padded` stands for the blank behind the opening
    /// and before the closing brace; the braces of an empty list touch. `None` for other pairs.
    pub fn pad_gap(&self, a: &SyntaxToken, b: &SyntaxToken, padded: bool) -> Option<bool> {
        let brace = |t: &SyntaxToken, set: &[SyntaxKind]| {
            set.contains(&t.kind())
                && t.parent()
                    .is_some_and(|p| self.pad.lists.contains(&p.kind()))
        };
        let list = if brace(a, &self.pad.open) {
            a.parent()
        } else {
            b.parent()
        };
        let block = |n: SyntaxNode| {
            n.descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| self.pad.except.contains(&t.kind()))
        };
        if (brace(a, &self.pad.open) || brace(b, &self.pad.close)) && list.is_some_and(block) {
            return None;
        }
        let closes = brace(b, &self.pad.close);
        if brace(a, &self.pad.open) {
            return Some(padded && !closes);
        }
        closes.then_some(padded)
    }

    /// Right-aligned marks (a blank before, none behind) minus left-aligned ones (none before, a
    /// blank behind) of the tree `t` is in (the evidence of `derive_pointer_alignment`).
    pub fn pointer_evidence(&self, t: &SyntaxToken) -> i32 {
        let Some(root) = t.parent().and_then(|p| p.ancestors().last()) else {
            return 0;
        };
        let blank = |x: Option<SyntaxToken>| {
            x.is_some_and(|x| x.text().trim().is_empty() && !x.text().contains('\n'))
        };
        let mut net = 0;
        for m in root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|m| self.pointer.marks.contains(&m.kind()))
        {
            let (before, after) = (blank(m.prev_token()), blank(m.next_token()));
            if before != after && self.is_mark(&m) {
                net += if before { 1 } else { -1 };
            }
        }
        net
    }

    /// `Flow::want`: a blank between the tokens (`None`: as the source has it).
    pub fn want(&self, a: &SyntaxToken, b: &SyntaxToken) -> Option<bool> {
        if self.tight_gap(a) {
            return Some(false);
        }
        let (a, b) = (Ctx::new(a), Ctx::new(b));
        let slot = self.index.slot(a.kind);
        self.index
            .rows(b.token)
            .iter()
            .filter(|i| self.index.allows(**i, slot))
            .map(|i| &self.space[*i as usize])
            .find(|r| r.matches(&a, &b))
            .and_then(|r| r.result)
    }

    /// `want` and `joined` for every pair of neighbouring tokens (entry `i` is the gap before
    /// token `i`; the first has none): each token's parent and grandparent are looked up once.
    pub fn gaps(&self, tokens: &[SyntaxToken]) -> Vec<(Option<bool>, bool)> {
        let cxs: Vec<Ctx<'_>> = tokens.iter().map(Ctx::new).collect();
        let mut out = vec![(None, false)];
        out.extend(cxs.windows(2).map(|w| {
            let (a, b) = (&w[0], &w[1]);
            let slot = self.index.slot(a.kind);
            let tight = self.tight_gap(a.token);
            let want = self
                .index
                .rows(b.token)
                .iter()
                .filter(|i| self.index.allows(**i, slot))
                .map(|i| &self.space[*i as usize])
                .find(|r| r.matches(a, b))
                .and_then(|r| r.result);
            let want = if tight { Some(false) } else { want };
            (want, self.joined.iter().any(|r| r.matches(a, b)))
        }));
        out
    }

    /// Does a language option place braces (an `attach` kind set)?
    pub fn has_attach(&self) -> bool {
        !self.attach.is_empty() || !self.attach_call.0.is_empty()
    }

    /// The `loop_call` binding of a call head whose name is a loop macro (`list_for_each(x)`).
    fn loop_call(
        &self,
        head: &SyntaxNode,
        options: &core_config::ResolvedOptions,
    ) -> Option<&crate::rules::Binding> {
        let name = (head.descendants_with_tokens())
            .filter_map(|e| e.into_token())
            .find(|t| !t.text().trim().is_empty() && !self.comments.contains(&t.kind()))
            .map(|t| t.text().to_string());
        self.attach_call.1.iter().find(|l| {
            l.kinds.contains(&head.kind())
                && options
                    .strings(l.option)
                    .any(|w| Some(w) == name.as_deref())
        })
    }

    /// Is `block` the body of a loop macro (`list_for_each(x) { ... }`)?
    pub fn is_loop_block(
        &self,
        block: &SyntaxNode,
        options: &core_config::ResolvedOptions,
    ) -> bool {
        block
            .prev_sibling()
            .is_some_and(|head| self.loop_call(&head, options).is_some())
    }

    /// The option and the word of the construct an opening brace `{` belongs to: the `{` that
    /// starts the block (child of a construct of an `attach` kind set); of several words for one
    /// kind (`class`, `struct`) the one the construct's own keyword spells.
    pub fn attach_word(
        &self,
        brace: &SyntaxToken,
        options: &core_config::ResolvedOptions,
    ) -> Option<(core_config::OptId, &str)> {
        if (self.attach.is_empty() && self.attach_call.0.is_empty()) || brace.text() != "{" {
            return None;
        }
        let block = brace.parent()?;
        let mut tokens = block
            .descendants_with_tokens()
            .filter_map(|e| e.into_token());
        if tokens.find(|t| !t.text().trim().is_empty()).as_ref() != Some(brace) {
            return None;
        }
        if let Some(head) = block.prev_sibling()
            && let Some(chosen) =
                (self.attach_call.0.iter()).find(|b| b.kinds.contains(&head.kind()))
        {
            let loops = self.loop_call(&head, options);
            return Some((chosen.option, loops.unwrap_or(chosen).word.as_str()));
        }
        let construct = block.parent()?;
        let mut of = self
            .attach
            .iter()
            .filter(|b| b.kinds.contains(&construct.kind()));
        let first = of.next()?;
        let spelled = |b: &&crate::rules::Binding| {
            (construct.children_with_tokens())
                .filter_map(|e| e.into_token())
                .any(|t| t.text() == b.word)
        };
        let chosen = std::iter::once(first)
            .chain(of)
            .find(|b| spelled(b))
            .unwrap_or(first);
        Some((chosen.option, chosen.word.as_str()))
    }

    /// Does a `{` that belongs to no known construct (`attach_word` is none) follow a token of the
    /// `attach_after` kinds, with no construct wrapping its brace: the block of a declaration the
    /// parse did not recognise, a lambda; it stays on its line.
    pub fn attach_unknown(
        &self,
        brace: &SyntaxToken,
        a: &SyntaxToken,
        options: &core_config::ResolvedOptions,
    ) -> bool {
        // (where no construct wraps its brace, every brace is on its line)
        if !self.attach_after.contains(&a.kind())
            || brace.text() != "{"
            || (self.attach.iter()).any(|b| options.strings(b.option).next().is_some())
            || a.parent_ancestors()
                .any(|n| self.attach_apart.contains(&n.kind()))
        {
            return false;
        }
        let Some(block) = brace
            .parent()
            .filter(|b| self.attach_in.contains(&b.kind()))
        else {
            return false;
        };
        let mut tokens = (block.descendants_with_tokens()).filter_map(|e| e.into_token());
        tokens.find(|t| !t.text().trim().is_empty()).as_ref() == Some(brace)
    }

    /// `Flow::joined`.
    pub fn joined(&self, a: &SyntaxToken, b: &SyntaxToken) -> bool {
        if self.joined.is_empty() {
            return false;
        }
        let (a, b) = (Ctx::new(a), Ctx::new(b));
        self.joined.iter().any(|r| r.matches(&a, &b))
    }

    /// `Flow::at_least_one`.
    pub fn at_least_one(&self, a: &SyntaxToken, b: &SyntaxToken) -> bool {
        if self.at_least_one.is_empty() {
            return false;
        }
        let (a, b) = (Ctx::new(a), Ctx::new(b));
        self.at_least_one.iter().any(|r| r.matches(&a, &b))
    }

    /// A blank between the tokens where the first matching row of `[space]` says `yes`: for a
    /// language whose rows end in `* / * : yes` and that has no "as the source has it".
    pub fn spaced(&self, a: &SyntaxToken, b: &SyntaxToken) -> bool {
        self.want(a, b) == Some(true)
    }

    /// `Flow::op_first`.
    pub fn op_first(&self, t: &SyntaxToken) -> bool {
        self.op_first.iter().any(|p| p.matches(t))
    }
}

/// The characters a language's tokens can fuse through (see [`fuses`]).
pub struct Fusion {
    /// Characters of operators.
    pub operators: &'static str,
    /// Characters that start a variable or a name after an operator (`@`, `#`).
    pub sigils: &'static str,
    /// The opener of a line comment: two characters (`--`).
    pub line_comment: [char; 2],
}

/// Would the text of two neighbouring tokens read as other tokens with nothing between them: a
/// line comment opener, two words, an operator that follows a prefix operator (`prev_is_prefix`)
/// or a sigil that follows an operator?
pub fn fuses(prev: &str, next: &str, prev_is_prefix: bool, f: &Fusion) -> bool {
    let (a, b) = (prev.chars().next_back(), next.chars().next());
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let operator = |c: Option<char>| c.is_some_and(|c| f.operators.contains(c));
    (a == Some(f.line_comment[0]) && b == Some(f.line_comment[1]))
        || (word(a) && word(b))
        || (prev_is_prefix && operator(b))
        || (operator(a) && b.is_some_and(|c| f.sigils.contains(c)))
}

#[cfg(test)]
mod fusion_tests {
    use super::*;

    const F: Fusion = Fusion {
        operators: "+-<>=",
        sigils: "@#",
        line_comment: ['-', '-'],
    };

    #[test]
    fn tokens_that_would_read_as_others_fuse() {
        assert!(fuses("-", "-", false, &F));
        assert!(fuses("a", "b", false, &F));
        assert!(fuses("-", "+", true, &F));
        assert!(fuses("<", "@y", false, &F));
        assert!(!fuses("<", "y", false, &F));
        assert!(!fuses(",", "@y", false, &F));
    }
}
