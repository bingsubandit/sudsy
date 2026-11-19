//! Code rewrites as data: a formatter that also rewrites code (a short array syntax, a default
//! visibility) lists token rows, and the source is edited before it is laid out.
//!
//! A line is `PATTERN : ACTION`, the pattern as in [`crate::spacing`] and the action one of
//! `replace "x"` (the token becomes `x`), `replace through next "x"` (the token and the next one,
//! with the blanks between them, become `x`), `insert before "x"` and `insert after "x"`. The
//! first row that matches a token is the one applied. Lines starting with `#` are comments.
//!
//! concept: token-rewrite-rows
//! what: Token rewrites (replace, insert) as rows applied to the source before layout.
//! tools: PHP-CS-Fixer short array syntax and visibility_required
//! reads: rules:rewrite

use crate::spacing::{EndSet, Pattern};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

enum Action {
    Replace(String),
    ReplaceThroughNext(String),
    InsertBefore(String),
    InsertAfter(String),
}

pub struct RewriteRules {
    rows: Vec<(Pattern, Action)>,
    /// By node kind: bit 0, a row looks at the tokens of a node of it (`parent=`); bit 1, a row
    /// looks at its last token (`end_of=`).
    anchors: Vec<u8>,
    /// Does every row have an anchor? Then only the tokens the anchors name are looked at,
    /// found while the caller walks the nodes anyway; else every token is.
    anchored: bool,
    /// The token kinds the `parent=` rows can match; `None`: any.
    parent_tokens: Option<Vec<core_syntax::SyntaxKind>>,
}

impl RewriteRules {
    pub fn parse(grammar: &Grammar, text: &str) -> RewriteRules {
        let rows = crate::rules::section(text, "rewrite")
            .map(|line| {
                let (pattern, action) = line
                    .split_once(" : ")
                    .unwrap_or_else(|| panic!("rewrite: `{line}`"));
                let (words, arg) = action
                    .split_once('"')
                    .unwrap_or_else(|| panic!("rewrite: `{line}`"));
                let arg = arg.trim_end_matches('"').to_string();
                let action = match words.trim() {
                    "replace" => Action::Replace(arg),
                    "replace through next" => Action::ReplaceThroughNext(arg),
                    "insert before" => Action::InsertBefore(arg),
                    "insert after" => Action::InsertAfter(arg),
                    other => panic!("rewrite: unknown action `{other}`"),
                };
                (Pattern::parse(grammar, pattern), action)
            })
            .collect();
        let rows: Vec<(Pattern, Action)> = rows;
        let mut anchors: Vec<u8> = Vec::new();
        let mut set = |kinds: Vec<core_syntax::SyntaxKind>, bit: u8| {
            for k in kinds {
                let at = k.index() as usize;
                if anchors.len() <= at {
                    anchors.resize(at + 1, 0);
                }
                anchors[at] |= bit;
            }
        };
        let mut anchored = true;
        let mut parent_tokens: Option<Vec<core_syntax::SyntaxKind>> = Some(Vec::new());
        for (p, _) in &rows {
            let (parents, ends) = (p.parent_kinds(), p.end_kinds());
            if !parents.is_empty() {
                match (p.kinds(), &mut parent_tokens) {
                    (Some(ks), Some(all)) => all.extend_from_slice(ks),
                    _ => parent_tokens = None,
                }
            }
            anchored &= !parents.is_empty() || !ends.is_empty();
            set(parents, 1);
            set(ends, 2);
        }
        RewriteRules {
            rows,
            anchors,
            anchored,
            parent_tokens,
        }
    }

    /// A scan of one tree: the caller shows it every node ([`RewriteScan::visit`], in any order)
    /// and takes the edits.
    pub fn scan(&self) -> RewriteScan<'_> {
        RewriteScan {
            rules: self,
            ends: EndSet::default(),
            tokens: Vec::new(),
        }
    }

    /// The source with the rows applied, `None` when no token matches.
    pub fn apply(&self, grammar: &Grammar, tree: &SyntaxNode, text: &str) -> Option<String> {
        let mut scan = self.scan();
        for n in tree.descendants() {
            scan.visit(&n);
        }
        scan.apply(grammar, tree, text)
    }

    /// Adds what the scan looks at to `wanted` ([`core_syntax::tree::find`]): the nodes that
    /// end something, and the tokens of the nodes a `parent=` row names ([`RewriteScan::add_token`]).
    pub fn wanted(&self, wanted: &mut core_syntax::tree::Wanted) {
        let set = |v: &mut Vec<bool>, at: usize| {
            if v.len() <= at {
                v.resize(at + 1, false);
            }
            v[at] = true;
        };
        for (i, f) in self.anchors.iter().enumerate() {
            if f & 1 != 0 {
                set(&mut wanted.parents, i);
            }
            if f & 2 != 0 {
                set(&mut wanted.nodes, i);
            }
        }
        match &self.parent_tokens {
            Some(kinds) => kinds
                .iter()
                .for_each(|k| set(&mut wanted.tokens, k.index() as usize)),
            None => wanted.any_token = true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The edits the rows make, `(from, to, text)` in document order: a language that rewrites
    /// more of its own in the same round merges them with its edits.
    pub fn edits(&self, grammar: &Grammar, tree: &SyntaxNode) -> Vec<(usize, usize, &str)> {
        let mut scan = self.scan();
        for n in tree.descendants() {
            scan.visit(&n);
        }
        scan.edits(grammar, tree)
    }
}

/// The rows' view of a tree while its nodes go by.
pub struct RewriteScan<'r> {
    rules: &'r RewriteRules,
    ends: EndSet,
    /// The tokens the anchors name (when every row has one).
    tokens: Vec<SyntaxToken>,
}

impl<'r> RewriteScan<'r> {
    /// A token `find` found for the rows ([`RewriteRules::wanted`]), instead of [`RewriteScan::visit`]
    /// of its parent.
    pub fn add_token(&mut self, token: SyntaxToken) {
        self.tokens.push(token);
    }

    pub fn visit(&mut self, node: &SyntaxNode) {
        let flags = self
            .rules
            .anchors
            .get(node.kind().index() as usize)
            .copied()
            .unwrap_or(0);
        if flags & 1 != 0 && self.rules.anchored {
            self.tokens
                .extend(node.children_with_tokens().filter_map(|e| e.into_token()));
        }
        if flags & 2 != 0 {
            self.visit_end(node);
        }
    }

    fn visit_end(&mut self, node: &SyntaxNode) {
        self.ends.add(node);
        if self.rules.anchored
            && let Some(l) = crate::spacing::last_code(node)
        {
            self.tokens.push(l);
        }
    }

    /// The source with the rows applied, `None` when no token matches.
    pub fn apply(self, grammar: &Grammar, tree: &SyntaxNode, text: &str) -> Option<String> {
        let edits = self.edits(grammar, tree);
        if edits.is_empty() {
            return None;
        }
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for (from, to, with) in edits {
            out.push_str(&text[at..from]);
            out.push_str(with);
            at = to;
        }
        out.push_str(&text[at..]);
        Some(out)
    }

    pub fn edits(mut self, grammar: &Grammar, tree: &SyntaxNode) -> Vec<(usize, usize, &'r str)> {
        let rules = self.rules;
        let mut edits: Vec<(usize, usize, &str)> = Vec::new();
        let mut one = |t: SyntaxToken| -> bool {
            if grammar.is_trivia(t.kind()) {
                return true;
            }
            let kind = t.kind();
            let Some((_, action)) = rules.rows.iter().find(|(p, _)| {
                p.kinds().is_none_or(|ks| ks.contains(&kind)) && p.matches_ends(&t, &self.ends)
            }) else {
                return true;
            };
            let (start, end) = (
                usize::from(t.text_range().start()),
                usize::from(t.text_range().end()),
            );
            edits.push(match action {
                Action::Replace(x) => (start, end, &x[..]),
                Action::InsertBefore(x) => (start, start, &x[..]),
                Action::InsertAfter(x) => (end, end, &x[..]),
                Action::ReplaceThroughNext(x) => match next_end(grammar, &t) {
                    Some(e) => (start, e, &x[..]),
                    None => return false,
                },
            });
            true
        };
        if rules.anchored {
            let mut tokens = std::mem::take(&mut self.tokens);
            tokens.sort_by_key(|t| t.text_range().start());
            tokens.dedup();
            for t in tokens {
                if !one(t) {
                    return Vec::new();
                }
            }
        } else {
            for t in tree
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
            {
                if !one(t) {
                    return Vec::new();
                }
            }
        }
        edits
    }
}

/// The end of the next token that is not whitespace or a comment.
fn next_end(grammar: &Grammar, t: &SyntaxToken) -> Option<usize> {
    let mut cur = t.next_token();
    while let Some(n) = cur {
        if !grammar.is_trivia(n.kind()) {
            return Some(usize::from(n.text_range().end()));
        }
        cur = n.next_token();
    }
    None
}

/// The `;` between and after statements, but not in a node of the kinds of `semicolon_keep_parents`:
/// one at the end of a line, in front of a `}`, an `else` or a comment is dropped, one between
/// two statements of a line becomes a line break. Edits `(from, to, text)`.
pub fn redundant_semicolons(
    grammar: &Grammar,
    tree: &SyntaxNode,
    text: &str,
    keep: &[String],
    edits: &mut Vec<(usize, usize, String)>,
) {
    // (a scan of the text finds the candidates; a walk of the tree for all tokens costs more)
    for t in text
        .match_indices(';')
        .filter_map(|(at, _)| {
            let at = u32::try_from(at).ok()?;
            tree.token_at_offset(at.into()).right_biased()
        })
        .filter(|t| t.text() == ";" && grammar.name(t.kind()).starts_with("Semi"))
    {
        if t.parent()
            .is_none_or(|p| keep.iter().any(|k| grammar.name(p.kind()) == k))
        {
            continue;
        }
        let (s, e) = (
            usize::from(t.text_range().start()),
            usize::from(t.text_range().end()),
        );
        let rest = text[e..].trim_start_matches([' ', '\t']);
        let ends_line = rest.is_empty()
            || rest.starts_with(['\n', '\r', '}', ')'])
            || rest.starts_with("//")
            || rest.starts_with("else");
        if ends_line {
            edits.push((s, e, String::new()));
        } else {
            edits.push((s, text.len() - rest.len(), "\n".into()));
        }
    }
}

/// Does the text have a line that starts with one of `keywords`, has a body after its
/// parenthesized head on the same line that does not start a `{` block, and is followed by a
/// deeper-indented line (a statement the layout wrapped, whose braces a rewrite that depends on
/// the width decides again)?
pub fn has_wrapped_statement(text: &str, keywords: &[String]) -> bool {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.next() {
        let t = l.trim_start();
        let Some(rest) = keywords.iter().find_map(|k| {
            t.strip_prefix(k.as_str())
                .filter(|r| r.starts_with([' ', '(']))
        }) else {
            continue;
        };
        let rest = rest.trim_start();
        let body = if rest.starts_with('(') {
            let mut depth = 0;
            let closed = rest.char_indices().find(|&(_, c)| {
                depth += i32::from(c == '(') - i32::from(c == ')');
                depth == 0
            });
            // (a head that goes on to the next line leaves the body to a later round)
            let Some((at, _)) = closed else { continue };
            &rest[at + 1..]
        } else {
            rest
        };
        let code = body.split_once("//").map_or(body, |(c, _)| c).trim();
        if code.is_empty()
            || code.starts_with(['{', '('])
            || code.ends_with('{')
            || code.starts_with("if ")
        {
            continue;
        }
        if lines
            .peek()
            .is_some_and(|n| !n.trim().is_empty() && indent(n) > indent(l))
        {
            return true;
        }
    }
    false
}
