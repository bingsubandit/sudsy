//! Code of another language held by the scalar at a key path of a data file (`run:` of a workflow
//! step is shell): the spec lists the key paths, where the language word is read from (the code's
//! own mapping, then its ancestors' defaults) and what the host takes out of the code. The
//! file is visited once for all the paths the rows read; the regions of the file read last are kept,
//! because the engine asks for them to parse the code and then each rule that reads the host's own
//! text asks again (three searches per file otherwise). The regions are the same [`Embedded`] the `embed` rows
//! of a grammar find; a region whose code no parser reads (a folded scalar, an unknown shell) has the language `""`: rules read its text.
//!
//! concept: embedded-code
//! what: Code of another language inside a file, reported by the host provider and parsed by that language.

use crate::hooks::RuleHooks;
use crate::regex::{Meta, meta_regex};
use core_syntax::embedded::size;
use core_syntax::escaped::Escaped;
use core_syntax::{Embedded, Grammar, SyntaxElement, SyntaxNode, SyntaxNodeExt, TextRange};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// One key path whose scalar is code. `*` stands for one key or sequence index; `$N` in the
/// other paths is what the N-th `*` matched.
pub struct KeyEmbed {
    pub path: &'static str,
    /// Paths of the scalar that names the language, the first that is set wins.
    pub words: &'static [&'static str],
    /// When no word is set: a path whose value, in lower case, contains the text gives the
    /// language.
    pub guess: Option<(&'static str, &'static str, &'static str)>,
    pub language: &'static str,
    /// The code is only code when the scalar at this path starts with the text.
    pub when: Option<(&'static str, &'static str)>,
    /// Option values of the language that the host sets for this code.
    pub settings: &'static [(&'static str, &'static str)],
    /// The row's language is the default only when nothing is at this path; else no language.
    pub unless: Option<&'static str>,
}

pub struct KeyEmbeds {
    pub rows: &'static [KeyEmbed],
    /// The language for the first word of a language word; any other word is no known language.
    pub languages: &'static [(&'static [&'static str], &'static str)],
    /// What the host takes out of the code (regex over the region).
    pub carve: &'static str,
    /// Token kinds: a plain scalar, a block scalar's header and the text that starts a literal one.
    pub plain: &'static str,
    pub block_header: &'static str,
    pub literal: &'static str,
    /// The code is held in escaped string literals (a string, or an array of them: JSON), not in
    /// a scalar: the regions decode them (`core_syntax::escaped`) and `format` lets the language
    /// rewrite them.
    pub escaped: bool,
    pub format: bool,
    /// Text that is not code of the row's language: a line that matches the first regex and not
    /// the second (a notebook's `%%bash` cell magic).
    pub foreign: Option<(&'static str, &'static str)>,
}

struct Reader<'a> {
    g: &'a Grammar,
    hooks: &'a dyn RuleHooks,
    text: &'a str,
}

impl Reader<'_> {
    fn key(&self, entry: &SyntaxNode) -> Option<String> {
        let key = self.g.slot(entry, "key")?;
        Some(self.hooks.key_name(self.g, &key, self.text).into_owned())
    }

    /// The node under an anchor or tag, else the node itself.
    fn inner(&self, node: &SyntaxNode) -> SyntaxNode {
        match self.g.slot(node, "content") {
            Some(SyntaxElement::Node(n)) => self.inner(&n),
            _ => node.clone(),
        }
    }

    /// Visits the values on the way to the patterns once, with their key paths: what the rows
    /// read (the code, its language word, the runner, the action) is looked up in `seen`.
    fn collect(
        &self,
        node: &SyntaxNode,
        path: &mut Vec<String>,
        patterns: &[Vec<String>],
        seen: &mut Seen,
    ) {
        let node = self.inner(node);
        if patterns.iter().any(|p| matches(p, path)) && !seen.nodes.contains_key(path) {
            seen.order.push(path.clone());
            seen.nodes.insert(path.clone(), node.clone());
        }
        let mut visit = |name: String, value: Option<SyntaxNode>| {
            path.push(name);
            let wanted = patterns
                .iter()
                .any(|p| p.len() >= path.len() && matches(&p[..path.len()], path));
            if let (true, Some(value)) = (wanted, value) {
                self.collect(&value, path, patterns, seen);
            }
            path.pop();
        };
        if self.g.has_tag(node.kind(), "mapping") {
            for entry in node
                .children()
                .filter(|c| self.g.has_tag(c.kind(), "key-value"))
            {
                let value = self.g.slot(&entry, "value").and_then(|v| v.into_node());
                if let Some(key) = self.key(&entry) {
                    visit(key, value);
                }
            }
        } else if self.g.has_tag(node.kind(), "sequence") {
            for (i, item) in node.children().enumerate() {
                let value = match self.g.slot(&item, "value") {
                    Some(SyntaxElement::Node(v)) if self.g.has_field(item.kind(), "value") => {
                        Some(v)
                    }
                    _ => Some(item),
                };
                visit(i.to_string(), value);
            }
        }
    }

    /// The text of a scalar node: decoded for a quoted or plain one.
    fn word(&self, node: &SyntaxNode) -> Option<String> {
        let node = self.inner(node);
        node.tokens()
            .any(|t| self.g.has_tag(t.kind(), "string"))
            .then(|| {
                let element = SyntaxElement::Node(node.clone());
                self.hooks
                    .key_name(self.g, &element, self.text)
                    .into_owned()
            })
    }
}

/// The values reached by the key paths, in file order.
#[derive(Default)]
struct Seen {
    nodes: HashMap<Vec<String>, SyntaxNode>,
    order: Vec<Vec<String>>,
}

impl Seen {
    /// The value at `path`, where `$N` is what the N-th `*` of the row matched.
    fn at(&self, path: &str, caught: &[String]) -> Option<&SyntaxNode> {
        let path: Vec<String> = (path.split('.'))
            .map(
                |s| match s.strip_prefix('$').and_then(|n| n.parse::<usize>().ok()) {
                    Some(n) => caught.get(n - 1).cloned().unwrap_or_default(),
                    None => s.to_string(),
                },
            )
            .collect();
        self.nodes.get(&path)
    }
}

/// Does the pattern (`*` is any segment) match the whole path?
fn matches(pattern: &[String], path: &[String]) -> bool {
    pattern.len() == path.len() && pattern.iter().zip(path).all(|(p, s)| p == "*" || p == s)
}

/// The path as a pattern: `$N` is any segment.
fn pattern(path: &str) -> Vec<String> {
    (path.split('.'))
        .map(|s| if s.starts_with('$') { "*" } else { s }.to_string())
        .collect()
}

/// The first value node of the file: its mapping or sequence.
fn body(g: &Grammar, root: &SyntaxNode) -> Option<SyntaxNode> {
    let mut node = root.clone();
    while !(g.has_tag(node.kind(), "mapping") || g.has_tag(node.kind(), "sequence")) {
        node = node.children().next()?;
    }
    Some(node)
}

/// The regex of a spec, built once (the specs are static).
fn carve_regex(source: &'static str) -> std::sync::Arc<Meta> {
    static CACHE: std::sync::Mutex<Vec<(&'static str, std::sync::Arc<Meta>)>> =
        std::sync::Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, r)) = cache.iter().find(|(s, _)| *s == source) {
        return r.clone();
    }
    let regex = std::sync::Arc::new(meta_regex(source).expect("a valid pattern"));
    cache.push((source, regex.clone()));
    regex
}

type Found = (SyntaxNode, usize, Rc<Vec<Embedded>>);

thread_local! {
    /// The regions of the file read last: the engine asks for them to parse the code, then each
    /// rule that reads the host's own text asks again.
    static LAST: RefCell<Option<Found>> = const { RefCell::new(None) };
}

/// The code regions of `spec` in the file, found once for the file read last.
pub fn find(
    spec: &'static KeyEmbeds,
    g: &Grammar,
    hooks: &dyn RuleHooks,
    root: &SyntaxNode,
    text: &str,
) -> Rc<Vec<Embedded>> {
    let key = std::ptr::from_ref(spec) as usize;
    LAST.with_borrow_mut(|last| {
        if let Some((r, k, found)) = last
            && *k == key
            && r == root
        {
            return found.clone();
        }
        let found = Rc::new(search(spec, g, hooks, root, text));
        *last = Some((root.clone(), key, found.clone()));
        found
    })
}

fn search(
    spec: &KeyEmbeds,
    g: &Grammar,
    hooks: &dyn RuleHooks,
    root: &SyntaxNode,
    text: &str,
) -> Vec<Embedded> {
    let Some(file) = body(g, root) else {
        return Vec::new();
    };
    let carve = carve_regex(spec.carve);
    let r = Reader { g, hooks, text };
    let mut patterns = Vec::new();
    for row in spec.rows {
        let others = (row.words.iter().copied())
            .chain(row.guess.map(|g| g.0))
            .chain(row.when.map(|w| w.0))
            .chain(row.unless);
        patterns.extend(std::iter::once(row.path).chain(others).map(pattern));
    }
    let mut seen = Seen::default();
    r.collect(&file, &mut Vec::new(), &patterns, &mut seen);
    let mut out = Vec::new();
    for row in spec.rows {
        let own = pattern(row.path);
        for path in seen.order.iter().filter(|p| matches(&own, p)) {
            let caught: Vec<String> = (own.iter().zip(path))
                .filter(|(p, _)| *p == "*")
                .map(|(_, s)| s.clone())
                .collect();
            if let Some((when, start)) = row.when {
                let word = seen.at(when, &caught).and_then(|n| r.word(n));
                if !word.is_some_and(|w| w.starts_with(start)) {
                    continue;
                }
            }
            let language = language(spec, row, &r, &seen, &caught);
            let value = &seen.nodes[path];
            let found = if spec.escaped {
                escaped_region(spec, &r, value, language)
            } else {
                region(spec, &r, value, language, &carve)
            };
            out.extend(found.map(|mut e| {
                e.settings = (row.settings.iter())
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                e
            }));
        }
    }
    out.sort_by_key(|e| e.range.start());
    out
}

/// The language of the code of a row: its word, else a guess from the host, else the default; ""
/// when the word is none of the spec's.
fn language(
    spec: &KeyEmbeds,
    row: &KeyEmbed,
    r: &Reader<'_>,
    seen: &Seen,
    caught: &[String],
) -> &'static str {
    let word = row
        .words
        .iter()
        .find_map(|p| seen.at(p, caught).and_then(|n| r.word(n)));
    match word {
        Some(w) => {
            let first = w.split_whitespace().next().unwrap_or("");
            (spec.languages.iter())
                .find(|(words, _)| words.contains(&first))
                .map_or("", |(_, language)| language)
        }
        None => match row.guess {
            Some((path, contains, language))
                if seen
                    .at(path, caught)
                    .is_some_and(|n| r.text[n.text_range()].to_lowercase().contains(contains)) =>
            {
                language
            }
            _ if row.unless.is_some_and(|p| seen.at(p, caught).is_some()) => "",
            _ => row.language,
        },
    }
}

/// The region of a scalar value: its text, the block's indentation and what the host carves.
fn region(
    spec: &KeyEmbeds,
    r: &Reader<'_>,
    value: &SyntaxNode,
    language: &str,
    carve: &Meta,
) -> Option<Embedded> {
    let node = r.inner(value);
    let token = node.tokens().find(|t| r.g.has_tag(t.kind(), "string"))?;
    let range = token.text_range();
    let body = &r.text[range];
    let header = node
        .tokens()
        .find(|t| r.g.name(t.kind()) == spec.block_header);
    let code = match &header {
        Some(h) => r.text[h.text_range()].starts_with(spec.literal),
        None => r.g.name(token.kind()) == spec.plain && !body.contains('\n'),
    };
    let start = usize::from(range.start());
    let at = |i: usize| size(start + i);
    let mut carved = Vec::new();
    // A block scalar's lines start with the indentation of its first line.
    let indent = (body.lines())
        .find(|l| !l.trim().is_empty())
        .map_or(0, |l| l.len() - l.trim_start().len());
    if body.contains('\n') {
        let mut from = 0;
        for line in body.split_inclusive('\n') {
            let spaces = (line.len() - line.trim_start_matches(' ').len()).min(indent);
            if spaces > 0 {
                carved.push(TextRange::new(at(from), at(from + spaces)));
            }
            from += line.len();
        }
    }
    carved.extend(
        carve
            .find_iter(body)
            .map(|m| TextRange::new(at(m.start()), at(m.end()))),
    );
    Some(Embedded::new(
        if code { language } else { "" },
        range,
        carved,
    ))
}

/// The region of a string, or of an array of strings, whose code is the decoded text.
fn escaped_region(
    spec: &KeyEmbeds,
    r: &Reader<'_>,
    value: &SyntaxNode,
    language: &str,
) -> Option<Embedded> {
    let node = r.inner(value);
    let string = |n: &SyntaxNode| n.tokens().find(|t| r.g.has_tag(t.kind(), "string"));
    let tokens: Vec<_> = if r.g.has_tag(node.kind(), "sequence") {
        node.children().map(|c| string(&c)).collect::<Option<_>>()?
    } else {
        vec![string(&node)?]
    };
    let syntax = r.g.string_row(tokens.first()?.kind());
    let pieces = tokens.iter().map(|t| t.text_range()).collect();
    let list = r.g.has_tag(node.kind(), "sequence");
    let escaped = Escaped::new(syntax, pieces, list, r.text)?;
    let foreign = spec.foreign.is_some_and(|(lines, except)| {
        let (lines, except) = (carve_regex(lines), carve_regex(except));
        (escaped.code().lines()).any(|l| lines.is_match(l) && !except.is_match(l))
    });
    let mut region = Embedded::new(
        if foreign { "" } else { language },
        node.text_range(),
        Vec::new(),
    )
    .with_escaped(escaped);
    region.format = spec.format;
    Some(region)
}
