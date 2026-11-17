//! Checks on comments.

use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::checks::NodeCheck;
use core_syntax::{SyntaxNode, TextSize};

/// The departments of RuboCop's own cops; a directive for any other (an extension that is not
/// loaded) names a cop RuboCop does not know.
const DEPARTMENTS: &[&str] = &[
    "Lint",
    "Style",
    "Layout",
    "Metrics",
    "Naming",
    "Security",
    "Gemspec",
    "Bundler",
    "Migration",
];

/// `# rubocop:disable Department/Cop` for a department that does not exist.
pub fn unknown_cop_name(check: &NodeCheck, program: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let text = ctx.file.text;
    if program.parent().is_none() {
        return;
    }
    let mut last = None;
    for (at, _) in text.match_indices("rubocop:") {
        let comment = ctx
            .file
            .root
            .token_at_offset(TextSize::new(at as u32))
            .right_biased()
            .filter(|t| t.kind() == k::COMMENT);
        let Some(comment) = comment.filter(|c| last != Some(c.text_range().start())) else {
            continue;
        };
        let rest = &text[at + "rubocop:".len()..];
        let line = rest.split('\n').next().unwrap_or("");
        let names = line
            .split_once(|c: char| c.is_whitespace())
            .map_or("", |(_, r)| r);
        let names = names.split_once("--").map_or(names, |(n, _)| n);
        let unknown = names
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|n| !n.is_empty())
            .filter_map(|n| n.split_once('/'))
            .find(|(department, _)| !DEPARTMENTS.contains(department));
        if let Some((department, name)) = unknown {
            last = Some(comment.text_range().start());
            let range = comment.text_range();
            check.hit(ctx, range, &format!("{department}/{name}"));
        }
    }
}
