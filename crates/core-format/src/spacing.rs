//! Token spacing as data: a language's rows say which blanks stand between two neighbouring
//! tokens (`Flow::want` and its companions), so a spacing change is a row, not a function.
//!
//! The text has sections `[space]`, `[joined]`, `[at_least_one]` and `[op_first]`; a line is
//! `PREV / NEXT : RESULT` (`[op_first]`: one pattern). The first row of a section that matches
//! wins. `RESULT` is `yes` (one blank), `no` or `keep` (as the source has it); the other sections
//! have no result. A pattern is `*` or conditions joined by `&`: `key=a|b` or `key!=a|b` with
//! `key` one of `kind` and `parent` and `grand` (names of the grammar's kinds, of the token, of
//! its parent node and of that node's parent), `end_of` (the token ends a node of one of the kinds)
//! and `text`, `exact`, `sibling` and `before` (texts, case-insensitive except `exact`: of the
//! token, of any other token of its parent node, of any token of the parent node
//! before it) and `ends=word` (the token's text
//! ends like a word or a literal: a letter, digit, `_`, quote or `@`). The result is the text after
//! the last `:` when that is `yes`, `no` or `keep`; the sides are split at the first ` / `. Lines
//! starting with `#` are comments. [`Pattern`] is also the matcher of `rewrite::RewriteRules`.

use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxToken};

/// A condition on a token: the token's value is (or, with `not`, is not) one of `any`.
struct Cond {
    key: Key,
    not: bool,
    kinds: Vec<SyntaxKind>,
    texts: Vec<String>,
}

#[derive(Clone, Copy)]
enum Key {
    Kind,
    Parent,
    Grand,
    EndOf,
    Text,
    Sibling,
    Before,
    Ends,
    Exact,
}

#[derive(Default)]
pub struct Pattern(Vec<Cond>);

impl Pattern {
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
                    texts.push(v.to_ascii_lowercase());
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

    /// The kinds a matching token's `kind=` condition allows, when it has one.
    fn kinds(&self) -> Option<&[SyntaxKind]> {
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
}

impl<'a> Ctx<'a> {
    fn new(token: &'a SyntaxToken) -> Self {
        Ctx {
            token,
            kind: token.kind(),
            parent: std::cell::OnceCell::new(),
            grand: std::cell::OnceCell::new(),
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
fn last_code(n: &core_syntax::SyntaxNode) -> Option<SyntaxToken> {
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
    unhang: Vec<SyntaxKind>,
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

impl SpaceRules {
    /// The sections `[space]`, `[joined]`, `[at_least_one]`, `[op_first]` and the kind lists
    /// `flat_after`, `closers` and `unhang` of `[flow]`.
    pub fn parse(grammar: &Grammar, text: &str) -> SpaceRules {
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
        SpaceRules {
            index: Index::build(&space),
            space,
            joined: rows("joined"),
            at_least_one: rows("at_least_one"),
            op_first: crate::rules::section(text, "op_first")
                .map(|l| Pattern::parse(grammar, l))
                .collect(),
            flat_after: kinds("flat_after"),
            closers: kinds("closers"),
            unhang: kinds("unhang"),
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
    pub fn unhangs(&self, kind: SyntaxKind) -> bool {
        self.unhang.contains(&kind)
    }

    /// `Flow::want`: a blank between the tokens (`None`: as the source has it).
    pub fn want(&self, a: &SyntaxToken, b: &SyntaxToken) -> Option<bool> {
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
