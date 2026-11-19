//! A formatter that is only data: one rules file per language with the sections `[space]`,
//! `[joined]`, `[at_least_one]`, `[op_first]` and `[flow]` ([`crate::spacing`]), `[rewrite]`
//! ([`crate::rewrite`]), `[case]` ([`crate::case`]), `[lines]` ([`crate::frames`]), `[layout]` ([`crate::layout`]), `[blank]`, `[file]` and `[comments]`
//! (here). [`LazyRules`] turns the file into the language's `NodeFormatter`.
//!
//! `[flow]`: `exact`, `source_hang` and `keep_blank_lines` (`yes`/`no`, see
//! [`crate::shapes::flow::Flow`]) besides the kind lists. `[file]`: `one_line_verbatim = yes` (a
//! file of one line stays as written), `verbatim_if = A|B` (so does a file with a token of those
//! kinds), `exact_end = A|B` (a file whose last token is of those kinds keeps its end).
//! `[comments]`: `max_blank_after = unlimited|N`. `[blank]` rows `PREV / NEXT [: group=KIND]` say
//! between which statements or members a blank line is required ([`crate::blank`]).
//!
//! concept: rules-file-formatter
//! what: A formatter that is only data: one rules file per language parsed against the grammar.
//! tools: none (internal)

use crate::FormatCtx;
use crate::blank::Blank;
use crate::calls::CallRules;
use crate::case::Cases;
use crate::comments::CommentStyle;
use crate::context::NodeFormatter;
use crate::doc::Doc;
use crate::error::FormatError;
use crate::frames::Brackets;
use crate::layout::{File, Layout};
use crate::rewrite::RewriteRules;
use crate::seq::kids;
use crate::shapes::flow::Flow;
use crate::spacing::SpaceRules;
use core_config::ResolvedOptions;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};
use std::sync::OnceLock;

/// The lines of a section: trimmed, without blank lines and `#` comments.
pub fn section<'a>(text: &'a str, name: &str) -> impl Iterator<Item = &'a str> {
    let mut inside = false;
    text.lines().map(str::trim).filter(move |l| {
        if let Some(n) = l.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = n == name;
            return false;
        }
        inside && !l.is_empty() && !l.starts_with('#')
    })
}

/// The `key = value` lines of a section.
pub fn settings<'a>(text: &'a str, name: &str) -> Vec<(&'a str, &'a str)> {
    section(text, name)
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect()
}

pub struct RuleFormatter {
    space: SpaceRules,
    brackets: Brackets,
    cases: Cases,
    /// The `[call]`, `[hug]` and `[simple]` text, parsed when a call asks for it.
    call_text: &'static str,
    calls: OnceLock<CallRules>,
    rewrite: RewriteRules,
    layout: Layout,
    blank: Blank,
    file: File,
    exact_end: Vec<SyntaxKind>,
    exact: bool,
    source_hang: bool,
    keep_blank_lines: bool,
    max_blank_after: usize,
}

impl RuleFormatter {
    pub fn parse(g: &Grammar, text: &'static str) -> RuleFormatter {
        let yes = |settings: &[(&str, &str)], key: &str| {
            settings.iter().any(|(k, v)| *k == key && *v == "yes")
        };
        let kinds = |v: &str| -> Vec<SyntaxKind> {
            v.split('|')
                .map(|n| {
                    g.kind_by_name(n.trim())
                        .unwrap_or_else(|| panic!("rules: kind `{n}`"))
                })
                .collect()
        };
        let get = |settings: &[(&str, &str)], key: &str| {
            settings
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| kinds(v))
                .unwrap_or_default()
        };
        let (flow, file, comments) = (
            settings(text, "flow"),
            settings(text, "file"),
            settings(text, "comments"),
        );
        let layout: Vec<&str> = section(text, "layout").collect();
        RuleFormatter {
            space: SpaceRules::parse(g, text),
            brackets: Brackets::parse(g, text),
            cases: Cases::parse(g, section(text, "case")),
            call_text: text,
            calls: OnceLock::new(),
            rewrite: RewriteRules::parse(g, text),
            layout: Layout::parse(g, &layout),
            blank: Blank::parse(g, section(text, "blank")),
            file: File {
                one_line: yes(&file, "one_line_verbatim"),
                any: get(&file, "verbatim_if"),
            },
            exact_end: get(&file, "exact_end"),
            exact: yes(&flow, "exact"),
            source_hang: yes(&flow, "source_hang"),
            keep_blank_lines: yes(&flow, "keep_blank_lines"),
            max_blank_after: match comments.iter().find(|(k, _)| *k == "max_blank_after") {
                Some((_, "unlimited")) => usize::MAX,
                Some((_, n)) => n.parse().unwrap_or(1),
                None => 1,
            },
        }
    }

    /// The `[call]`, `[simple]` and `[hug]` rows.
    pub fn calls(&self, g: &Grammar) -> &CallRules {
        self.calls
            .get_or_init(|| CallRules::parse(g, self.call_text))
    }

    /// The `[case]` rows.
    pub fn cases(&self) -> &Cases {
        &self.cases
    }

    /// The `[lines]` kinds that open and close a frame.
    pub fn brackets(&self) -> &Brackets {
        &self.brackets
    }

    /// The token spacing rows.
    pub fn space(&self) -> &SpaceRules {
        &self.space
    }

    pub fn flow(&self, source_hang: bool) -> Flow<'_> {
        Flow {
            want: |_, _| None,
            flat_after: |_| false,
            closers: &[],
            unhang: &[],
            joined: |_, _| false,
            at_least_one: |_, _| false,
            op_first: |_| false,
            exact: self.exact,
            source_hang,
            keep_blank_lines: self.keep_blank_lines,
            rules: Some(&self.space),
        }
    }

    pub fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let (flow, level) = (self.flow(self.source_hang), self.flow(false));
        match self.layout.format(cx, node, (&flow, &level), &self.file) {
            Some(r) => r.map(Some),
            None => flow.seq(cx, &kids(cx, node)).map(Some),
        }
    }

    /// The node laid out by a `[layout]` row with the language's own `flow`, `None` when no row
    /// matches (a language that formats most nodes in Rust and only some by rows).
    pub fn layout_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
        flow: &Flow<'_>,
    ) -> Option<Result<Doc, FormatError>> {
        self.layout.format(cx, node, (flow, flow), &self.file)
    }

    pub fn blank_between(&self, cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
        self.blank.between(cx, prev, next)
    }

    pub fn exact_end(&self, g: &Grammar, root: &SyntaxNode) -> bool {
        if self.file.is_one_line(root) {
            return true;
        }
        let mut last = root.last_token();
        while let Some(t) = last {
            if self.exact_end.contains(&t.kind()) {
                return true;
            }
            if !g.is_trivia(t.kind()) && !t.text().is_empty() {
                return false;
            }
            last = t.prev_token();
        }
        false
    }

    pub fn rewrite_source(&self, g: &Grammar, tree: &SyntaxNode, text: &str) -> Option<String> {
        self.rewrite.apply(g, tree, text)
    }

    pub fn comment_style(&self) -> CommentStyle {
        CommentStyle {
            max_blank_after: self.max_blank_after,
            ..CommentStyle::default()
        }
    }
}

/// A language's formatter: the rules file parsed on first use. A language declares
/// `pub static NAME: LazyRules = LazyRules::new(|| grammars::X.get(), include_str!("..."));`.
pub struct LazyRules {
    grammar: fn() -> &'static Grammar,
    text: &'static str,
    cell: OnceLock<RuleFormatter>,
}

impl LazyRules {
    pub const fn new(grammar: fn() -> &'static Grammar, text: &'static str) -> LazyRules {
        LazyRules {
            grammar,
            text,
            cell: OnceLock::new(),
        }
    }

    pub fn get(&self) -> &RuleFormatter {
        self.cell
            .get_or_init(|| RuleFormatter::parse((self.grammar)(), self.text))
    }
}

impl NodeFormatter for LazyRules {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        self.get().format_node(node, cx)
    }

    fn blank_between(&self, cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
        self.get().blank_between(cx, prev, next)
    }

    fn exact_end(&self, root: &SyntaxNode) -> bool {
        self.get().exact_end((self.grammar)(), root)
    }

    fn rows(&self) -> &'static str {
        self.text
    }

    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &Grammar,
        _: &ResolvedOptions,
    ) -> Option<String> {
        self.get().rewrite_source(grammar, tree, text)
    }

    fn comment_style(&self, _: &ResolvedOptions) -> CommentStyle {
        self.get().comment_style()
    }
}
