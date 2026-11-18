//! The import statements of a tree as entries: what each one imports, read from the words of its
//! tokens and the grammar's `imports` line (keywords, modifiers, alias words, member list word).
//!
//! concept: sorted-imports
//! what: Import statements ordered by group, name and case, with blank lines between groups.
//! tools: isort, google-java-format, ktlint import-ordering, StyleCop SA1210, PHP-CS-Fixer ordered_imports, buf format
//! reads: family:imports, grammar:imports

use crate::tree::{code_start, is_code};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode};

/// One imported path (a statement with several, `import a, b`, is several entries).
#[derive(Debug)]
pub struct Entry {
    /// The statement, with the wrapper node around an `#import` node of the same extent.
    pub node: SyntaxNode,
    /// The words before the path that are not part of it (`static`, `global`, `public`).
    pub modifiers: Vec<String>,
    /// The path or module, without blanks and without the dots of a relative one.
    pub path: String,
    /// The leading dots of a relative module (`from .. import x` is 2).
    pub level: usize,
    pub alias: Option<String>,
    /// The names after the path (`from m import a, b as c`) with their aliases.
    pub members: Vec<(String, Option<String>)>,
    /// A trailing comma in parentheses after the members.
    pub magic: bool,
    /// The statement has a member list (even an empty one).
    pub from: bool,
    /// A preprocessor directive stands before it (filled by [`entries`]).
    pub directive_before: bool,
}

impl Entry {
    pub fn has(&self, modifier: &str) -> bool {
        self.modifiers.iter().any(|m| m == modifier)
    }

    /// The path and the alias, which are what makes two imports the same.
    pub fn same_as(&self, other: &Entry) -> bool {
        self.modifiers == other.modifiers && self.path == other.path && self.alias == other.alias
    }
}

/// The statement of an `#import` node: the parent that covers exactly the same text, if any
/// (Java wraps the import in an `ImportDeclaration`).
pub fn item(node: &SyntaxNode) -> SyntaxNode {
    let mut n = node.clone();
    while let Some(p) = n.parent() {
        if p.text_range() != n.text_range() || p.parent().is_none() {
            break;
        }
        n = p;
    }
    n
}

/// Is `item` the first of a run of import statements (the node before it, with only trivia
/// between, is not one)?
pub fn starts_run(item: &SyntaxNode, g: &Grammar) -> bool {
    let mut prev = item.prev_sibling_or_token();
    while let Some(p) = prev {
        match &p {
            SyntaxElement::Token(t) if g.is_trivia(t.kind()) => prev = p.prev_sibling_or_token(),
            SyntaxElement::Node(n) => return !is_import(n, g),
            _ => return true,
        }
    }
    true
}

/// Is the node an import statement (or the wrapper of one)?
pub fn is_import(node: &SyntaxNode, g: &Grammar) -> bool {
    g.has_tag(node.kind(), "import")
        || node
            .first_child()
            .is_some_and(|c| c.text_range() == node.text_range() && is_import(&c, g))
}

/// The run of import statements that starts at `first`.
pub fn run(first: &SyntaxNode, g: &Grammar) -> Vec<SyntaxNode> {
    let mut out = vec![first.clone()];
    let mut next = first.next_sibling_or_token();
    while let Some(n) = next {
        match &n {
            SyntaxElement::Token(t) if g.is_trivia(t.kind()) => {}
            SyntaxElement::Node(node) if is_import(node, g) => out.push(node.clone()),
            _ => break,
        }
        next = n.next_sibling_or_token();
    }
    out
}

fn is_word(list: &[String], w: &str) -> bool {
    list.iter().any(|x| x == w)
}

/// The words of `words` split at the commas outside brackets.
fn split(words: &[String]) -> Vec<&[String]> {
    let (mut out, mut depth, mut from) = (Vec::new(), 0i32, 0);
    for (i, w) in words.iter().enumerate() {
        match w.as_str() {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            w if !w.is_empty() && w.chars().all(|c| c == '>') => depth -= w.len() as i32,
            "," if depth == 0 => {
                out.push(&words[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    out.push(&words[from..]);
    out.retain(|p| !p.is_empty());
    out
}

/// A name and its alias, split at the word `list` names (before or after the name).
fn alias_of(words: &[String], g: &Grammar) -> (String, Option<String>) {
    let s = g.imports();
    let at = |list: &[String]| words.iter().position(|w| is_word(list, w));
    if let Some(i) = at(&s.alias_before) {
        return (words[i + 1..].concat(), Some(words[..i].concat()));
    }
    match at(&s.alias_after) {
        Some(i) => (words[..i].concat(), Some(words[i + 1..].concat())),
        None => (words.concat(), None),
    }
}

/// The entries of one import statement.
pub fn read(node: &SyntaxNode, g: &Grammar) -> Vec<Entry> {
    let syntax = g.imports();
    let words: Vec<String> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.text_range().is_empty() && is_code(t, g))
        .map(|t| t.text().to_string())
        .collect();
    let (mut at, mut modifiers) = (0, Vec::new());
    while let Some(w) = words.get(at) {
        let qualifier = words.get(at + 1).is_some_and(|n| n == "::");
        if is_word(&syntax.modifiers, w) && !qualifier {
            modifiers.push(w.clone());
        } else if !is_word(&syntax.keywords, w) {
            break;
        }
        at += 1;
    }
    let mut rest = &words[at..];
    while rest.last().is_some_and(|w| w == ";") {
        rest = &rest[..rest.len() - 1];
    }
    let make = |path: String, alias: Option<String>| Entry {
        node: node.clone(),
        modifiers: modifiers.clone(),
        path,
        level: 0,
        alias,
        members: Vec::new(),
        magic: false,
        from: false,
        directive_before: false,
    };
    let Some(m) = rest.iter().position(|w| is_word(&syntax.members, w)) else {
        return split(rest)
            .into_iter()
            .map(|item| {
                let (path, alias) = alias_of(item, g);
                make(path, alias)
            })
            .collect();
    };
    let head = &rest[..m];
    let dots = head
        .iter()
        .take_while(|w| w.chars().all(|c| c == '.'))
        .collect::<Vec<_>>();
    let mut entry = make(head[dots.len()..].concat(), None);
    entry.level = dots.iter().map(|w| w.len()).sum();
    entry.from = true;
    let mut tail = &rest[m + 1..];
    if tail.first().is_some_and(|w| w == "(") {
        entry.magic = tail.len() > 2 && tail[tail.len() - 2] == ",";
        tail = &tail[1..tail.len() - usize::from(tail.last().is_some_and(|w| w == ")"))];
    }
    entry.members = split(tail).into_iter().map(|m| alias_of(m, g)).collect();
    vec![entry]
}

/// The entries of a run of statements, each with whether a preprocessor directive stands before it.
pub fn entries(nodes: &[SyntaxNode], source: &str, g: &Grammar) -> Vec<Entry> {
    let mut out = Vec::new();
    for node in nodes {
        let from = usize::from(node.text_range().start());
        let to = usize::from(code_start(node, g));
        let directive = source[from..to].lines().any(|l| l.trim_start().starts_with('#'));
        for mut e in read(node, g) {
            e.directive_before = directive;
            out.push(e);
        }
    }
    out
}
