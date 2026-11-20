//! Import statements in order: the entries of a run of `#import` nodes ([`entry`]), the order a
//! tool wants ([`order`]: groups, roots, compare, blank lines), the rule that reports a run that
//! is out of order (the `imports` family) and the fix that writes it back ([`layout`], used by the
//! formatters too).
//!
//! The family has two ways of reporting, `report: misplaced` (every statement that is out of
//! place, with the reason, and a fix that sorts the list) and `report: block` (one finding when
//! the block differs from the way isort writes it: [`block`]). Params: `report`, `message`,
//! `roots` (the path roots that come first with option `system_first`), `fixed` (modifiers that
//! keep a statement in place), `alias_by_name`. The options are the rule's own.
//!
//! concept: sorted-imports
//! what: Import statements ordered by group, name and case, with blank lines between groups.
//! tools: isort, google-java-format, ktlint import-ordering, StyleCop SA1210, PHP-CS-Fixer ordered_imports, buf format
//! reads: family:imports, grammar:imports

mod block;
pub mod entry;
mod layout;
mod misplaced;
mod order;

use crate::tree::{code_start, entry_range, reorder_edits, significant_range};
use crate::{Family, Params, Rule, RuleContext, RuleMeta};
pub use block::SECTIONS;
use core_syntax::{SyntaxNode, TextRange};
pub use layout::{sort_edits, sort_source};
pub use order::{Compare, Order, Separate, compare, words};
use std::marker::PhantomData;

/// What a language says about where its imports come from: the sections of an isort block.
pub trait ImportLang: 'static {
    /// The top-level names of the standard library, one to a line.
    const STDLIB: &'static str = "";
    /// The module of future statements.
    const FUTURE: &'static str = "";
    /// Files that mark the root of a project.
    const MARKERS: &'static [&'static str] = &[];
    /// Folders of the project root that hold modules besides the root itself.
    const SOURCE_DIRS: &'static [&'static str] = &[];
    /// The extension of a module file.
    const EXTENSION: &'static str = "";
}

/// A language with no sections.
pub struct Plain;

impl ImportLang for Plain {}

pub const fn family<L: ImportLang>() -> Family {
    Family {
        name: "imports",
        build: build::<L>,
    }
}

#[derive(Clone, Copy)]
enum Report {
    Misplaced,
    Block,
}

struct Imports<L> {
    meta: &'static RuleMeta,
    report: Report,
    message: String,
    roots: Vec<String>,
    fixed: Vec<String>,
    alias_by_name: bool,
    lang: PhantomData<fn() -> L>,
}

fn param_words(text: Option<String>) -> Vec<String> {
    text.unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn build<L: ImportLang>(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = Imports::<L> {
        meta,
        report: p.choice(
            "report",
            true,
            &[("misplaced", Report::Misplaced), ("block", Report::Block)],
        )?,
        message: p.optional("message").unwrap_or_default(),
        roots: param_words(p.optional("roots")),
        fixed: param_words(p.optional("fixed")),
        alias_by_name: p.flag("alias_by_name")?,
        lang: PhantomData,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl<L: ImportLang> Imports<L> {
    fn order(&self, ctx: &RuleContext<'_>) -> Order {
        let o = ctx.options;
        let compare = match o.string("compare") {
            "culture" => Compare::Culture,
            "culture_ignore_case" => Compare::CultureIgnoreCase,
            _ => Compare::Ordinal,
        };
        let mut order = Order::new(compare, &["*"]);
        if o.string("static_position") == "after" {
            order.categories.push("static".into());
        }
        if o.string("alias_position") == "after" {
            order.categories.push("alias".into());
        }
        if o.bool("system_first") {
            order.roots_first = self.roots.clone();
        }
        order.fixed = self.fixed.clone();
        order.alias_by_name = self.alias_by_name;
        order
    }

    fn misplaced(&self, run: &[SyntaxNode], ctx: &mut RuleContext<'_>) {
        let (text, g) = (ctx.file.text, ctx.file.grammar);
        let entries = entry::entries(run, text, g);
        let order = self.order(ctx);
        let kw = g.imports().keywords.first().map_or("", String::as_str);
        let found = misplaced::misplaced_entries(&entries, &order, kw);
        let (Some(first), Some(last)) = (entries.first(), entries.last()) else {
            return;
        };
        let span = TextRange::new(
            significant_range(&first.node, g).start(),
            entry_range(&last.node, g).end(),
        );
        let directives = text[span].lines().any(|l| l.trim_start().starts_with('#'));
        let sorted = order.sorted(&entries);
        let ranges: Vec<_> = entries.iter().map(|e| entry_range(&e.node, g)).collect();
        let mut fix = (!directives && sorted.iter().enumerate().any(|(i, &s)| i != s)).then(|| {
            (
                format!("Sort the {kw} directives"),
                reorder_edits(text, &ranges, &sorted),
            )
        });
        for m in found {
            let node = &entries[m.at].node;
            let range = TextRange::new(code_start(node, g), significant_range(node, g).end());
            ctx.report_fix(range, &m.message, fix.take());
        }
    }
}

impl<L: ImportLang> Rule for Imports<L> {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
    }

    fn tags(&self) -> &[&'static str] {
        &["import"]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let first = entry::item(node);
        if !entry::starts_run(&first, g) {
            return;
        }
        let run = entry::run(&first, g);
        match self.report {
            Report::Misplaced => self.misplaced(&run, ctx),
            Report::Block => {
                let sections: Vec<String> = match ctx.options.list("sections") {
                    s if s.is_empty() => SECTIONS.iter().map(|s| (*s).to_owned()).collect(),
                    s => s.into_iter().map(str::to_owned).collect(),
                };
                let style = block::Style {
                    sections: &sections,
                    force_sort_within_sections: ctx.options.bool("force_sort_within_sections"),
                    width: usize::try_from(ctx.options.int("line_width")).unwrap_or(88),
                };
                let range = block::misplaced::<L>(&run, ctx.file.text, ctx.file.path, g, &style);
                if let Some(range) = range {
                    ctx.report(range, self.message.clone());
                }
            }
        }
    }
}
