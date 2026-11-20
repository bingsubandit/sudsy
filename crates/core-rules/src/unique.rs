//! Names that must not repeat: [`Repeats`], the first-seen bookkeeping of the rules that report a
//! repeat against its first occurrence, and the `unique-names` family built on it.
//!
//! concept: unique-names
//! what: Names that must not repeat, reported against the first occurrence.
//! tools: eslint no-dupe-class-members, hadolint DL3024, checkmake unique-targets, markdownlint MD024, sqlfluff AL04
//! reads: family:unique-names

use crate::rule::{Rule, RuleContext, RuleMeta};
use crate::slot_path::{Found, SlotPath, green_children};
use crate::tree::significant_range;
use crate::{Family, Params};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange};
use rowan::GreenNodeData;
use rustc_hash::FxHashMap as HashMap;
use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::sync::OnceLock;

/// The first range of each key.
#[derive(Default)]
pub struct Repeats<K>(HashMap<K, TextRange>);

impl<K: Hash + Eq> Repeats<K> {
    /// Remembers `range` as the first of `key`; a repeat is reported at `range` with `message()`
    /// and the first one as `related`.
    pub fn check(
        &mut self,
        ctx: &mut RuleContext<'_>,
        key: K,
        range: TextRange,
        message: impl FnOnce() -> String,
        related: &str,
    ) {
        if let Some(first) = self.0.get(&key) {
            ctx.report(range, message()).related(*first, related);
        } else {
            self.0.insert(key, range);
        }
    }
}

pub const FAMILY: Family = Family {
    name: "unique-names",
    build,
};

/// `container`: the grammar tag of the nodes that hold the items; `item`: the tag of the items,
/// whose name (`name`, a [`SlotPath`]; the slot `name` by default) must not repeat among them.
/// `deep` finds the items below the container too, not entering an item or another container.
/// `group`: a second path whose text is part of the key (the same name in another group is not a
/// repeat). `fold: lower` compares without case, `strip` lists characters trimmed off both ends
/// of the name. `reset`: a name that starts with it and one that does not undo each other (a
/// later `!x` after `x` is no repeat of an earlier `!x`). `report: item` reports the item's code,
/// not its name; `lead` is a token kind that, found in the item before its code on the same line, starts the
/// report. `related`: the text of the first occurrence's note. `message` has `{name}` and
/// `{group}`.
struct UniqueNames {
    meta: &'static RuleMeta,
    message: String,
    tags: [&'static str; 1],
    item: String,
    /// The node kinds that carry the item tag, read from the grammar at the first check.
    item_kinds: OnceLock<Vec<SyntaxKind>>,
    deep: bool,
    name: SlotPath,
    group: Option<SlotPath>,
    /// The name and group paths start with the same label: one descent for both.
    shared_start: bool,
    lower: bool,
    strip: Vec<char>,
    reset: Option<String>,
    report_item: bool,
    lead: Option<String>,
    related: Option<String>,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let name = SlotPath::parse(&p.optional("name").unwrap_or_else(|| "name".into()))?;
    let group = p
        .optional("group")
        .map(|g| SlotPath::parse(&g))
        .transpose()?;
    let rule = UniqueNames {
        meta,
        shared_start: group.as_ref().is_some_and(|g| name.shares_start(g)),
        message: p.text("message")?,
        tags: [Box::leak(p.text("container")?.into_boxed_str())],
        item: p.text("item")?,
        item_kinds: OnceLock::new(),
        deep: p.flag("deep")?,
        name,
        group,
        lower: p.choice("fold", false, &[("none", false), ("lower", true)])?,
        strip: p.optional("strip").unwrap_or_default().chars().collect(),
        reset: p.optional("reset"),
        report_item: p.choice("report", false, &[("name", false), ("item", true)])?,
        lead: p.optional("lead"),
        related: p.optional("related"),
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl UniqueNames {
    fn is_item(&self, grammar: &Grammar, kind: SyntaxKind) -> bool {
        let kinds = self
            .item_kinds
            .get_or_init(|| grammar.nodes_with_tag(&self.item).to_vec());
        kinds.contains(&kind)
    }

    /// Calls `f` with the items below `node`: its children, and with `deep` what is below those
    /// that are neither items nor containers.
    fn each(&self, node: &SyntaxNode, grammar: &Grammar, f: &mut dyn FnMut(&SyntaxNode)) {
        for child in node.children() {
            if self.is_item(grammar, child.kind()) {
                f(&child);
            } else if self.deep && !grammar.has_tag(child.kind(), self.tags[0]) {
                self.each(&child, grammar, f);
            }
        }
    }
}

impl Rule for UniqueNames {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
    }

    fn tags(&self) -> &[&'static str] {
        &self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let grammar = ctx.file.grammar;
        if !self.deep && !self.report_item && self.lead.is_none() {
            return self.check_green(node, ctx);
        }
        // Most containers hold fewer than two items: count before reading any name.
        let mut n = 0;
        self.each(node, grammar, &mut |_| n += 1);
        if n < 2 {
            return;
        }
        let mut seen: Seen<'_> = HashMap::default();
        self.each(node, grammar, &mut |item| self.visit(item, ctx, &mut seen));
    }
}

type Seen<'t> = HashMap<(Cow<'t, str>, Cow<'t, str>), (TextRange, bool)>;

impl UniqueNames {
    /// [`Rule::check`] for the rules that read names from the green tree where the paths allow
    /// it: no red node per child. The first item waits until a second one shows that the
    /// container has a name worth reading.
    fn check_green(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let grammar = ctx.file.grammar;
        let green = node.green();
        let start = u32::from(node.text_range().start());
        let mut seen: Seen<'_> = HashMap::default();
        let mut first = None;
        let mut index = 0;
        for (at, child) in green_children(&green, start) {
            let rowan::NodeOrToken::Node(child) = child else {
                continue;
            };
            if !self.is_item(grammar, SyntaxKind(child.kind().0)) {
                continue;
            }
            index += 1;
            if index == 1 {
                first = Some((at, child));
                continue;
            }
            if let Some((at, child)) = first.take() {
                self.green_item(node, ctx, &mut seen, at, child, 1);
            }
            self.green_item(node, ctx, &mut seen, at, child, index);
        }
    }

    /// The `index`th item (from 1) of `node`, at `at` in the file.
    fn green_item<'t>(
        &self,
        node: &SyntaxNode,
        ctx: &mut RuleContext<'t>,
        seen: &mut Seen<'t>,
        at: u32,
        child: &GreenNodeData,
        index: usize,
    ) {
        let grammar = ctx.file.grammar;
        let text = ctx.file.text;
        let read = match &self.group {
            Some(g) if self.shared_start => self.name.find_green_with(g, grammar, child, at, text),
            Some(g) => self
                .name
                .find_green(grammar, child, at, text)
                .and_then(|n| {
                    let group = g.find_green(grammar, child, at, text)?;
                    Some(n.map(|n| (n, group)))
                }),
            None => self
                .name
                .find_green(grammar, child, at, text)
                .map(|n| n.map(|n| (n, None))),
        };
        match read {
            Some(Some((found, group))) => self.note(ctx, seen, found, group, None),
            Some(None) => {}
            None => {
                let item = node
                    .children()
                    .filter(|c| self.is_item(grammar, c.kind()))
                    .nth(index - 1);
                if let Some(item) = item {
                    self.visit(&item, ctx, seen);
                }
            }
        }
    }

    /// One item read from the red tree.
    fn visit<'t>(&self, item: &SyntaxNode, ctx: &mut RuleContext<'t>, seen: &mut Seen<'t>) {
        let grammar = ctx.file.grammar;
        let text = ctx.file.text;
        let Some(found) = self.name.find(grammar, item, text) else {
            return;
        };
        let group = self
            .group
            .as_ref()
            .and_then(|g| g.find(grammar, item, text));
        self.note(ctx, seen, found, group, Some(item));
    }

    /// Notes one name: reports it when an earlier item had it. `item` is needed for `report: item`
    /// and `lead`.
    fn note<'t>(
        &self,
        ctx: &mut RuleContext<'t>,
        seen: &mut Seen<'t>,
        found: Found<'t>,
        group: Option<Found<'t>>,
        item: Option<&SyntaxNode>,
    ) {
        let grammar = ctx.file.grammar;
        let text = ctx.file.text;
        let name: Cow<'_, str> = match found.text {
            Cow::Borrowed(s) => Cow::Borrowed(s.trim_matches(self.strip.as_slice())),
            Cow::Owned(s) => Cow::Owned(s.trim_matches(self.strip.as_slice()).to_string()),
        };
        let group = group.map_or(Cow::Borrowed(""), |g| g.text);
        let key = if self.lower {
            Cow::Owned(name.to_lowercase())
        } else {
            name.clone()
        };
        let mut range = found.range;
        if let Some(item) = item {
            if self.report_item {
                range = significant_range(item, grammar);
            }
            let lead = self.lead.as_deref().and_then(|k| grammar.kind_by_name(k));
            if let Some(kind) = lead
                && let Some(t) = item
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == kind)
                && t.text_range().start() < range.start()
                && !text[t.text_range().end().into()..range.start().into()].contains('\n')
            {
                range = TextRange::new(t.text_range().start(), range.end());
            }
        }
        let negated = self
            .reset
            .as_ref()
            .is_some_and(|r| name.starts_with(r.as_str()));
        if self.reset.is_some() {
            seen.retain(|_, (_, n)| *n == negated);
        }
        match seen.entry((group, key)) {
            Entry::Occupied(e) => {
                let message = fill(&self.message, &name, &e.key().0);
                let first = e.get().0;
                let d = ctx.report(range, message);
                if let Some(related) = &self.related {
                    d.related(first, related);
                }
            }
            Entry::Vacant(e) => {
                e.insert((range, negated));
            }
        }
    }
}

/// `template` with `{name}` and `{group}` filled in: one allocation.
fn fill(template: &str, name: &str, group: &str) -> String {
    let mut out = String::with_capacity(template.len() + name.len() + group.len());
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some(r) = rest.strip_prefix("{name}") {
            out.push_str(name);
            rest = r;
        } else if let Some(r) = rest.strip_prefix("{group}") {
            out.push_str(group);
            rest = r;
        } else {
            out.push('{');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}
