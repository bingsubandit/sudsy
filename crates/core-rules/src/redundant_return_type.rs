//! `redundant-return-type`: a function with a block body whose declared return type is one the
//! language gives every such function anyway (`: Unit` in Kotlin, the language's option
//! `redundant_return_types`) loses the colon and the type. The functions are the `#function`
//! nodes, their `parameters` and `body` slots; the type is the node after the colon that follows
//! the parameters, directly in front of the body. Only the formatter runs it (lint has the
//! language's own report rule); the rule is `off` unless the config turns it on.
//!
//! concept: redundant-return-type
//! what: The return type a block-body function has by default is left out.
//! tools: ktlint no-unit-return
//! rules: redundant-return-type
//! reads: tag:function, tag:block

use crate::diagnostic::Edit;
use crate::rule;
use crate::rule::{FileContext, RuleContext};
use core_syntax::{SyntaxElement, TextRange, TextSize};

rule! {
    pub RedundantReturnType {
        id: "redundant-return-type",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A function with a block body declares the return type the language gives \
                  it anyway (`: Unit`). The fix leaves out the colon and the type.",
        example: "fun f(): Unit { }",
        format: true,
    }

    fn format_kinds(&self, grammar: &core_syntax::Grammar) -> Vec<core_syntax::SyntaxKind> {
        grammar.nodes_with_tag("function").to_vec()
    }

    fn format_applies(
        &self,
        _grammar: &core_syntax::Grammar,
        text: &str,
        _own: &crate::options::RuleOptions,
        format: &core_config::ResolvedOptions,
    ) -> bool {
        has_typed(text, &format.strings("redundant_return_types"))
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let g = file.grammar;
        // (the formatter calls it only when `format_applies`)
        let Some(options) = ctx.format.as_ref().map(|f| f.options) else {
            return;
        };
        let kinds = self.format_kinds(g);
        for n in ctx.nodes(&kinds) {
            let (Some(SyntaxElement::Node(params)), Some(SyntaxElement::Node(body))) =
                (g.slot(&n, "parameters"), g.slot(&n, "body"))
            else {
                continue;
            };
            if !g.has_tag(body.kind(), "block") {
                continue;
            }
            // `)`, `:`, the type, then the body: nothing else in between
            let mut after = n
                .children_with_tokens()
                .skip_while(|e| e.as_node() != Some(&params))
                .skip(1)
                .filter(|e| !e.as_token().is_some_and(|t| g.is_trivia(t.kind())));
            let (Some(SyntaxElement::Token(colon)), Some(SyntaxElement::Node(ty)), Some(next)) =
                (after.next(), after.next(), after.next())
            else {
                continue;
            };
            if colon.text() != ":"
                || next.as_node() != Some(&body)
                || !options.list_has("redundant_return_types", ty.text().to_string().trim())
            {
                continue;
            }
            let Some(end) = params
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| !g.is_trivia(t.kind()))
                .last()
                .map(|t| t.text_range().end())
            else {
                continue;
            };
            let ty_end = ty
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| !g.is_trivia(t.kind()))
                .last()
                .map_or(ty.text_range().end(), |t| t.text_range().end());
            let range = TextRange::new(end, ty_end);
            // (a comment between the parts would go with it: leave such a function alone)
            if file.text[usize::from(range.start())..usize::from(range.end())].contains("/") {
                continue;
            }
            ctx.report_fix(
                TextRange::empty(TextSize::from(u32::from(end))),
                "`Unit` is the default return type: leave it out",
                ("Remove the return type", vec![Edit::replace(range, "")]),
            );
        }
    }
}

/// Does the text have one of the `words` between a colon and an opening brace (no such type: no
/// walk of the functions)?
fn has_typed(text: &str, words: &[String]) -> bool {
    words.iter().any(|w| {
        text.match_indices(w.as_str()).any(|(at, _)| {
            text[at + w.len()..].trim_start().starts_with('{')
                && text[..at].trim_end_matches([' ', '\t']).ends_with(':')
        })
    })
}
