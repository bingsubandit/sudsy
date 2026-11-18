//! Grammar descriptions (`grammars/<lang>.grammar`).
//!
//! ```text
//! // comment
//! language json 1 "JSON"              // name, LanguageId (1..=31), display name (optional)
//! comments line "//" block "/*" "*/"   // either part optional
//! suppress disable-next-line "sudsy-disable-next-line"   // role, marker; roles: disable-next-line,
//!                                                        // disable-line, disable, enable, disable-file, ignore*
//! token Whitespace #trivia #whitespace // variable-text token with tags
//! token LBrace "{" #punctuation        // fixed-text token
//! node Member #key-value = key:String Colon value:#value
//! node Array #value #delimited-list = LBracket elements:#value* RBracket
//! roles                                // opt in: every node kind has a role tag or #opaque
//! node Call #call = callee:#value args=arguments:ArgList   // `args` fills the role field `arguments`
//! ```
//!
//! A node body is a list of items `[label:]Ref[?|*]`. `Ref` is a node or token
//! name, or `#tag` meaning "any kind carrying that tag" (all nodes or all
//! tokens, or a union of both, which has no typed accessor). Labeled items become typed
//! accessors; unlabeled ones document the shape. A declaration may continue on following indented
//! lines. A body is matched against a node's children in order (see [`crate::ast::assign`]), so the same type may appear in several
//! items; bodies are not enforced against trees, the conformance suites check them.
//!
//! Kind ids are assigned in declaration order, see [`crate::kind`]. The
//! grammar's version is a SHA-256 over its lexemes (comments and layout do not
//! change it; declaration order does).

mod parse;

use crate::kind::{LanguageId, SyntaxKind};
use crate::tree::{SyntaxElement, SyntaxNode};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub use parse::GrammarError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cardinality {
    One,
    Optional,
    Many,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Kind(SyntaxKind),
    Tag(Arc<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: Option<String>,
    /// The role field the item fills when its label differs (`label=field:Type`).
    pub field: Option<String>,
    pub target: Target,
    pub cardinality: Cardinality,
    /// The range of `Grammar::pool` holding the sorted kinds the item accepts.
    kinds: (u32, u32),
}

#[derive(Clone, Debug)]
pub struct KindDef {
    pub name: String,
    pub kind: SyntaxKind,
    /// Fixed text of a token (punctuation, keywords).
    pub text: Option<String>,
    pub tags: Vec<Arc<str>>,
    /// Shape of a node; empty for tokens.
    pub body: Vec<Item>,
}

impl KindDef {
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| &**t == tag)
    }

    /// Labeled body items, i.e. the ones that become accessors.
    pub fn fields(&self) -> impl Iterator<Item = &Item> {
        self.body.iter().filter(|i| i.label.is_some())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommentSyntax {
    pub line: Option<String>,
    pub block: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suppression {
    pub role: String,
    pub marker: String,
}

/// How operators of one precedence level join into a chain (`precedence` lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assoc {
    /// `a op b op c` is `(a op b) op c`; every operator of the level flattens.
    Left,
    /// `a op (b op c)`: flattens down the right side.
    Right,
    /// Only repeats of the same operator flatten (`a * b * c`, not `a * b / c`).
    Same,
    /// Never flattens (`a == b == c`).
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Precedence {
    /// Higher binds tighter.
    pub level: u8,
    pub assoc: Assoc,
}

/// One `precedence <level> <assoc> "op"...` line.
#[derive(Clone, Debug)]
pub struct PrecedenceRow {
    pub level: u8,
    pub assoc: Assoc,
    pub operators: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Grammar {
    pub language: String,
    pub language_id: LanguageId,
    /// The language's name for people (`C#`); the `language` name when the grammar gives none.
    pub title: String,
    pub comments: CommentSyntax,
    pub suppressions: Vec<Suppression>,
    /// The grammar declared `roles`: [`Grammar::check_roles`] applies.
    pub requires_roles: bool,
    /// Operator text to operator kind (`operators` lines).
    operators: HashMap<String, String>,
    /// The `precedence` lines (a handful of rows: scanned, no table).
    precedence_rows: Vec<PrecedenceRow>,
    nodes: Vec<KindDef>,
    tokens: Vec<KindDef>,
    by_name: HashMap<String, SyntaxKind>,
    /// The grammar text: the hash is computed from it on demand.
    source: Box<str>,
    hash: OnceLock<String>,
    /// Hot tags per kind, indexed like `tokens` and `nodes`.
    token_flags: Vec<u16>,
    node_flags: Vec<u16>,
    /// The sorted kinds of every body item (each tag's once, shared by its items); an item holds
    /// its range.
    pool: Vec<SyntaxKind>,
    /// Trivia and `ERROR`, sorted: invisible to bodies.
    skip: Vec<SyntaxKind>,
    /// Node kinds per tag, in id order.
    tag_nodes: HashMap<String, Vec<SyntaxKind>>,
}

/// Tags that formatting code asks about for every token or node: answered from a bit set
/// instead of scanning the tag strings.
const HOT_TAGS: [&str; 12] = [
    "trivia",
    "separator",
    "optional",
    "restyle",
    "comment",
    "delimited-list",
    "block",
    "key-value",
    "verbatim-trivia",
    "glue",
    "close",
    "indentable",
];

fn hot_flags(defs: &[KindDef]) -> Vec<u16> {
    defs.iter()
        .map(|d| {
            HOT_TAGS
                .iter()
                .enumerate()
                .filter(|(_, t)| d.has_tag(t))
                .fold(0u16, |acc, (i, _)| acc | 1 << i)
        })
        .collect()
}

/// Where a labelled body item is (see [`Grammar::slot`]).
struct FieldAt {
    item: usize,
    /// The child position when each item before it and it are `One`.
    fixed: Option<usize>,
}

/// One node kind's body as [`crate::ast::assign`] sees it.
#[derive(Clone, Copy)]
struct GrammarBody<'g> {
    grammar: &'g Grammar,
    body: &'g [Item],
}

impl crate::ast::Body for GrammarBody<'_> {
    fn slots(&self) -> usize {
        self.body.len()
    }
    fn accepts(&self, slot: usize, kind: SyntaxKind) -> bool {
        self.grammar
            .range(self.body[slot].kinds)
            .binary_search(&kind)
            .is_ok()
    }
    fn many(&self, slot: usize) -> bool {
        self.body[slot].cardinality == Cardinality::Many
    }
    fn skips(&self, kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ERROR || self.grammar.is_trivia(kind)
    }
}

impl Grammar {
    /// Fills the tables derived from the kinds: resolved bodies, the skipped kinds and the nodes
    /// per tag. One pass collects the kinds of every tag; a body item takes its range of the pool.
    fn resolve(&mut self) {
        let mut all: HashMap<&str, Vec<SyntaxKind>> = HashMap::new();
        let mut of_nodes: HashMap<&str, Vec<SyntaxKind>> = HashMap::new();
        for d in self.tokens.iter().chain(&self.nodes) {
            for t in &d.tags {
                all.entry(t).or_default().push(d.kind);
            }
        }
        for d in &self.nodes {
            for t in &d.tags {
                of_nodes.entry(t).or_default().push(d.kind);
            }
        }
        // The pool holds each tag's sorted kinds once; a body item of that tag shares the range.
        let mut pool: Vec<SyntaxKind> = Vec::new();
        let mut ranges: HashMap<&str, (u32, u32)> = HashMap::new();
        let mut bodies = Vec::new();
        for d in &self.nodes {
            for i in &d.body {
                bodies.push(match &i.target {
                    Target::Kind(k) => {
                        pool.push(*k);
                        (pool.len() as u32 - 1, pool.len() as u32)
                    }
                    Target::Tag(t) => *ranges.entry(t).or_insert_with(|| {
                        let start = pool.len() as u32;
                        let mut kinds = all.get(&**t).cloned().unwrap_or_default();
                        kinds.sort();
                        pool.extend(kinds);
                        (start, pool.len() as u32)
                    }),
                });
            }
        }
        let mut skip: Vec<SyntaxKind> = std::iter::once(SyntaxKind::ERROR)
            .chain(all.get("trivia").into_iter().flatten().copied())
            .collect();
        skip.sort();
        let tag_nodes = of_nodes
            .into_iter()
            .map(|(t, v)| (t.to_string(), v))
            .collect();
        let mut ranges = bodies.into_iter();
        for i in self.nodes.iter_mut().flat_map(|d| &mut d.body) {
            i.kinds = ranges.next().expect("a range per body item");
        }
        self.pool = pool;
        self.skip = skip;
        self.tag_nodes = tag_nodes;
    }

    fn range(&self, (start, end): (u32, u32)) -> &[SyntaxKind] {
        &self.pool[start as usize..end as usize]
    }

    /// For each body item of a node kind, the sorted kinds it accepts.
    pub fn body_kinds(&self, node: SyntaxKind) -> Option<impl Iterator<Item = &[SyntaxKind]>> {
        Some(self.def(node)?.body.iter().map(|i| self.range(i.kinds)))
    }

    /// Trivia kinds and `ERROR`, sorted: they take no part in matching bodies.
    pub fn skipped_kinds(&self) -> &[SyntaxKind] {
        &self.skip
    }

    /// The node kinds carrying `tag`, in id order (precomputed; empty for an unknown tag).
    pub fn nodes_with_tag(&self, tag: &str) -> &[SyntaxKind] {
        self.tag_nodes.get(tag).map_or(&[], Vec::as_slice)
    }

    /// The child of `node` in a body item labelled `label` or filling the role field `label` (the
    /// first, for a repeated item or several items of one field: a postfix `operator` after the
    /// operand and a prefix one before it are two items of the field `operator`).
    /// Found with the routine the generated accessors use, so it agrees with them.
    pub fn slot(&self, node: &SyntaxNode, label: &str) -> Option<SyntaxElement> {
        let def = self.def(node.kind())?;
        let body = GrammarBody {
            grammar: self,
            body: &def.body,
        };
        crate::ast::assign(node, body)
            .find(|(i, _)| {
                let item = &def.body[*i];
                item.label.as_deref() == Some(label) || item.field.as_deref() == Some(label)
            })
            .map(|(_, e)| e)
    }

    /// [`Grammar::slot`] on the non-trivia children of `node` already collected (`children`):
    /// no second walk of the tree; an item that follows only single required items is read at its
    /// fixed child position.
    pub fn slot_in(
        &self,
        kind: SyntaxKind,
        children: &[SyntaxElement],
        label: &str,
    ) -> Option<SyntaxElement> {
        let at = self.field_at(kind, label)?;
        if let Some(p) = at.fixed {
            let e = children.get(p)?;
            if e.kind() != SyntaxKind::ERROR && self.accepts(kind, at.item, e.kind()) {
                return Some(e.clone());
            }
        }
        self.slot_slow(kind, children.iter().cloned(), label)
    }

    fn field_at(&self, kind: SyntaxKind, label: &str) -> Option<FieldAt> {
        let def = self.def(kind)?;
        let mut all_one = true;
        for (i, item) in def.body.iter().enumerate() {
            all_one &= item.cardinality == Cardinality::One;
            if item.label.as_deref() == Some(label) || item.field.as_deref() == Some(label) {
                return Some(FieldAt {
                    item: i,
                    fixed: all_one.then_some(i),
                });
            }
        }
        None
    }

    fn accepts(&self, kind: SyntaxKind, item: usize, child: SyntaxKind) -> bool {
        let def = self.def(kind);
        def.is_some_and(|d| self.range(d.body[item].kinds).binary_search(&child).is_ok())
    }

    fn slot_slow(
        &self,
        kind: SyntaxKind,
        children: impl Iterator<Item = SyntaxElement>,
        label: &str,
    ) -> Option<SyntaxElement> {
        let def = self.def(kind)?;
        let body = GrammarBody {
            grammar: self,
            body: &def.body,
        };
        crate::ast::assign_of(children, body)
            .find(|(i, _)| {
                let item = &def.body[*i];
                item.label.as_deref() == Some(label) || item.field.as_deref() == Some(label)
            })
            .map(|(_, e)| e)
    }

    /// Every child of `node` in the items of the field `label`, in order (the `cases` of a switch).
    pub fn slots<'a>(
        &'a self,
        node: &'a SyntaxNode,
        label: &'a str,
    ) -> impl Iterator<Item = SyntaxElement> + 'a {
        let def = self.def(node.kind());
        let body = def.and_then(|def| {
            Some(GrammarBody {
                grammar: self,
                body: &def.body,
            })
        });
        body.into_iter().flat_map(move |body| {
            crate::ast::assign(node, body).filter_map(move |(i, e)| {
                let item = &body.body[i];
                (item.label.as_deref() == Some(label) || item.field.as_deref() == Some(label))
                    .then_some(e)
            })
        })
    }

    /// The operator kind (`equality`, `logical`, ...; [`crate::roles::OPERATOR_KINDS`]) of operator
    /// text: the grammar's `operators` lines, then the common table.
    pub fn operator_kind(&self, text: &str) -> Option<&str> {
        match self.operators.get(text) {
            Some(kind) => Some(kind),
            None => crate::roles::common_operator_kind(text),
        }
    }

    /// The precedence level and associativity of binary operator text (the grammar's `precedence`
    /// lines); `None` for text with no entry.
    pub fn precedence(&self, text: &str) -> Option<Precedence> {
        self.precedence_rows
            .iter()
            .find(|r| r.operators.iter().any(|o| o == text))
            .map(|r| Precedence {
                level: r.level,
                assoc: r.assoc,
            })
    }

    /// What is wrong with the roles of a grammar that declared `roles`: a node kind with no role
    /// tag and no `#opaque`, an opaque one with a role, a role whose required fields its body
    /// lacks. Empty for a grammar that did not opt in.
    pub fn check_roles(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !self.requires_roles {
            return errors;
        }
        for n in &self.nodes {
            let roles: Vec<_> = n
                .tags
                .iter()
                .filter_map(|t| crate::roles::role(t))
                .collect();
            let opaque = n.has_tag(crate::roles::OPAQUE);
            match (roles.is_empty(), opaque) {
                (true, false) => errors.push(format!("{}: no role and not #opaque", n.name)),
                (false, true) => errors.push(format!("{}: #opaque with a role", n.name)),
                _ => {}
            }
            for r in roles {
                for (field, required) in r.fields() {
                    let has = n.body.iter().any(|i| {
                        i.label.as_deref() == Some(field) || i.field.as_deref() == Some(field)
                    });
                    if required && !has {
                        errors.push(format!(
                            "{}: role #{} needs field `{field}`",
                            n.name, r.name
                        ));
                    }
                }
            }
        }
        errors
    }

    pub fn parse(source: &str) -> Result<Grammar, GrammarError> {
        parse::parse(source)
    }

    /// Content hash (hex SHA-256); the grammar's version.
    pub fn hash(&self) -> &str {
        self.hash.get_or_init(|| parse::content_hash(&self.source))
    }

    pub fn nodes(&self) -> &[KindDef] {
        &self.nodes
    }

    pub fn tokens(&self) -> &[KindDef] {
        &self.tokens
    }

    pub fn kind_by_name(&self, name: &str) -> Option<SyntaxKind> {
        self.by_name.get(name).copied()
    }

    /// Definition of a kind of this language; `None` for core or foreign kinds.
    pub fn def(&self, kind: SyntaxKind) -> Option<&KindDef> {
        if kind.lang() != self.language_id {
            return None;
        }
        let list = if kind.is_token() {
            &self.tokens
        } else {
            &self.nodes
        };
        list.get(kind.index() as usize)
    }

    pub fn has_tag(&self, kind: SyntaxKind, tag: &str) -> bool {
        if let Some(bit) = HOT_TAGS.iter().position(|t| *t == tag) {
            return self.flags_of(kind) & (1 << bit) != 0;
        }
        self.def(kind).is_some_and(|d| d.has_tag(tag))
    }

    fn flags_of(&self, kind: SyntaxKind) -> u16 {
        if kind.lang() != self.language_id {
            return 0;
        }
        let list = if kind.is_token() {
            &self.token_flags
        } else {
            &self.node_flags
        };
        list.get(kind.index() as usize).copied().unwrap_or(0)
    }

    /// The text of a comment without its delimiters (`//`, or `/*` and `*/`), trimmed.
    pub fn comment_body<'a>(&self, text: &'a str) -> &'a str {
        let body = match (self.comments.line.as_deref(), &self.comments.block) {
            (Some(prefix), _) if text.starts_with(prefix) => &text[prefix.len()..],
            (_, Some((open, close))) => text
                .strip_prefix(open.as_str())
                .map(|t| t.strip_suffix(close.as_str()).unwrap_or(t))
                .unwrap_or(text),
            _ => text,
        };
        body.trim()
    }

    pub fn is_trivia(&self, kind: SyntaxKind) -> bool {
        self.flags_of(kind) & 1 != 0
    }

    /// All kinds (tokens first, then nodes) carrying `tag`, in id order.
    pub fn kinds_with_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a KindDef> {
        self.tokens
            .iter()
            .chain(&self.nodes)
            .filter(move |d| d.has_tag(tag))
    }

    pub fn name(&self, kind: SyntaxKind) -> &str {
        if kind == SyntaxKind::ERROR {
            return "ERROR";
        }
        self.def(kind).map_or("?", |d| d.name.as_str())
    }
}
