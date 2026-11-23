//! `no-hard-tabs`: a tab character anywhere in a line, one report per run of tabs. Tokens of the
//! kinds the option `ignore_kinds` names (front matter, code) are not looked at.
//!
//! concept: hard-tabs
//! what: Tab characters in the text of a file, except inside tokens of named kinds.
//! tools: markdownlint MD010

use crate::options::{Check, Opt, OptionDecl};
use crate::rule;
use crate::rule::{FileContext, RuleContext};
use core_syntax::{TextRange, TextSize};

rule! {
    pub NoHardTabs {
        id: "no-hard-tabs",
        category: Style,
        default: Off,
        description: "A tab character in a line. Each run of tabs is one report.",
        example: "\tindented with a tab   // reported at the tab",
        options: [OptionDecl {
            name: "ignore_kinds",
            default: Opt::List(&[]),
            check: Check::Any,
            description: "Kinds of tokens whose tabs are not reported (for example the code of a fence).",
        }],
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if !file.text.contains('\t') {
            return;
        }
        let ignore = ctx.options.list("ignore_kinds");
        let skipped: Vec<TextRange> = if ignore.is_empty() {
            Vec::new()
        } else {
            file.root
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| ignore.contains(&file.grammar.name(t.kind())))
                .map(|t| t.text_range())
                .collect()
        };
        let bytes = file.text.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] != b'\t' {
                at += 1;
                continue;
            }
            let start = at;
            while bytes.get(at) == Some(&b'\t') {
                at += 1;
            }
            let range = TextRange::new(TextSize::from(start as u32), TextSize::from(at as u32));
            if !skipped.iter().any(|s| s.contains_range(range)) {
                ctx.report(range, "hard tab");
            }
        }
    }
}
