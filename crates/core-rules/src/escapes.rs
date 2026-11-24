//! `invalid-escape-sequence`, on the nodes tagged `#string-escapes` (one string piece with one
//! prefix: a literal, an f-string): a backslash in a string that is not raw and starts no escape
//! its `string` row defines. The pieces of an interpolated string have no delimiters of their own;
//! a backslash that ends one escapes the brace that opens the next field.
//!
//! concept: invalid-escape-sequence
//! what: A backslash in a non-raw string that starts no escape its string row defines.
//! rules: invalid-escape-sequence
//! reads: rule:invalid-escape-sequence

use crate::rule;
use crate::rule::RuleContext;
use core_syntax::{SyntaxNode, TextRange, TextSize};

/// Does the grammar tag nodes for this rule?
pub fn applies(g: &core_syntax::Grammar) -> bool {
    !g.nodes_with_tag("string-escapes").is_empty()
}

rule! {
    pub InvalidEscapeSequence {
        id: "invalid-escape-sequence",
        category: Suspicious,
        default: Warn,
        description: "A backslash in a string that is not raw and does not start an escape sequence (`\\d`, `\\_`), also in the pieces of an f-string.",
        example: "x = \"\\d\"",
        tags: &["string-escapes"],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let own = |t: &core_syntax::SyntaxToken| {
            t.parent_ancestors()
                .find(|a| g.has_tag(a.kind(), "string-escapes"))
                .is_none_or(|a| a == *node)
        };
        let prefix: String = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| g.has_tag(t.kind(), "string"))
            .map(|t| {
                (t.text().chars())
                    .take_while(char::is_ascii_alphabetic)
                    .collect()
            })
            .unwrap_or_default();
        let tokens = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| g.has_tag(t.kind(), "string") && own(t));
        for t in tokens {
            let Some(row) = g.string_rows(t.kind()).next() else {
                continue;
            };
            if prefix.chars().any(|c| row.raw.contains(c)) && !row.raw.is_empty() {
                continue;
            }
            let text = if row.split(t.text()).is_some() {
                t.text().to_owned()
            } else {
                format!("{}{{", t.text())
            };
            let start = t.text_range().start();
            row.unknown_escapes(&text, '"', "", |i, next| {
                let at = start + TextSize::try_from(i).unwrap_or_default();
                let len = TextSize::try_from(1 + next.len_utf8()).unwrap_or_default();
                ctx.report(
                    TextRange::at(at, len),
                    format!("invalid escape sequence `\\{next}`: use a raw string or double the backslash"),
                );
            });
        }
    }
}
