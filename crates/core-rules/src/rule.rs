//! The rule trait and the contexts rules run in.
//!
//! concept: none (infrastructure: rule trait)

use crate::diagnostic::{Category, Diagnostic, Edit, Fix, FixKind, Severity};
use crate::hooks::RuleHooks;
use crate::layout::{LayoutRole, LayoutSink};
use crate::lines::LineIndex;
use crate::options::{Check, OptionDecl, RuleOptions};
use crate::project::{ProjectContext, ProjectFacts};
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
    /// The rule as the config's JSON Schema describes it.
    pub fn schema(&self) -> RuleSchema {
        RuleSchema {
            id: self.id.to_string(),
            description: self.description.to_string(),
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
    /// The language's `format.rules` text: a rule that rewrites code reads its rows in the section
    /// named like it.
    pub rows: &'a str,
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
            rows: file.hooks.rows(),
        }
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

    /// Project scope, phase 1: gather facts from each file.
    fn collect(&self, _file: &FileContext<'_>, _facts: &mut ProjectFacts) {}

    /// Project scope, phase 2: report across files.
    fn finish(&self, _facts: &ProjectFacts, _ctx: &mut ProjectContext<'_>) {}
}
