//! The family `unicode-escape`: a `\uXXXX` escape in a string or character literal (a token
//! tagged `#string`) that writes a visible character, where the character itself could stand.
//! An escape for a control, space, format or invisible character is fine, and so is a literal on
//! a line that ends with a comment (the comment tells what the escape is).
//!
//! concept: unicode-escape
//! what: A unicode escape of a visible character in a string literal, unless a comment explains it.
//! tools: Checkstyle AvoidEscapedUnicodeCharacters
//! rules: avoid-unicode-escape

use crate::{Family, FileContext, Params, Rule, RuleContext, RuleMeta};
use core_syntax::TextRange;

pub const FAMILY: Family = Family {
    name: "unicode-escape",
    build,
};

struct UnicodeEscape {
    meta: &'static RuleMeta,
    message: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let message = p.text("message")?;
    p.done()?;
    Ok(Box::new(UnicodeEscape { meta, message }))
}

/// Does the code point write nothing a reader could see (control, space, format, line breaks)?
fn invisible(c: u32) -> bool {
    matches!(c,
        0..=0x1f | 0x7f..=0xa0 | 0xad | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f | 0x1680 | 0x180e
        | 0x2000..=0x200f | 0x2028..=0x202f | 0x205f..=0x2064 | 0x2066..=0x206f | 0x3000
        | 0xfeff | 0xfff9..=0xfffb)
}

/// The offset of the first escape of the literal that writes a visible character.
fn visible_escape(text: &str) -> Option<usize> {
    let b = text.as_bytes();
    let mut i = 0;
    while i + 6 <= b.len() {
        if b[i] == b'\\' && b[i + 1] == b'u' {
            if text.is_char_boundary(i + 6)
                && let Ok(c) = u32::from_str_radix(&text[i + 2..i + 6], 16)
                && !invisible(c)
            {
                return Some(i);
            }
            i += 6;
        } else {
            i += 1;
        }
    }
    None
}

impl Rule for UnicodeEscape {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if !file.text.contains("\\u") {
            return;
        }
        let g = file.grammar;
        for t in file
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| g.has_tag(t.kind(), "string") && t.text().contains("\\u"))
        {
            let Some(at) = visible_escape(t.text()) else {
                continue;
            };
            let end = usize::from(t.text_range().end());
            let rest = file.text[end..].split('\n').next().unwrap_or("");
            if rest.contains("//") || rest.contains("/*") {
                continue;
            }
            let start = usize::from(t.text_range().start()) + at;
            ctx.report(
                TextRange::new((start as u32).into(), ((start + 6) as u32).into()),
                &self.message,
            );
        }
    }
}
