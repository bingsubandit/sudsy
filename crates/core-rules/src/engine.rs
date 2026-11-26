//! The rule engine: one traversal per file, dispatching nodes to the rules subscribed to their
//! kind, then the file-scope and body-scope calls, then suppressions.
//!
//! concept: none (infrastructure: rule engine)

use crate::diagnostic::{Diagnostic, Severity};
use crate::layout::LayoutSink;
use crate::options::RuleOptions;
use crate::project::{DirectoryScope, ProjectContext, ProjectFacts, Scope};
use crate::rule::{BodyContext, FileContext, Rule, RuleContext, RuleMeta};
use crate::suppress::Suppressions;
use core_semantics::Layers;
use core_syntax::SyntaxKind;
use std::path::{Path, PathBuf};

/// A rule that is on for a file: its severity and checked options.
pub struct ActiveRule<'a> {
    pub rule: &'a dyn Rule,
    pub severity: Severity,
    pub options: RuleOptions,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileReport {
    /// Sorted by position, then rule.
    pub diagnostics: Vec<Diagnostic>,
    /// What the rules collected for the project phase.
    pub facts: ProjectFacts,
}

/// The layers the active rules need: what the provider is asked to answer for the file.
pub fn layers_needed(active: &[ActiveRule<'_>]) -> Layers {
    let mut layers = Layers::default();
    for a in active {
        let meta = a.rule.meta();
        for &layer in meta.requires.iter().chain(meta.uses) {
            layers.insert(layer);
        }
    }
    layers
}

/// Runs `active` over one file and drops what a suppression comment silences. `collect` says
/// whether project facts are wanted (analysis depth `deep`).
pub fn run_file(file: &FileContext<'_>, active: &[ActiveRule<'_>], collect: bool) -> FileReport {
    let mut report = run_rules(file, active, collect);
    silence(file, &mut report.diagnostics);
    report
}

/// Drops what a suppression comment of the file silences.
pub fn silence(file: &FileContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let suppressions = Suppressions::scan(file);
    diagnostics.retain(|d| !suppressions.suppresses(file, d));
}

/// [`run_file`] without the suppressions: for a caller whose own caller applies them to the
/// result (a plugin, whose host does).
pub fn run_rules(file: &FileContext<'_>, active: &[ActiveRule<'_>], collect: bool) -> FileReport {
    run_rules_with(file, active, collect, None)
}

/// [`run_rules`] with what formatting would change in the file, for the rules that say
/// [`Rule::reads_formatted`].
pub fn run_rules_with(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
    collect: bool,
    changes: Option<&crate::format_diff::FormatChanges>,
) -> FileReport {
    traverse(file, active, collect, None, changes)
}

/// [`run_file`] with what formatting would change in the file.
pub fn run_file_with(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
    collect: bool,
    changes: Option<&crate::format_diff::FormatChanges>,
) -> FileReport {
    let mut report = run_rules_with(file, active, collect, changes);
    silence(file, &mut report.diagnostics);
    report
}

/// The diagnostics of the rules' [`Rule::check_file`] alone, with no walk of the nodes for the
/// rules that subscribe to kinds: the rules the formatter runs before layout (`Rule::formats`)
/// look at the file as a whole.
pub fn run_file_rules<'a>(
    file: &'a FileContext<'a>,
    active: &[&ActiveRule<'_>],
    format: crate::rule::FormatInput<'a>,
    index: Option<Vec<Vec<core_syntax::SyntaxNode>>>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let indexed = index.is_some();
    let ranges = match index {
        Some(i) => crate::tree::TraversalCache::with_index(i),
        None => crate::tree::TraversalCache::default(),
    };
    for a in active {
        let mut ctx = RuleContext::new(
            file,
            &a.options,
            a.rule.meta(),
            a.severity,
            &mut diagnostics,
        );
        // (the shared index of nodes by kind pays only when several rules ask)
        ctx.ranges = (indexed || active.len() > 1).then_some(&ranges);
        ctx.format = Some(format);
        a.rule.check_file(file, &mut ctx);
    }
    diagnostics.sort_by(|a, b| {
        (a.range.start(), a.range.end(), &a.rule).cmp(&(b.range.start(), b.range.end(), &b.rule))
    });
    diagnostics
}

/// A group of rules run as one ([`Rule::cluster`]): its name and the active members.
type Cluster = (&'static str, Vec<(&'static RuleMeta, Severity)>);

/// The rules subscribed to one node kind, in rule order: the first few inline, so a kind with
/// few rules costs no allocation per file.
#[derive(Default)]
struct Subscribed {
    len: usize,
    inline: [usize; 4],
    more: Vec<usize>,
}

impl Subscribed {
    fn push(&mut self, rule: usize) {
        let last = match self.len {
            0 => None,
            n if n > self.inline.len() => self.more.last(),
            n => self.inline.get(n - 1),
        };
        if last == Some(&rule) {
            return;
        }
        match self.inline.get_mut(self.len) {
            Some(slot) => *slot = rule,
            None => self.more.push(rule),
        }
        self.len += 1;
    }

    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        let inline = self.len.min(self.inline.len());
        self.inline[..inline].iter().chain(&self.more).copied()
    }
}

/// The traversal behind [`run_rules`] and [`crate::layout::run_layout`]; `layout` is where the
/// rules' `attach` calls go (they are ignored without it).
pub(crate) fn traverse(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
    collect: bool,
    mut layout: Option<&mut LayoutSink>,
    changes: Option<&crate::format_diff::FormatChanges>,
) -> FileReport {
    let mut diagnostics = Vec::new();
    // Sorted by kind: a handful of kinds, looked up once per node, so a binary search beats hashing.
    let mut by_kind: Vec<(SyntaxKind, Subscribed)> = Vec::new();
    // The groups of rules that share one run per node: the lead (first active) and the members.
    let mut clusters: Vec<Cluster> = Vec::new();
    for (i, a) in active.iter().enumerate() {
        if let Some(name) = a.rule.cluster() {
            let member = (a.rule.meta(), a.severity);
            match clusters.iter_mut().find(|c| c.0 == name) {
                Some(c) => {
                    c.1.push(member);
                    continue;
                }
                None => clusters.push((name, vec![member])),
            }
        }
        // A tag resolves through the file's grammar to the node kinds carrying it.
        let tagged = a
            .rule
            .tags()
            .iter()
            .flat_map(|t| file.grammar.nodes_with_tag(t));
        for &kind in a.rule.kinds().iter().chain(tagged) {
            match by_kind.binary_search_by_key(&kind, |e| e.0) {
                Ok(at) => by_kind[at].1.push(i),
                Err(at) => {
                    let mut rules = Subscribed::default();
                    rules.push(i);
                    by_kind.insert(at, (kind, rules));
                }
            }
        }
    }
    let ranges = crate::tree::TraversalCache::default();
    let mut run = |index: usize, f: &mut dyn FnMut(&dyn Rule, &mut RuleContext<'_>)| {
        let a = &active[index];
        let mut ctx = RuleContext::new(
            file,
            &a.options,
            a.rule.meta(),
            a.severity,
            &mut diagnostics,
        );
        ctx.layout = layout.as_deref_mut();
        if let Some(name) = a.rule.cluster() {
            ctx.members = clusters.iter().find(|c| c.0 == name).map_or(&[], |c| &c.1);
        }
        ctx.ranges = Some(&ranges);
        ctx.format_changes = changes;
        f(a.rule, &mut ctx);
    };
    for node in file.root.descendants() {
        if let Ok(at) = by_kind.binary_search_by_key(&node.kind(), |e| e.0) {
            for i in by_kind[at].1.iter() {
                run(i, &mut |rule, ctx| rule.check(&node, ctx));
            }
        }
        if file.body_kinds.contains(&node.kind()) {
            let flow = file.semantics.as_ref().and_then(|view| {
                let range = crate::tree::significant_range(&node, file.grammar);
                view.body(range.start().into(), range.len().into())
            });
            let body = BodyContext {
                file,
                node: &node,
                flow,
            };
            for (i, a) in active.iter().enumerate() {
                if a.rule.bodies() {
                    run(i, &mut |rule, ctx| rule.check_body(&body, ctx));
                }
            }
        }
    }
    for i in 0..active.len() {
        run(i, &mut |rule, ctx| rule.check_file(file, ctx));
    }
    diagnostics.sort_by(|a, b| {
        (a.range.start(), a.range.end(), &a.rule).cmp(&(b.range.start(), b.range.end(), &b.rule))
    });
    crate::cells::except_cell_tail(file, active, &mut diagnostics);

    let mut facts = ProjectFacts::default();
    if collect {
        for a in active.iter().filter(|a| a.rule.scope() == Scope::Project) {
            facts.begin(a.rule.meta().id, file.path);
            a.rule.collect(file, &mut facts);
        }
        facts.end();
    }
    FileReport { diagnostics, facts }
}

/// The project phase: every active rule sees the facts gathered from all files.
pub fn run_project(active: &[ActiveRule<'_>], facts: &ProjectFacts) -> Vec<(PathBuf, Diagnostic)> {
    let mut out = Vec::new();
    for a in active.iter().filter(|a| a.rule.scope() == Scope::Project) {
        let mut ctx = ProjectContext {
            options: &a.options,
            meta: a.rule.meta(),
            files: &[],
            complete: true,
            severity: a.severity,
            out: &mut out,
        };
        a.rule.finish(facts, &mut ctx);
    }
    out
}

/// The directory rules among `active`.
pub fn directory_rules<'a, 'b>(
    active: &'b [ActiveRule<'a>],
) -> impl Iterator<Item = (&'b ActiveRule<'a>, DirectoryScope)> {
    active.iter().filter_map(|a| match a.rule.scope() {
        Scope::Directory(d) => Some((a, d)),
        Scope::Project => None,
    })
}

/// Phase 1 of the directory rules among `active`, on one file of the unit.
pub fn collect_directory(file: &FileContext<'_>, active: &[ActiveRule<'_>]) -> ProjectFacts {
    let mut facts = ProjectFacts::default();
    for (a, _) in directory_rules(active) {
        facts.begin(a.rule.meta().id, file.path);
        a.rule.collect(file, &mut facts);
    }
    facts.end();
    facts
}

/// Phase 1 of the directory rules for a file of the unit that gives no facts (a generated file):
/// it counts as read.
pub fn collect_nothing(path: &Path, active: &[ActiveRule<'_>]) -> ProjectFacts {
    let mut facts = ProjectFacts::default();
    for (a, _) in directory_rules(active) {
        facts.begin(a.rule.meta().id, path);
    }
    facts.end();
    facts
}

/// Phase 2 of the directory rules among `active` over the facts of the unit's `files`. `opaque`:
/// the directory holds files that were not read. A rule reports on any file of the unit; the
/// caller keeps the ones it was asked for.
pub fn run_directory(
    active: &[ActiveRule<'_>],
    facts: &ProjectFacts,
    files: &[PathBuf],
    opaque: bool,
) -> Vec<(PathBuf, Diagnostic)> {
    let mut out = Vec::new();
    for (a, _) in directory_rules(active) {
        let id = a.rule.meta().id;
        let complete = !opaque && files.iter().all(|f| facts.collected(id, f));
        let mut ctx = ProjectContext {
            options: &a.options,
            meta: a.rule.meta(),
            files,
            complete,
            severity: a.severity,
            out: &mut out,
        };
        a.rule.finish(facts, &mut ctx);
    }
    out
}
