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

use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxToken};

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
    /// The `tight_after` and `tight_behind` kind sets: no blank after a token of the first when the
    /// token before it is of the second (`Args &&...rest`).
    tight: (Vec<SyntaxKind>, Vec<SyntaxKind>),
    /// The `pointer` tokens (`*`, `&`) in nodes of the `pointer_in` kinds are declarator marks: the
    /// blank before one (after a token of `pointer_type`) and after one (before a token of
    /// `pointer_name`) follows the language's pointer alignment.
    pointer: [Vec<SyntaxKind>; 4],
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
        let set = |role: &str| -> Vec<SyntaxKind> {
            (bindings.iter())
                .filter(|b| b.role == role)
                .flat_map(|b| b.kinds.iter().copied())
                .collect()
        };
        SpaceRules {
            attach,
            tight: (set("tight_after"), set("tight_behind")),
            pointer: [
                set("pointer"),
                set("pointer_in"),
                set("pointer_type"),
                set("pointer_name"),
            ],
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
        !self.pointer[0].is_empty()
    }

    /// The blank between `a` and `b` when one of them is a declarator mark (`T *x`, `T& x`):
    /// `before` stands for the blank in front of the mark, `after` for the one behind it.
    pub fn pointer_gap(
        &self,
        a: &SyntaxToken,
        b: &SyntaxToken,
        before: bool,
        after: bool,
    ) -> Option<bool> {
        let [marks, within, types, names] = &self.pointer;
        if marks.is_empty() {
            return None;
        }
        let mark = |t: &SyntaxToken| {
            marks.contains(&t.kind()) && t.parent().is_some_and(|n| within.contains(&n.kind()))
        };
        if mark(b) {
            if mark(a) {
                return Some(false);
            }
            return types.contains(&a.kind()).then_some(before);
        }
        (mark(a) && names.contains(&b.kind())).then_some(after)
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
        !self.attach.is_empty()
    }

    /// The option and the word of the construct an opening brace `{` belongs to: the `{` that
    /// starts the block (child of a construct of an `attach` kind set); of several words for one
    /// kind (`class`, `struct`) the one the construct's own keyword spells.
    pub fn attach_word(&self, brace: &SyntaxToken) -> Option<(&str, &str)> {
        if self.attach.is_empty() || brace.text() != "{" {
            return None;
        }
        let block = brace.parent()?;
        let construct = block.parent()?;
        let mut tokens = block
            .descendants_with_tokens()
            .filter_map(|e| e.into_token());
        if tokens.find(|t| !t.text().trim().is_empty()).as_ref() != Some(brace) {
            return None;
        }
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
        Some((chosen.option.as_str(), chosen.word.as_str()))
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
