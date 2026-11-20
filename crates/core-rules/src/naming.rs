//! The `naming-convention` family: the name of a kind of declaration follows the convention an
//! option sets. Each param but `message` and `match` is `<option>: <rows>`: the rows are
//! `|`-separated declaration rows ([`crate::decl_rows`]: a grammar tag, `<parent-tag`, `&modifier`,
//! `&@annotation`, `=path`; the node's `name` slot, or its `key` slot, by default). The
//! option's value is a convention (`PascalCase`, `camelCase`, `snake_case`, `UPPER_CASE`) or
//! `off`; leading `$` and `_` are ignored. With `match: regex` the value is a regular expression
//! the name must match instead (empty: not checked). The message has `{name}` and
//! `{convention}` (the value); a param `message.<option>` is the message of that option.
//!
//! concept: naming-convention
//! what: The name of a kind of declaration follows the convention an option sets.
//! rules: naming-convention
//! reads: family:naming-convention

use crate::decl_rows::Row;
use crate::slot_path::SlotPath;
use crate::{Family, Params, RegexCache, Rule, RuleContext, RuleMeta};
use core_syntax::SyntaxNode;

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

struct Naming {
    meta: &'static RuleMeta,
    message: String,
    /// `message.<option>` params: the message of one option.
    messages: Vec<(String, String)>,
    regex: Option<RegexCache>,
    /// Rows with the most conditions first.
    rows: Vec<Row>,
    tags: Vec<&'static str>,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let message = p.text("message")?;
    let regex = p.choice("match", false, &[("convention", false), ("regex", true)])?;
    let mut rows = Vec::new();
    let mut messages = Vec::new();
    for (option, value) in p.rest() {
        if let Some(option) = option.strip_prefix("message.") {
            messages.push((option.to_string(), value));
            continue;
        }
        for alt in value.split('|') {
            rows.push(Row::parse(&option, alt)?);
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.conditions()));
    let mut tags: Vec<&'static str> = Vec::new();
    for r in &rows {
        if !tags.contains(&r.tag.as_str()) {
            tags.push(Box::leak(r.tag.clone().into_boxed_str()));
        }
    }
    Ok(Box::new(Naming {
        meta,
        message,
        messages,
        regex: regex.then(RegexCache::default),
        rows,
        tags,
    }))
}

impl Rule for Naming {
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
        // a more specific row whose option is empty (regex mode: not checked) leaves the node to the next row
        let regex = self.regex.is_some();
        let Some(row) = (self.rows.iter())
            .find(|r| r.matches(g, node) && !(regex && ctx.options.string(&r.option).is_empty()))
        else {
            return;
        };
        let default = SlotPath::parse("name|key").unwrap_or_else(|_| unreachable!());
        let path = row.path.as_ref().unwrap_or(&default);
        let Some(found) = path.find(g, node, ctx.file.text) else {
            return;
        };
        let convention = ctx.options.string(&row.option);
        let (text, follows) = match &self.regex {
            Some(cache) => (
                &*found.text,
                convention.is_empty() || cache.is_match(convention, &found.text),
            ),
            None => {
                let text = found.text.trim_start_matches('$');
                (text, follows(convention, text))
            }
        };
        if !follows {
            let template = (self.messages.iter())
                .find(|(o, _)| *o == row.option)
                .map_or(&self.message, |(_, m)| m);
            let message = template.replace("{name}", text);
            ctx.report(found.range, message.replace("{convention}", convention));
        }
    }
}
