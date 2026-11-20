//! A formatter that is only data: one rules file per language with the sections `[space]`,
//! `[joined]`, `[at_least_one]`, `[op_first]` and `[flow]` ([`crate::spacing`]), `[rewrite]`
//! ([`crate::rewrite`]), `[case]` ([`crate::case`]), `[lines]` ([`crate::frames`]), `[layout]` ([`crate::layout`]) and `[blank]`
//! ([`crate::blank`]: rows `PREV / NEXT [: group=KIND]` say between which statements or members a
//! blank line is required). [`LazyRules`] turns the file into the language's `NodeFormatter`; the
//! language's [`RuleFacts`] say what no row does.
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
use crate::rewrite::{RewriteRules, RewriteScan};
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

/// What a language says about how its rules file is run, as a table of the language.
#[derive(Clone, Copy, Debug)]
pub struct RuleFacts {
    /// The flow keeps what the source has where no row decides ([`crate::shapes::flow::Flow`]).
    pub exact: bool,
    /// A line break in the source starts a hanging part ([`crate::shapes::flow::Flow`]).
    pub source_hang: bool,
    /// Blank lines in the source stay ([`crate::shapes::flow::Flow`]).
    pub keep_blank_lines: bool,
    /// Kinds after which a hanging part ends ([`crate::shapes::flow::Flow`]).
    pub unhang: &'static [&'static str],
    /// Patterns (as in `[space]` rows) of the tokens that start a line when a line breaks at them.
    pub op_first: &'static [&'static str],
    /// A file of one line stays as written.
    pub one_line_verbatim: bool,
    /// A file with a token of these kinds stays as written.
    pub verbatim_if: &'static [&'static str],
    /// A file whose last token is of these kinds keeps its end.
    pub exact_end: &'static [&'static str],
    /// Most blank lines kept after a comment (`None`: one).
    pub max_blank_after: Option<usize>,
    /// Blanks before a trailing comment stay when there are more than the style's gap.
    pub keep_comment_gap: bool,
    /// Block comments that end with one of these (`=*/`) have no blank before the token after them.
    pub glued_comments: &'static [&'static str],
}

impl RuleFacts {
    pub const DEFAULT: RuleFacts = RuleFacts {
        exact: false,
        source_hang: false,
        keep_blank_lines: false,
        unhang: &[],
        op_first: &[],
        one_line_verbatim: false,
        verbatim_if: &[],
        exact_end: &[],
        max_blank_after: None,
        keep_comment_gap: false,
        glued_comments: &[],
    };
}

impl Default for RuleFacts {
    fn default() -> Self {
        RuleFacts::DEFAULT
    }
}

pub struct RuleFormatter {
    space: SpaceRules,
    brackets: Brackets,
    cases: Cases,
    /// The `[call]` text, parsed when a call asks for it.
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
    keep_comment_gap: bool,
    glued_comments: &'static [&'static str],
}

impl RuleFormatter {
    pub fn parse(g: &Grammar, text: &'static str, facts: &RuleFacts) -> RuleFormatter {
        RuleFormatter::parse_with(g, text, facts, &[])
    }

    /// [`RuleFormatter::parse`] with the kind sets of the language's options.
    pub fn parse_with(
        g: &Grammar,
        text: &'static str,
        facts: &RuleFacts,
        option_kinds: OptionKinds,
    ) -> RuleFormatter {
        let bindings: Vec<Binding> = option_kinds
            .iter()
            .map(|(option, role, word, names)| Binding {
                option: core_config::opt::must(option),
                role: role.to_string(),
                word: word.to_string(),
                kinds: names
                    .iter()
                    .map(|n| {
                        g.kind_by_name(n)
                            .unwrap_or_else(|| panic!("option `{option}`: kind `{n}`"))
                    })
                    .collect(),
            })
            .collect();
        let kinds = |names: &[&str]| -> Vec<SyntaxKind> {
            names
                .iter()
                .map(|n| {
                    g.kind_by_name(n)
                        .unwrap_or_else(|| panic!("rules: kind `{n}`"))
                })
                .collect()
        };
        let layout: Vec<&str> = section(text, "layout").collect();
        RuleFormatter {
            space: SpaceRules::parse(g, text, facts, &bindings),
            brackets: Brackets::parse(g, text),
            cases: Cases::parse(g, section(text, "case")),
            call_text: text,
            calls: OnceLock::new(),
            rewrite: RewriteRules::parse(g, text),
            layout: Layout::parse(g, &layout, bindings),
            blank: Blank::parse(g, section(text, "blank")),
            file: File {
                one_line: facts.one_line_verbatim,
                any: kinds(facts.verbatim_if),
            },
            exact_end: kinds(facts.exact_end),
            exact: facts.exact,
            source_hang: facts.source_hang,
            keep_blank_lines: facts.keep_blank_lines,
            max_blank_after: facts.max_blank_after.unwrap_or(1),
            keep_comment_gap: facts.keep_comment_gap,
            glued_comments: facts.glued_comments,
        }
    }

    /// The `[call]` rows.
    pub fn calls(&self, g: &Grammar) -> &CallRules {
        self.calls
            .get_or_init(|| CallRules::parse(g, self.call_text, &Default::default()))
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
            keep_comment_gap: self.keep_comment_gap,
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

    /// The `[rewrite]` rows, when there are any.
    pub fn rewrite_rules(&self) -> Option<&RewriteRules> {
        (!self.rewrite.is_empty()).then_some(&self.rewrite)
    }

    /// A scan for the `[rewrite]` rows ([`crate::rewrite::RewriteRules::scan`]): a language that
    /// walks the nodes itself shows them to it and merges the edits with its own.
    pub fn rewrite_scan(&self) -> crate::rewrite::RewriteScan<'_> {
        self.rewrite.scan()
    }

    pub fn comment_style(&self) -> CommentStyle {
        CommentStyle {
            max_blank_after: self.max_blank_after,
            keep_gap: self.keep_comment_gap,
            keep_attached: self.keep_comment_gap,
            glued: self.glued_comments,
            ..CommentStyle::default()
        }
    }
}

/// A language's formatter: the rules file parsed on first use. A language declares
/// `pub static NAME: LazyRules = LazyRules::new(|| grammars::X.get(), include_str!("..."));`.
pub struct LazyRules {
    grammar: fn() -> &'static Grammar,
    text: &'static str,
    facts: RuleFacts,
    option_kinds: OptionKinds,
    cell: OnceLock<RuleFormatter>,
}

/// The kind sets of the language's options, `(option, role, word, kinds)` (`kinds:` of an option in
/// `lang.yaml`): the constructs an option governs, which the shared shapes read (the roles are
/// listed in docs/ai/formatting/engine.md).
pub type OptionKinds = &'static [(
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
)];

/// A kind set of an option, with the grammar's kinds resolved.
#[derive(Clone)]
pub struct Binding {
    pub option: core_config::OptId,
    pub role: String,
    pub word: String,
    pub kinds: Vec<SyntaxKind>,
}

impl LazyRules {
    pub const fn new(grammar: fn() -> &'static Grammar, text: &'static str) -> LazyRules {
        LazyRules {
            grammar,
            text,
            facts: RuleFacts::DEFAULT,
            option_kinds: &[],
            cell: OnceLock::new(),
        }
    }

    /// The kind sets of the language's options.
    pub const fn option_kinds(mut self, kinds: OptionKinds) -> LazyRules {
        self.option_kinds = kinds;
        self
    }

    /// The language's [`RuleFacts`].
    pub const fn facts(mut self, facts: RuleFacts) -> LazyRules {
        self.facts = facts;
        self
    }

    pub fn get(&self) -> &RuleFormatter {
        self.cell.get_or_init(|| {
            RuleFormatter::parse_with((self.grammar)(), self.text, &self.facts, self.option_kinds)
        })
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

    fn rewrite_rows(&self) -> Option<&RewriteRules> {
        self.get().rewrite_rules()
    }

    fn rewrite_scanned(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &Grammar,
        _: &ResolvedOptions,
        scan: Option<RewriteScan<'_>>,
    ) -> Option<String> {
        match scan {
            Some(s) => s.apply(grammar, tree, text),
            None => self.get().rewrite_source(grammar, tree, text),
        }
    }

    /// The facts' style; a language that declares `spaces_before_trailing_comments` and
    /// `align_trailing_comments` has them: the gap is that many blanks, and the comments of
    /// consecutive lines start in one column when the second is true.
    fn comment_style(&self, o: &ResolvedOptions) -> CommentStyle {
        let mut style = self.get().comment_style();
        if o.get(core_config::opt::SPACES_BEFORE_TRAILING_COMMENTS)
            .is_some()
        {
            style.gap = o.uint(core_config::opt::SPACES_BEFORE_TRAILING_COMMENTS, 1);
            style.keep_gap = false;
            style.align_lines =
                o.flag(core_config::opt::ALIGN_TRAILING_COMMENTS, false) && style.gap > 0;
            style.align_within_width = true;
        }
        style
    }
}
