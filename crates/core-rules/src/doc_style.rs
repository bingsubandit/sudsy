//! The family `doc-comment`: style checks on the lines of a `/** */` comment
//! ([`crate::doc_lines`]). The param `check` picks one:
//! - `paragraph`: `<p>` stands right before the first word of a paragraph and after an empty
//!   line, an empty line between paragraphs is followed by `<p>`, and `<p>` is not the first
//!   thing in the comment;
//! - `summary`: the comment starts with a summary sentence that ends with a period and starts
//!   with none of the option `forbidden_fragments` (a regular expression);
//! - `asterisk`: every line after the first starts with an asterisk;
//! - `tag-indent`: the lines that continue a block tag (`@param x`) have the option `indent`
//!   blanks after the asterisk or more.
//!
//! concept: doc-comment-style
//! what: Style checks on the lines of a doc comment: paragraphs, summary, asterisks, tag indent.
//! tools: Checkstyle JavadocParagraph, SummaryJavadoc, JavadocMissingLeadingAsterisk, JavadocTagContinuationIndentation
//! rules: doc-paragraph, doc-summary, doc-asterisk, doc-tag-indent

use crate::doc_lines::{DocLine, lines};
use crate::{Family, FileContext, Params, RegexCache, Rule, RuleContext, RuleMeta};
use core_syntax::TextRange;

pub const FAMILY: Family = Family {
    name: "doc-comment",
    build,
};

#[derive(Clone, Copy)]
enum Check {
    Paragraph,
    Summary,
    Asterisk,
    TagIndent,
}

struct DocStyle {
    meta: &'static RuleMeta,
    check: Check,
    regex: RegexCache,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let check = p.choice(
        "check",
        true,
        &[
            ("paragraph", Check::Paragraph),
            ("summary", Check::Summary),
            ("asterisk", Check::Asterisk),
            ("tag-indent", Check::TagIndent),
        ],
    )?;
    p.done()?;
    Ok(Box::new(DocStyle {
        meta,
        check,
        regex: RegexCache::default(),
    }))
}

impl Rule for DocStyle {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if !file.text.contains("/**") {
            return;
        }
        for t in file
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.text().starts_with("/**") && t.text().len() > 4)
            .filter(|t| file.grammar.has_tag(t.kind(), "comment"))
        {
            let at = usize::from(t.text_range().start());
            let lines = lines(t.text(), at);
            match self.check {
                Check::Paragraph => paragraph(&lines, ctx),
                Check::Summary => summary(&lines, ctx, &self.regex, at),
                Check::Asterisk => asterisk(&lines, ctx),
                Check::TagIndent => tag_indent(&lines, ctx),
            }
        }
    }
}

fn range(start: usize, len: usize) -> TextRange {
    TextRange::new((start as u32).into(), ((start + len.max(1)) as u32).into())
}

/// The HTML elements that open a block: text inside them is not a paragraph of the comment.
const BLOCKS: [&str; 8] = [
    "pre",
    "blockquote",
    "ul",
    "ol",
    "table",
    "dl",
    "div",
    "code",
];

fn block_depth(text: &str, depth: &mut [i32; 8]) {
    let lower = text.to_ascii_lowercase();
    for (d, name) in depth.iter_mut().zip(BLOCKS) {
        *d += lower.matches(&format!("<{name}>")).count() as i32
            + lower.matches(&format!("<{name} ")).count() as i32
            - lower.matches(&format!("</{name}>")).count() as i32;
        *d = (*d).max(0);
    }
}

/// Does the text start an HTML block (so an empty line before it needs no `<p>`)?
fn starts_block(text: &str) -> bool {
    if text.starts_with("{@snippet") {
        return true;
    }
    let Some(rest) = text.strip_prefix('<') else {
        return false;
    };
    let rest = rest.strip_prefix('/').unwrap_or(rest).to_ascii_lowercase();
    let name: String = rest
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    matches!(
        name.as_str(),
        "p" | "pre"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "tr"
            | "td"
            | "th"
            | "blockquote"
            | "div"
            | "dl"
            | "dt"
            | "dd"
            | "hr"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}

fn paragraph(lines: &[DocLine<'_>], ctx: &mut RuleContext<'_>) {
    let mut depth = [0i32; 8];
    let mut tags = false;
    let mut seen = false;
    for (i, l) in lines.iter().enumerate() {
        tags |= l.block_tag();
        let inside = depth.iter().any(|d| *d > 0);
        if l.blank() {
            let next = lines.get(i + 1).filter(|n| !n.blank());
            if let Some(n) = next
                && seen
                && !tags
                && !inside
                && !l.inline
                && !n.text.starts_with("<p>")
                && !n.block_tag()
                && !starts_block(n.text)
            {
                ctx.report(
                    range(l.text_start, 1),
                    "an empty line between paragraphs is followed by a `<p>` tag",
                );
            }
            continue;
        }
        for (k, _) in l.text.match_indices("<p>") {
            let first = !seen && l.text[..k].trim().is_empty();
            let at = range(l.text_start + k, 3);
            if first {
                ctx.report(at, "redundant `<p>` tag");
                continue;
            }
            let after_blank = k == 0 && i > 0 && lines[i - 1].blank();
            if !after_blank {
                ctx.report(at, "a `<p>` tag is preceded with an empty line");
            }
            let after = &l.text[k + 3..];
            if after.is_empty() || after.starts_with(char::is_whitespace) {
                ctx.report(
                    at,
                    "put the `<p>` tag right before the first word, with no space",
                );
            }
        }
        seen = true;
        block_depth(l.text, &mut depth);
    }
}

/// The summary sentence, reported at the opening `/**`.
fn summary(lines: &[DocLine<'_>], ctx: &mut RuleContext<'_>, regex: &RegexCache, open: usize) {
    let at = range(open, 3);
    if lines.iter().all(DocLine::blank) {
        ctx.report(at, "the doc comment has no summary");
        return;
    }
    let mut description = String::new();
    for l in lines
        .iter()
        .take_while(|l| !l.block_tag())
        .filter(|l| !l.blank())
    {
        if !description.is_empty() {
            description.push(' ');
        }
        description.push_str(l.text);
    }
    if description.is_empty() {
        ctx.report(at, "the doc comment has no summary");
        return;
    }
    if description.starts_with("{@inheritDoc}") || description.starts_with("{@return") {
        return;
    }
    let end = description.match_indices('.').find(|(i, _)| {
        description[i + 1..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
    });
    let Some((end, _)) = end else {
        ctx.report(
            at,
            "the first sentence of the doc comment ends with no period",
        );
        return;
    };
    let pattern = ctx.options.string("forbidden_fragments");
    if !pattern.is_empty() && regex.is_match(pattern, &description[..=end]) {
        ctx.report(
            at,
            "the summary of the doc comment starts with a forbidden fragment",
        );
    }
}

fn asterisk(lines: &[DocLine<'_>], ctx: &mut RuleContext<'_>) {
    for l in lines.iter().skip(1).filter(|l| !l.star && !l.blank()) {
        ctx.report(
            range(l.text_start, 1),
            "the line of the doc comment starts with no asterisk",
        );
    }
}

fn tag_indent(lines: &[DocLine<'_>], ctx: &mut RuleContext<'_>) {
    let need = usize::try_from(ctx.options.int("indent")).unwrap_or(0);
    let mut depth = [0i32; 8];
    let mut tag = false;
    for l in lines.iter().skip(1) {
        let inside = depth.iter().any(|d| *d > 0);
        if l.block_tag() {
            tag = true;
        } else if tag
            && l.star
            && !l.blank()
            && !l.inline
            && !inside
            && !l.text.starts_with('<')
            && l.indent < need
        {
            ctx.report(
                range(l.text_start, 1),
                format!("a line that continues a block tag is indented by {need} blanks or more"),
            );
        }
        block_depth(l.text, &mut depth);
    }
}
