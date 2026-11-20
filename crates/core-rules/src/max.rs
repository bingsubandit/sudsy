//! The `max` family: a count over grammar tags that must not pass option `max`. Params:
//! - `tag`: the nodes the rule looks at;
//! - `count`: `ancestors` (the ancestors with the same tag: nesting depth), `depth` (how deeply
//!   the tag nests below an outermost node: the node itself is level 1), `lines` (the lines of
//!   the descendants tagged `of`), `children` (the children tagged `of`; with `single-line:
//!   true` only of a node written on one line) or `decimals` (the digits after the point of each
//!   token tagged `of`, which must be `#number`; reported per token);
//! - `report`: the tag of a descendant whose first token gets the report (default: the node); the first token that is not trivia, so blank lines before it are not reported.
//!
//! The message has `{count}` and `{max}`.
//!
//! concept: max-family
//! what: A count over grammar tags that must not pass an option max.
//! tools: stylelint max-nesting-depth, graphql-eslint selection-set-depth, checkmake maxbodylength
//! reads: family:max

use crate::tree::significant_range;
use crate::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{Grammar, SyntaxNode, TextRange};

pub const FAMILY: Family = Family { name: "max", build };

#[derive(Clone, Copy)]
enum Count {
    Ancestors,
    Depth,
    Lines,
    Children,
    Decimals,
}

struct Max {
    meta: &'static RuleMeta,
    count: Count,
    message: String,
    tags: [&'static str; 1],
    of: String,
    report: Option<String>,
    single_line: bool,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let count = p.choice(
        "count",
        true,
        &[
            ("ancestors", Count::Ancestors),
            ("depth", Count::Depth),
            ("lines", Count::Lines),
            ("children", Count::Children),
            ("decimals", Count::Decimals),
        ],
    )?;
    let rule = Max {
        meta,
        count,
        message: p.text("message")?,
        tags: [Box::leak(p.text("tag")?.into_boxed_str())],
        of: p.optional("of").unwrap_or_default(),
        report: p.optional("report"),
        single_line: p.flag("single-line")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

/// Levels of `tag` from `node` down.
fn depth(node: &SyntaxNode, tag: &str, g: &Grammar) -> usize {
    let inner = node
        .children()
        .map(|c| depth(&c, tag, g))
        .max()
        .unwrap_or(0);
    inner + usize::from(g.has_tag(node.kind(), tag))
}

impl Rule for Max {
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
        let g = ctx.file.grammar;
        let tag = self.tags[0];
        let max = usize::try_from(ctx.options.int("max")).unwrap_or(0);
        let node_range = significant_range(node, g);
        if self.single_line && ctx.file.text[node_range].contains(['\n', '\r']) {
            return;
        }
        let found = match self.count {
            Count::Ancestors => node
                .ancestors()
                .skip(1)
                .filter(|a| g.has_tag(a.kind(), tag))
                .count(),
            Count::Depth => {
                if node.ancestors().skip(1).any(|a| g.has_tag(a.kind(), tag)) {
                    return;
                }
                depth(node, tag, g)
            }
            Count::Lines => node
                .descendants()
                .filter(|d| g.has_tag(d.kind(), &self.of))
                .map(|d| d.text().to_string().trim().matches('\n').count() + 1)
                .sum(),
            Count::Children => node
                .children()
                .filter(|c| g.has_tag(c.kind(), &self.of))
                .count(),
            Count::Decimals => {
                for t in node
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| g.has_tag(t.kind(), &self.of))
                {
                    self.over(ctx, fraction_digits(t.text()), max, t.text_range());
                }
                return;
            }
        };
        let range: Option<TextRange> = match &self.report {
            None if matches!(self.count, Count::Depth) => Some(node.text_range()),
            None => Some(node_range),
            Some(t) => node
                .descendants()
                .find(|d| g.has_tag(d.kind(), t))
                .and_then(|d| {
                    d.descendants_with_tokens()
                        .filter_map(|e| e.into_token())
                        .find(|t| crate::tree::is_code(t, g))
                })
                .map(|t| t.text_range()),
        };
        if let Some(range) = range {
            self.over(ctx, found, max, range);
        }
    }
}

impl Max {
    fn over(&self, ctx: &mut RuleContext<'_>, found: usize, max: usize, range: TextRange) {
        if found > max {
            let message = self.message.replace("{count}", &found.to_string());
            ctx.report(range, message.replace("{max}", &max.to_string()));
        }
    }
}

/// The digits after the point of the number a token starts with (`1.50px`: 2).
fn fraction_digits(raw: &str) -> usize {
    let mut chars = raw.trim_start_matches(['+', '-']).chars().peekable();
    while chars.next_if(char::is_ascii_digit).is_some() {}
    if chars.next_if_eq(&'.').is_none() {
        return 0;
    }
    chars.take_while(char::is_ascii_digit).count()
}
