//! The `format-diff` family: a layout rule of a lint tool that the formatter enforces too. The
//! rule reports where formatting would break or join a line, or edit the text (a trailing
//! comma), inside the region of a node, so its findings are real and sit on the line the tool
//! reports. Params:
//! - `kind` (words): the kind names of the nodes the rule looks at;
//! - `until` (words, optional): the region of a node ends at its first child (node or token) of
//!   one of these kinds, that child's first character included; without it, the whole node;
//! - `through` (words, optional): kinds among those of `until` whose region also takes the first
//!   character after them (the body after `=`);
//! - `change` (words): `split` (a line break the formatter adds), `join` (one it removes), `edit`
//!   (code it adds, changes or removes), `comma` (a comma it adds or removes), `blank` (a blank
//!   line it adds or removes); default `split join`;
//! - `at` (`start`): only the changes at the first character of the node;
//! - `leading` (`comments`): the region also takes the comments right above the node (no blank
//!   line between them and it), so a blank line added before them is the node's;
//! - `multiline` (`true`): only a node that spans lines in the source;
//! - `skip` (words): kind names of nested nodes whose inside is not the region's (a lambda body in
//!   an argument list).
//!
//! The formatted text comes from the engine (`RuleContext::format_changes`); the rule does
//! nothing where the file does not format.
//!
//! concept: format-diff-family
//! what: Reports where the formatter would break, join or edit inside a node's region.
//! tools: ktlint function-signature, argument-list-wrapping, class-signature

use crate::FileContext;
use crate::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxElement, SyntaxNode, TextRange, TextSize};
use std::collections::BTreeSet;

pub const FAMILY: Family = Family {
    name: "format-diff",
    build,
};

/// How formatting changes the file at a place.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A line break is added before the code at the offset.
    Split = 1,
    /// The line break before the code at the offset is removed.
    Join = 2,
    /// Code other than a comma is added, changed or removed right after the code at the offset.
    Edit = 4,
    /// A comma is added or removed right after the code at the offset.
    Comma = 8,
    /// The number of line breaks before the code at the offset changes (a blank line added or
    /// removed), both being at least one.
    Blank = 16,
}

/// Where formatting changes the code of a file: the byte offset of the character the change
/// is at, and how. The formatted text is aligned to the source by its non-whitespace characters.
pub struct FormatChanges {
    flips: Vec<(u32, u8)>,
}

impl FormatChanges {
    pub fn new(source: &str, formatted: &str) -> FormatChanges {
        let (src, src_nl) = code_chars(source);
        let (fmt, fmt_nl) = code_chars(formatted);
        let before: Vec<char> = src.iter().map(|&(_, c)| c).collect();
        let after: Vec<char> = fmt.iter().map(|&(_, c)| c).collect();
        let mut flips = Vec::new();
        for op in similar::capture_diff_slices(similar::Algorithm::Myers, &before, &after) {
            let (tag, old, new) = op.as_tag_tuple();
            if tag == similar::DiffTag::Equal {
                // The first pair of a run follows code that changed: its gap says nothing.
                for k in 1..old.len() {
                    let (i, j) = (old.start + k, new.start + k);
                    let at = src[i].0;
                    match (src_nl[i], fmt_nl[j]) {
                        (0, f) if f > 0 => flips.push((at, Change::Split as u8)),
                        (n, 0) if n > 0 => flips.push((at, Change::Join as u8)),
                        (n, f) if n.min(2) != f.min(2) => flips.push((at, Change::Blank as u8)),
                        _ => {}
                    }
                }
            } else {
                // Code that goes is where the change is; code that comes in follows the code
                // before it.
                let at = if old.is_empty() {
                    old.start.checked_sub(1).and_then(|p| src.get(p))
                } else {
                    src.get(old.start)
                };
                let Some(at) = at else { continue };
                let comma = before[old.clone()]
                    .iter()
                    .chain(&after[new.clone()])
                    .all(|&c| c == ',');
                let kind = if comma { Change::Comma } else { Change::Edit };
                flips.push((at.0, kind as u8));
            }
        }
        flips.sort_unstable();
        FormatChanges { flips }
    }

    /// The changes of one of the kinds of `mask` at offsets from `start` to `end` inclusive.
    pub fn within(&self, start: u32, end: u32, mask: u8) -> impl Iterator<Item = u32> + '_ {
        let from = self.flips.partition_point(|&(at, _)| at < start);
        self.flips[from..]
            .iter()
            .take_while(move |&&(at, _)| at <= end)
            .filter(move |&&(_, kind)| kind & mask != 0)
            .map(|&(at, _)| at)
    }
}

/// The non-whitespace characters of a text with their byte offsets, and for each the number of
/// line breaks in the whitespace before it.
fn code_chars(text: &str) -> (Vec<(u32, char)>, Vec<u32>) {
    let mut chars = Vec::new();
    let mut breaks = Vec::new();
    let mut newlines = 0;
    for (at, c) in text.char_indices() {
        if c.is_whitespace() {
            newlines += u32::from(c == '\n');
        } else {
            chars.push((at as u32, c));
            breaks.push(newlines);
            newlines = 0;
        }
    }
    (chars, breaks)
}

struct FormatDiff {
    meta: &'static RuleMeta,
    message: String,
    kinds: Vec<String>,
    until: Vec<String>,
    through: Vec<String>,
    skip: Vec<String>,
    multiline: bool,
    at_start: bool,
    leading: bool,
    mask: u8,
}

fn words(p: &mut Params, key: &str) -> Vec<String> {
    p.optional(key)
        .map(|s| s.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let mut rule = FormatDiff {
        meta,
        message: p.text("message")?,
        kinds: words(&mut p, "kind"),
        until: words(&mut p, "until"),
        through: words(&mut p, "through"),
        skip: words(&mut p, "skip"),
        multiline: p.optional("multiline").is_some_and(|v| v == "true"),
        at_start: p.optional("at").is_some_and(|v| v == "start"),
        leading: p.optional("leading").is_some_and(|v| v == "comments"),
        mask: Change::Split as u8 | Change::Join as u8,
    };
    let change = words(&mut p, "change");
    if !change.is_empty() {
        rule.mask = 0;
        for w in &change {
            rule.mask |= match w.as_str() {
                "split" => Change::Split as u8,
                "join" => Change::Join as u8,
                "edit" => Change::Edit as u8,
                "comma" => Change::Comma as u8,
                "blank" => Change::Blank as u8,
                _ => return Err(format!("param `change`: unknown word `{w}`")),
            };
        }
    }
    if rule.kinds.is_empty() {
        return Err("param `kind` is missing".into());
    }
    p.done()?;
    Ok(Box::new(rule))
}

impl FormatDiff {
    /// The byte range of the node's region, its end the last offset a change may be at.
    fn region(&self, node: &SyntaxNode, file: &FileContext<'_>) -> (u32, u32) {
        let g = file.grammar;
        let range = crate::tree::significant_range(node, g);
        let (start, end) = (u32::from(range.start()), u32::from(range.end()));
        let whole = end.saturating_sub(1).max(start);
        if self.until.is_empty() {
            return (start, whole);
        }
        let stop = node.children_with_tokens().find_map(|e| {
            let (name, at, to) = match &e {
                SyntaxElement::Node(n) => (
                    g.name(n.kind()),
                    n.text_range().start(),
                    n.text_range().end(),
                ),
                SyntaxElement::Token(t) => (
                    g.name(t.kind()),
                    t.text_range().start(),
                    t.text_range().end(),
                ),
            };
            self.until.iter().any(|u| u == name).then(|| {
                if self.through.iter().any(|u| u == name) {
                    // The first code after the child.
                    let rest = &file.text[usize::from(to)..];
                    let gap = rest.len() - rest.trim_start().len();
                    u32::from(to) + gap as u32
                } else {
                    u32::from(at)
                }
                .max(start)
            })
        });
        (start, stop.unwrap_or(whole))
    }

    /// The offset of the first comment right above `node`, the node's own start when there is none.
    fn lead_start(&self, node: &SyntaxNode, start: u32, file: &FileContext<'_>) -> u32 {
        let g = file.grammar;
        let mut from = start;
        for t in node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if u32::from(t.text_range().start()) >= start {
                break;
            }
            if g.has_tag(t.kind(), "comment") {
                from = from.min(u32::from(t.text_range().start()));
            } else if t.text().matches('\n').count() > 1 {
                from = start;
            }
        }
        from
    }

    /// The ranges of the nested nodes of the `skip` kinds.
    fn skipped(&self, node: &SyntaxNode, file: &FileContext<'_>) -> Vec<(u32, u32)> {
        let g = file.grammar;
        if self.skip.is_empty() {
            return Vec::new();
        }
        node.descendants()
            .skip(1)
            .filter(|d| self.skip.iter().any(|k| k == g.name(d.kind())))
            .map(|d| {
                let r = crate::tree::significant_range(&d, g);
                (u32::from(r.start()), u32::from(r.end()))
            })
            .collect()
    }
}

impl Rule for FormatDiff {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn reads_formatted(&self) -> bool {
        true
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(changes) = ctx.format_changes else {
            return;
        };
        let g = file.grammar;
        let mut lines = BTreeSet::new();
        for node in file.root.descendants() {
            if !self.kinds.iter().any(|k| k == g.name(node.kind())) {
                continue;
            }
            let (start, end) = self.region(&node, file);
            if self.multiline && !file.text[crate::tree::significant_range(&node, g)].contains('\n')
            {
                continue;
            }
            let skipped = self.skipped(&node, file);
            let end = if self.at_start { start } else { end };
            let from = if self.leading {
                self.lead_start(&node, start, file)
            } else {
                start
            };
            for at in changes.within(from, end, self.mask) {
                if skipped.iter().any(|&(s, e)| at > s && at < e) {
                    continue;
                }
                if lines.insert(file.lines.line(TextSize::from(at))) {
                    ctx.report(
                        TextRange::new(at.into(), (at + 1).into()),
                        self.message.clone(),
                    );
                }
            }
        }
    }
}
