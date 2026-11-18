//! Writing a sorted list of import statements back into the source: every statement keeps the
//! comments on the lines above it and after it, the blank lines between statements go (an order
//! may ask for one between groups), and `#if` regions stay as they are behind the sorted
//! statements. A list with another preprocessor directive is left alone.
//!
//! concept: sorted-imports
//! what: Import statements ordered by group, name and case, with blank lines between groups.
//! tools: isort, google-java-format, ktlint import-ordering, StyleCop SA1210, PHP-CS-Fixer ordered_imports, buf format
//! reads: family:imports, grammar:imports

use super::entry::{self, Entry, item, starts_run};
use super::order::Order;
use crate::tree::{code_start, entry_range, significant_range};
use core_syntax::{Grammar, SyntaxNode, TextRange};

/// How one list is written: `#if` regions stay verbatim behind the sorted statements.
struct Plan {
    /// From the first statement (or `#if` region) to the end of the last.
    span: TextRange,
    /// Per entry: inside an `#if` region.
    in_region: Vec<bool>,
    /// Where the lead (comments above) of each entry starts, after any region before it.
    lead_from: Vec<usize>,
    /// The regions, `#if` line start to `#endif` line end, in source order.
    regions: Vec<(usize, usize)>,
}

#[derive(Default)]
struct Scan {
    depth: usize,
    start: usize,
    consumed: usize,
    regions: Vec<(usize, usize)>,
}

/// Walks the lines of `source[from..to]`, counting `#if` nesting. `None` for another directive
/// or an `#endif` without `#if`.
fn scan(source: &str, from: usize, to: usize, state: &mut Scan, first: bool) -> Option<()> {
    let mut at = from;
    for line in source[from..to].split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with("#if") {
            if state.depth == 0 {
                state.start = if first { at } else { from.max(state.consumed) };
            }
            state.depth += 1;
        } else if t.starts_with("#endif") {
            state.depth = state.depth.checked_sub(1)?;
            if state.depth == 0 {
                state.consumed = at + line.trim_end().len();
                state.regions.push((state.start, state.consumed));
            }
        } else if t.starts_with('#') && !t.starts_with("#el") {
            return None;
        }
        at += line.len();
    }
    Some(())
}

/// Splits the list at its `#if` regions. `None` for any other directive or an unbalanced `#if`.
fn plan(entries: &[Entry], source: &str, g: &Grammar, move_first: bool) -> Option<Plan> {
    let first = entries.first()?;
    let last = entries.last()?;
    let mut span = TextRange::new(
        significant_range(&first.node, g).start(),
        entry_range(&last.node, g).end(),
    );
    let mut st = Scan::default();
    let (mut in_region, mut lead_from) = (Vec::new(), Vec::new());
    let mut span_start = usize::from(span.start());
    let mut prev_end = 0;
    for (i, e) in entries.iter().enumerate() {
        let from = if i == 0 {
            usize::from(e.node.text_range().start())
        } else {
            prev_end
        }
        .max(st.consumed);
        scan(source, from, usize::from(code_start(&e.node, g)), &mut st, i == 0)?;
        if i == 0 && st.depth > 0 {
            span_start = st.start;
        } else if i == 0 && move_first {
            // The list starts at the first comment line above the statement.
            let lead = &source[from..usize::from(code_start(&e.node, g))];
            let mut at = 0;
            for line in lead.split_inclusive('\n') {
                if !line.trim().is_empty() {
                    span_start = from + at;
                    break;
                }
                at += line.len();
            }
        }
        lead_from.push(st.consumed.max(from));
        in_region.push(st.depth > 0);
        prev_end = usize::from(entry_range(&e.node, g).end());
    }
    let mut end = usize::from(span.end());
    if st.depth > 0 {
        let to = end + source[end..].find("#endif")? + 6;
        scan(source, end, to, &mut st, false)?;
        end = st.consumed.max(to);
    }
    span = TextRange::new(span_start.try_into().ok()?, end.try_into().ok()?);
    (st.depth == 0).then_some(Plan {
        span,
        in_region,
        lead_from,
        regions: st.regions,
    })
}

fn written(entries: &[Entry], plan: &Plan, source: &str, g: &Grammar, order: &Order) -> String {
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let bodies: Vec<TextRange> = entries.iter().map(|e| entry_range(&e.node, g)).collect();
    // The comment lines above each statement but the first, whose are left in front of the list.
    let leads: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            if i == 0 && !order.first_comment_moves {
                return String::new();
            }
            let to = usize::from(code_start(&e.node, g));
            source[plan.lead_from[i]..to]
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let t = l.trim_start();
                    let l = if t.starts_with("//") || t.starts_with("/*") {
                        t
                    } else {
                        l
                    };
                    format!("{}{newline}", l.trim_end())
                })
                .collect()
        })
        .collect();
    let main: Vec<usize> = order
        .sorted_unique(entries)
        .into_iter()
        .filter(|&i| !plan.in_region[i])
        .collect();
    let mut out = String::new();
    for (n, &i) in main.iter().enumerate() {
        if n > 0 {
            out.push_str(newline);
            if order.blank_between(&entries[main[n - 1]], &entries[i]) {
                out.push_str(newline);
            }
        }
        out.push_str(&leads[i]);
        out.push_str(&source[bodies[i]]);
    }
    for &(from, to) in &plan.regions {
        if !out.is_empty() {
            out.push_str(newline);
        }
        out.push_str(source[from..to].trim_start_matches(['\r', '\n']));
    }
    out
}

/// Does the text of the statements, or between them, hold a comment?
fn has_comment(nodes: &[SyntaxNode], source: &str, g: &Grammar) -> bool {
    let marks: Vec<&str> = g
        .comments
        .line
        .iter()
        .map(String::as_str)
        .chain(g.comments.block.iter().map(|b| b.0.as_str()))
        .collect();
    let (Some(first), Some(last)) = (nodes.first(), nodes.last()) else {
        return false;
    };
    let all = &source[first.text_range().start().into()..last.text_range().end().into()];
    marks.iter().any(|m| all.contains(m))
}

/// The edits that sort the runs of import statements of `root`: the text of a range and what it
/// becomes.
pub fn sort_edits(
    root: &SyntaxNode,
    source: &str,
    g: &Grammar,
    order: &Order,
) -> Vec<(TextRange, String)> {
    let mut edits = Vec::new();
    let mut walk = root.preorder();
    while let Some(event) = walk.next() {
        let core_syntax::WalkEvent::Enter(node) = event else {
            continue;
        };
        if g.has_tag(node.kind(), "type-declaration") || g.has_tag(node.kind(), "function") {
            walk.skip_subtree();
            continue;
        }
        if !g.has_tag(node.kind(), "import") {
            continue;
        }
        let first = item(&node);
        if !starts_run(&first, g) {
            continue;
        }
        let nodes = entry::run(&first, g);
        let entries = entry::entries(&nodes, source, g);
        if entries.len() != nodes.len() || (!order.keep_comments && has_comment(&nodes, source, g)) {
            continue;
        }
        let Some(plan) = plan(&entries, source, g, order.keep_comments && order.first_comment_moves) else {
            continue;
        };
        let new = written(&entries, &plan, source, g, order);
        if source[plan.span] != new {
            edits.push((plan.span, new));
        }
    }
    edits
}

/// `source` with the runs of import statements sorted by `order`, or `None` when they are.
pub fn sort_source(
    root: &SyntaxNode,
    source: &str,
    g: &Grammar,
    order: &Order,
) -> Option<String> {
    let edits = sort_edits(root, source, g, order);
    if edits.is_empty() {
        return None;
    }
    let mut out = source.to_string();
    for (span, new) in edits.into_iter().rev() {
        out.replace_range(usize::from(span.start())..usize::from(span.end()), &new);
    }
    Some(out)
}
