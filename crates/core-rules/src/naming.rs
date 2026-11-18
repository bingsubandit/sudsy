//! The `naming-convention` family: the name of a kind of declaration follows the convention an
//! option sets. Each param but `message` is `<option>: <rows>`: the rows are `|`-separated and a
//! row is a grammar tag, optionally `<parent-tag` (the parent has that tag); the name is the
//! node's `name` slot, or its `key` slot. The option's value is a convention (`PascalCase`,
//! `camelCase`, `snake_case`, `UPPER_CASE`) or `off`; leading `$` and `_` are ignored. The message
//! has `{name}` and `{convention}`.

use crate::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxElement, SyntaxNode};

pub const FAMILY: Family = Family {
    name: "naming-convention",
    build,
};

/// Whether `name` follows `convention`; an unknown convention (`off`) accepts every name.
pub fn follows(convention: &str, name: &str) -> bool {
    let name = name.trim_start_matches(['$', '_']);
    let first = name.chars().next();
    let plain = name.chars().all(|c| c.is_ascii_alphanumeric());
    match convention {
        "PascalCase" => plain && first.is_some_and(|c| c.is_ascii_uppercase()),
        "camelCase" => plain && first.is_some_and(|c| c.is_ascii_lowercase()),
        "snake_case" => !name.chars().any(|c| c.is_ascii_uppercase()),
        "UPPER_CASE" => !name.chars().any(|c| c.is_ascii_lowercase()),
        _ => true,
    }
}

struct Row {
    option: String,
    tag: String,
    parent: Option<String>,
}

struct Naming {
    meta: &'static RuleMeta,
    message: String,
    /// Rows with a parent condition first.
    rows: Vec<Row>,
    tags: Vec<&'static str>,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let message = p.text("message")?;
    let mut rows = Vec::new();
    for (option, value) in p.rest() {
        for alt in value.split('|') {
            let (tag, parent) = match alt.split_once('<') {
                Some((t, par)) => (t, Some(par.to_string())),
                None => (alt, None),
            };
            rows.push(Row {
                option: option.clone(),
                tag: tag.to_string(),
                parent,
            });
        }
    }
    rows.sort_by_key(|r| r.parent.is_none());
    let mut tags: Vec<&'static str> = Vec::new();
    for r in &rows {
        if !tags.contains(&r.tag.as_str()) {
            tags.push(Box::leak(r.tag.clone().into_boxed_str()));
        }
    }
    Ok(Box::new(Naming {
        meta,
        message,
        rows,
        tags,
    }))
}

impl Rule for Naming {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn tags(&self) -> &[&'static str] {
        &self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let row = self.rows.iter().find(|r| {
            g.has_tag(node.kind(), &r.tag)
                && r.parent
                    .as_ref()
                    .is_none_or(|t| node.parent().is_some_and(|p| g.has_tag(p.kind(), t)))
        });
        let Some(row) = row else { return };
        let Some(name) = g.slot(node, "name").or_else(|| g.slot(node, "key")) else {
            return;
        };
        let Some(token) = (match name {
            SyntaxElement::Token(t) => Some(t),
            SyntaxElement::Node(n) => n.first_token(),
        }) else {
            return;
        };
        let convention = ctx.options.string(&row.option);
        let text = token.text().trim_start_matches('$');
        if !follows(convention, text) {
            let message = self.message.replace("{name}", text);
            ctx.report(
                token.text_range(),
                message.replace("{convention}", convention),
            );
        }
    }
}
