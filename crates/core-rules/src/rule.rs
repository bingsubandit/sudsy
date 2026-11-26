//! The rule trait and the contexts rules run in.
//!
//! concept: none (infrastructure: rule trait)

use crate::diagnostic::{Category, Diagnostic, Edit, Fix, FixKind, Severity};
use crate::hooks::RuleHooks;
use crate::layout::{LayoutRole, LayoutSink};
use crate::lines::LineIndex;
use crate::options::{Check, OptionDecl, RuleOptions};
use crate::project::{ProjectContext, ProjectFacts, Scope};
use crate::tree::TraversalCache;
use core_config::{RuleOptionSchema, RuleSchema};
use core_semantics::{BodyFlow, SemanticView};
use core_syntax::{Grammar, SyntaxNode, TextRange};
use std::path::Path;

pub use core_config::Level;
pub use core_semantics::Layer;
pub use core_syntax::SyntaxKind;

/// What `sudsy rules` shows and the engine reads about a rule.
#[derive(Debug)]
pub struct RuleMeta {
    pub id: &'static str,
    pub category: Category,
    pub description: &'static str,
    /// Why the rule exists: the harm it prevents or the convention it keeps. Empty while the
    /// rule has none (the list in `docs/ai/rules/undescribed.txt` only shrinks).
    pub why: &'static str,
    /// The text of the rule's file, when its `>-` blocks `description` and `why` are read from it
    /// on demand (a built-in pattern rule): the fields are then the one-line values, or empty.
    /// Empty for every other rule. Read through `description_text` and `why_text`.
    pub source: &'static str,
    /// Code that triggers the rule and what it should look like instead, for `sudsy rules <id>`.
    pub example: &'static str,
    /// The level when the config does not mention the rule.
    pub default_level: Level,
    /// The most trusted kind of fix the rule offers, if any.
    pub fix: Option<FixKind>,
    /// Layers without which the rule does not run.
    pub requires: &'static [Layer],
    /// Layers the rule uses when they are available, and does less without (a syntactic
    /// fallback at `fast`).
    pub uses: &'static [Layer],
    pub options: &'static [OptionDecl],
}

impl RuleMeta {
    /// The `description`, folded from the rule's file when it is a block there.
    pub fn description_text(&self) -> std::borrow::Cow<'static, str> {
        match crate::rule_text::block(self.source, "description") {
            Some(text) if !self.source.is_empty() => std::borrow::Cow::Owned(text),
            _ => std::borrow::Cow::Borrowed(self.description),
        }
    }

    /// The `why` the rule's own file or code gives, empty when it has none.
    pub fn own_why(&self) -> std::borrow::Cow<'static, str> {
        match crate::rule_text::block(self.source, "why") {
            Some(text) if !self.source.is_empty() => std::borrow::Cow::Owned(text),
            _ => std::borrow::Cow::Borrowed(self.why),
        }
    }

    /// The `why` of the rule: its own, or the text that every language's rule of this id shares
    /// (`texts/why.txt`). Empty while the rule has none. Never read while linting.
    pub fn why_text(&self) -> std::borrow::Cow<'static, str> {
        let own = self.own_why();
        if own.trim().is_empty() {
            crate::rule_text::shared_why(self.id).map_or(own, std::borrow::Cow::Owned)
        } else {
            own
        }
    }

    /// The first sentence of `description`, which stands alone: the rule list's column.
    pub fn first_sentence(&self) -> String {
        let description = self.description_text();
        let first = description.lines().next().unwrap_or("");
        let end = first
            .match_indices(". ")
            .map(|(i, _)| i)
            .find(|&i| !first[..i].ends_with('.'));
        end.map_or(first, |i| &first[..=i]).to_string()
    }

    /// What the editor, the schema and the SARIF table show: the first sentence of a rule that has
    /// its `why`, the whole `description` of one that still has the older one-paragraph text.
    pub fn summary(&self) -> String {
        if self.why_text().trim().is_empty() {
            self.description_text().into_owned()
        } else {
            self.first_sentence()
        }
    }

    /// The rule as the config's JSON Schema describes it.
    pub fn schema(&self) -> RuleSchema {
        RuleSchema {
            id: self.id.to_string(),
            description: self.summary(),
            options: self
                .options
                .iter()
                .map(|o| RuleOptionSchema {
                    name: o.name.to_string(),
                    description: o.description.to_string(),
                    default: o.default.value(),
                    choices: match o.check {
                        Check::OneOf(values) => values.iter().map(|s| (*s).to_string()).collect(),
                        _ => Vec::new(),
                    },
                })
                .collect(),
        }
    }
}

/// One file as rules see it.
pub struct FileContext<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub root: &'a SyntaxNode,
    pub grammar: &'a Grammar,
    pub lines: &'a LineIndex,
    /// The provider's methods, lambdas and local functions: body-scope rules are called once for
    /// each node of these kinds.
    pub body_kinds: &'a [SyntaxKind],
    /// What the provider's semantic model says about the file, when the file's rules need the
    /// layers and the depth allows them.
    pub semantics: Option<SemanticView<'a>>,
    /// The language's answers to the shared rules' questions about keys and scalars.
    pub hooks: &'a dyn RuleHooks,
    /// The parsed code of other languages in the file, for rules that read it (a Dockerfile's
    /// `RUN` as shell).
    pub embedded: &'a [EmbeddedTree],
    /// The cells of a program run piece by piece (a notebook's code cells), in order; empty for a
    /// plain file. Rules with per-cell semantics read them: an import block ends at a cell's end,
    /// a cell's last expression is its output, top-level `await` runs in the kernel's loop.
    pub cells: &'a [TextRange],
}

/// The number of the cell of [`FileContext::cells`] that holds the code of `node` (its leading
/// comments may belong to the cell before); 0 for a plain file.
pub fn cell_of(cells: &[TextRange], node: &SyntaxNode, g: &Grammar) -> usize {
    if cells.is_empty() {
        return 0;
    }
    let at = crate::tree::code_start(node, g);
    cells.partition_point(|c| c.end() <= at)
}

/// The formatter's side of a file for [`Rule::formats`] rules: its options.
#[derive(Clone, Copy)]
pub struct FormatInput<'a> {
    pub options: &'a core_config::ResolvedOptions,
}

impl<'a> RuleContext<'a> {
    /// What a rule makes of its rows, made once per language (the key is the rows' text, which is
    /// static) and shared by the files.
    pub fn parsed<T: std::any::Any + Send + Sync>(
        &self,
        rule: &'static str,
        make: impl FnOnce() -> T,
    ) -> std::sync::Arc<T> {
        type Store = std::sync::Mutex<
            std::collections::HashMap<
                (usize, usize, &'static str),
                std::sync::Arc<dyn std::any::Any + Send + Sync>,
            >,
        >;
        static STORE: std::sync::OnceLock<Store> = std::sync::OnceLock::new();
        let key = (self.rows.as_ptr() as usize, self.rows.len(), rule);
        let mut store = STORE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let found = store
            .entry(key)
            .or_insert_with(|| std::sync::Arc::new(make()));
        found
            .clone()
            .downcast()
            .unwrap_or_else(|_| panic!("rows of `{rule}` change type"))
    }

    /// The rows of the section named `rule` in the language's `format.rules`.
    pub fn section<'r>(&self, rule: &'r str) -> impl Iterator<Item = &'a str> + use<'a, 'r> {
        core_format::rules::section(self.rows, rule)
    }
}

/// The parse of a region of the file written in another language: its offsets count from the
/// start of `range`.
pub struct EmbeddedTree {
    pub range: TextRange,
    /// The language's name (`shell`).
    pub language: String,
    pub root: SyntaxNode,
}

/// A method, lambda or local function, with its flow graph when the flow layer is available.
pub struct BodyContext<'a> {
    pub file: &'a FileContext<'a>,
    pub node: &'a SyntaxNode,
    pub flow: Option<BodyFlow<'a>>,
}

/// Where a rule reports to. The rule id, category and severity come from the rule and its
/// configuration, never from the rule's code.
pub struct RuleContext<'a> {
    pub file: &'a FileContext<'a>,
    pub options: &'a RuleOptions,
    pub(crate) meta: &'static RuleMeta,
    pub(crate) severity: Severity,
    pub(crate) out: &'a mut Vec<Diagnostic>,
    pub(crate) layout: Option<&'a mut LayoutSink>,
    pub(crate) ranges: Option<&'a TraversalCache>,
    /// What the formatter hands the rules it runs before layout (`None` in a lint run).
    pub format: Option<FormatInput<'a>>,
    /// What formatting would change in the file, for rules that say [`Rule::reads_formatted`]; `None`
    /// when no rule asks or the file does not format.
    pub format_changes: Option<&'a crate::format_diff::FormatChanges>,
    /// The language's `format.rules` text: a rule that rewrites code reads its rows in the section
    /// named like it.
    pub rows: &'a str,
    /// For the lead of a [`Rule::cluster`]: the metadata and severity of every active rule of it.
    pub(crate) members: &'a [(&'static RuleMeta, Severity)],
}

impl<'a> RuleContext<'a> {
    /// The nodes of the file of the given kinds, in document order (the traversal shares one walk
    /// among the rules that ask).
    pub fn nodes(&self, kinds: &[core_syntax::SyntaxKind]) -> Vec<core_syntax::SyntaxNode> {
        match self.ranges {
            Some(cache) => cache.nodes(self.file.root, kinds),
            None => self
                .file
                .root
                .descendants()
                .filter(|n| kinds.contains(&n.kind()))
                .collect(),
        }
    }

    pub fn new(
        file: &'a FileContext<'a>,
        options: &'a RuleOptions,
        meta: &'static RuleMeta,
        severity: Severity,
        out: &'a mut Vec<Diagnostic>,
    ) -> Self {
        RuleContext {
            file,
            options,
            meta,
            severity,
            out,
            layout: None,
            ranges: None,
            format: None,
            format_changes: None,
            rows: file.hooks.rows(),
            members: &[],
        }
    }

    /// The active rules of the cluster this rule leads (see [`Rule::cluster`]), the lead included.
    pub fn members(&self) -> &[(&'static RuleMeta, Severity)] {
        self.members
    }

    /// Reports for `member`, one of [`RuleContext::members`], in its name and severity.
    pub fn report_as(
        &mut self,
        member: (&'static RuleMeta, Severity),
        range: TextRange,
        message: impl Into<String>,
    ) {
        let (meta, severity) = member;
        self.out.push(Diagnostic {
            rule: meta.id.to_string(),
            category: meta.category,
            severity,
            message: message.into(),
            range,
            related: Vec::new(),
            fixes: Vec::new(),
        });
    }

    /// The cache of [`crate::tree::significant_range`] shared by the rules run on one node (the
    /// engine sets it).
    pub fn ranges(&self) -> Option<&'a TraversalCache> {
        self.ranges
    }

    /// Is this the first time in this file that this rule asks about `range`? Always true outside
    /// the engine's traversal.
    pub fn first_visit(&self, range: TextRange) -> bool {
        self.ranges.is_none_or(|c| c.first(self.meta.id, range))
    }

    /// Has this rule already reported exactly `message` in this context?
    pub fn has_reported(&self, message: &str) -> bool {
        self.out
            .iter()
            .any(|d| d.message == message && d.rule == self.meta.id)
    }

    /// Reports a diagnostic; add related spans and fixes to the returned value.
    pub fn report(&mut self, range: TextRange, message: impl Into<String>) -> &mut Diagnostic {
        self.out.push(Diagnostic {
            rule: self.meta.id.to_string(),
            category: self.meta.category,
            severity: self.severity,
            message: message.into(),
            range,
            related: Vec::new(),
            fixes: Vec::new(),
        });
        self.out.last_mut().expect("just pushed")
    }

    /// [`report`](Self::report) with a fix `(title, edits)` of the kind the rule declares in its
    /// metadata (`fix:`); `None` reports without one.
    pub fn report_fix<T: Into<String>>(
        &mut self,
        range: TextRange,
        message: impl Into<String>,
        fix: impl Into<Option<(T, Vec<Edit>)>>,
    ) -> &mut Diagnostic {
        let kind = self.meta.fix;
        let diagnostic = self.report(range, message);
        if let Some((title, edits)) = fix.into() {
            let kind = kind.expect("a rule that fixes declares `fix`");
            diagnostic.fix(Fix::new(title, kind, edits));
        }
        diagnostic
    }
}

/// A rule. Implement whichever scopes the rule needs; all default to doing nothing. Rules hold
/// no state: options arrive through the context, project facts through [`ProjectFacts`].
pub trait Rule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;

    /// The name of a group of rules that read the same nodes through one analysis. The first
    /// active rule of the group is the only one that is run (once per node, with
    /// [`RuleContext::members`]) and reports for all of them, in the name of each; the others are
    /// never called. The rules of a group must subscribe to the same kinds and tags.
    fn cluster(&self) -> Option<&'static str> {
        None
    }

    /// Whether the rule attaches layout directives (`RuleContext::attach`) and whether it also
    /// reports. A layout rule is also run when the file is formatted, with its diagnostics dropped.
    fn layout_role(&self) -> LayoutRole {
        LayoutRole::None
    }

    /// Whether the formatter runs the rule's fixes on the source before it lays the code out (a
    /// rewrite such as dropping an empty statement): the rule's data is the section named like it
    /// in the language's `format.rules`, no section means the language has none of it. Such a
    /// rule is `off` in lint unless the config turns it on (it reads its rows through `RuleContext::section`).
    fn formats(&self) -> bool {
        false
    }

    /// For a [`Rule::formats`] rule: the node kinds its `check_file` asks for with
    /// `RuleContext::nodes`. The formatter finds them in its one walk of the nodes and hands them
    /// to the rule, so the rule needs no walk of its own.
    fn format_kinds(&self, _grammar: &core_syntax::Grammar) -> Vec<SyntaxKind> {
        Vec::new()
    }

    /// For a [`Rule::formats`] rule: a cheap look at the text (a substring scan, no tree) that
    /// says whether the rule can have anything to fix. When it says no, the formatter neither
    /// walks for the rule's kinds nor runs it. `own`: the rule's options.
    fn format_applies(
        &self,
        _grammar: &Grammar,
        _text: &str,
        _own: &RuleOptions,
        _format: &core_config::ResolvedOptions,
    ) -> bool {
        true
    }

    /// Node scope: the kinds [`Rule::check`] is called for.
    fn kinds(&self) -> &[SyntaxKind] {
        &[]
    }

    /// Node scope by grammar tag: [`Rule::check`] is also called for every node whose kind
    /// carries one of these tags in the file's grammar (`mapping`, `sequence`), so a rule about a
    /// shared concept runs in every language that tags it.
    fn tags(&self) -> &[&'static str] {
        &[]
    }

    /// Why the rule could not run on the last file, when it was skipped (a built-in pattern rule
    /// whose patterns need the language's worker and could not compile). Reported once per run
    /// like any other unavailable analysis.
    fn unavailable(&self) -> Option<String> {
        None
    }

    /// Whether the rule reads the parsed code of other languages in the file
    /// (`FileContext::embedded`). The code is parsed for the rules of its own language, and for
    /// the host's rules only when one of them says so.
    fn reads_embedded(&self) -> bool {
        false
    }

    /// Whether the rule reads the symbol index ([`Rule::check_index`]): only then does a run build
    /// one.
    fn reads_index(&self) -> bool {
        false
    }

    /// Index scope: once per file, after the others, with what the index knows about the names
    /// the file writes.
    fn check_index(&self, _index: &dyn crate::index_rules::IndexView, _ctx: &mut RuleContext<'_>) {}

    /// Whether the rule reads the formatter's output (`FileContext::format_changes`).
    fn reads_formatted(&self) -> bool {
        false
    }

    /// Whether [`Rule::check_body`] does anything: a rule that returns false is not called at body
    /// nodes. The default is true; a rule with no body check says false to cost nothing there.
    fn bodies(&self) -> bool {
        true
    }

    fn check(&self, _node: &SyntaxNode, _ctx: &mut RuleContext<'_>) {}

    /// File scope: once per file.
    fn check_file(&self, _file: &FileContext<'_>, _ctx: &mut RuleContext<'_>) {}

    /// Body scope: once per method, lambda or local function of the provider.
    fn check_body(&self, _body: &BodyContext<'_>, _ctx: &mut RuleContext<'_>) {}

    /// Where the rule's project phase looks: every file of the project at `analysis: deep`
    /// (default), or the files of one directory at every depth ([`Scope::Directory`]).
    fn scope(&self) -> Scope {
        Scope::Project
    }

    /// Project scope, phase 1: gather facts from each file.
    fn collect(&self, _file: &FileContext<'_>, _facts: &mut ProjectFacts) {}

    /// Project scope, phase 2: report across files.
    fn finish(&self, _facts: &ProjectFacts, _ctx: &mut ProjectContext<'_>) {}
}
