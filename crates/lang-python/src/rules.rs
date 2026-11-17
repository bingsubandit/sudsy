//! The families of Python's rules; rules that fit a pattern or a check are files in `rules/`.

use crate::analysis::hits;
use crate::generated::kind as k;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

pub static RULES: &[&dyn Rule] = &[];

pub static FAMILIES: &[Family] = &[
    Family {
        name: "check",
        build,
    },
    core_rules::bug::FAMILY,
];

/// A rule that reports the findings of one check of `analysis` (`params: { check: <name> }`);
/// `{name}` in the message is the name the finding carries.
struct Check {
    meta: &'static RuleMeta,
    check: String,
    message: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = Check {
        meta,
        check: p.text("check")?,
        message: p.text("message")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Check {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[k::MOD_MODULE]
    }

    fn check(&self, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        for h in hits(ctx.file).iter().filter(|h| h.check == self.check) {
            let range = TextRange::new(h.start.into(), h.end.into());
            ctx.report(range, self.message.replace("{name}", &h.name));
        }
    }
}
