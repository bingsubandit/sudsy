//! The `selector-max` family: a count per complex selector that must not pass option `max`. Param
//! `count`: `class`, `id`, `attribute`, `pseudo-class`, `type`, `universal`, `compounds` or
//! `combinators`. The message has `{count}` and `{max}`.

use super::model::{self, Part};
use crate::generated::kind as k;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt};

pub const FAMILY: Family = Family {
    name: "selector-max",
    build,
};

#[derive(Clone, Copy)]
enum Count {
    Part(Part),
    Compounds,
    Combinators,
}

struct Max {
    meta: &'static RuleMeta,
    count: Count,
    message: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    use Count::*;
    let count = p.choice(
        "count",
        true,
        &[
            ("class", Part(model::Part::Class)),
            ("id", Part(model::Part::Id)),
            ("attribute", Part(model::Part::Attribute)),
            ("pseudo-class", Part(model::Part::PseudoClass)),
            ("type", Part(model::Part::Type)),
            ("universal", Part(model::Part::Universal)),
            ("compounds", Compounds),
            ("combinators", Combinators),
        ],
    )?;
    let rule = Max {
        meta,
        count,
        message: p.text("message")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Max {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[k::RULE]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let max = usize::try_from(ctx.options.int("max")).unwrap_or(0);
        let Some(prelude) = node.child(k::PRELUDE) else {
            return;
        };
        if model::in_keyframes(node) {
            return;
        }
        for sel in model::selectors(&prelude) {
            let Some(range) = sel.range else { continue };
            let count = match self.count {
                Count::Part(part) => sel.count(part),
                Count::Compounds => sel.compounds.len(),
                Count::Combinators => sel.combinators.len(),
            };
            if count > max {
                let message = self.message.replace("{count}", &count.to_string());
                ctx.report(range, message.replace("{max}", &max.to_string()));
            }
        }
    }
}
