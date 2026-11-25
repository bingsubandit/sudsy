//! The `detached-token` family: a token of the named kind that must follow the code before it
//! directly (the `;` that ends a statement), not after spaces, a line break, a comment or a token
//! of its own kind. Params: `kind` (the token kind name).
//!
//! The report starts where the code before the token ends (the start of the file when a comment
//! is all there is before it), so it is on the line of that code, and runs to the token or to the
//! end of that line.
//!
//! concept: detached-token-family
//! what: Reports a terminator token written after white space or another terminator instead of right after the code.
//! tools: sqlfluff CV06

use crate::{Family, FileContext, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{TextRange, TextSize};

pub const FAMILY: Family = Family {
    name: "detached-token",
    build,
};

struct DetachedToken {
    meta: &'static RuleMeta,
    message: String,
    kind: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = DetachedToken {
        meta,
        message: p.text("message")?,
        kind: p.text("kind")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for DetachedToken {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let g = file.grammar;
        let mut previous_kind = false;
        // The end of the last token that is not trivia, and whether a comment follows it.
        let mut code_end = TextSize::from(0);
        let mut comment = false;
        for t in file
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if t.text_range().is_empty() {
                continue; // layout markers (indent, dedent) are not code
            }
            if g.name(t.kind()) != self.kind {
                previous_kind = false;
                if g.has_tag(t.kind(), "comment") {
                    comment = true;
                } else if !g.is_trivia(t.kind()) {
                    code_end = t.text_range().end();
                    comment = false;
                }
                continue;
            }
            let range = t.text_range();
            let before = file.text[..usize::from(range.start())].trim_end();
            let detached = previous_kind || comment || before.len() < usize::from(range.start());
            previous_kind = true;
            let from = if comment {
                code_end
            } else {
                TextSize::of(before)
            };
            let reported = detached && (comment || !before.is_empty());
            code_end = range.end();
            comment = false;
            if reported {
                let line_end = file.text[usize::from(from)..]
                    .find('\n')
                    .map_or(range.end(), |n| from + TextSize::from(n as u32));
                ctx.report(
                    TextRange::new(from, range.end().min(line_end)),
                    self.message.clone(),
                );
            }
        }
    }
}
