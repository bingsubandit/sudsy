//! Dockerfile formatting: one instruction per line group, the keyword upper case, one space between
//! arguments where the source has blanks, line continuations ending the line and indenting what
//! follows. Shell text, options, paths and pairs are kept as written; only the line breaks of
//! multi-line text are re-indented (never inside a quote, where the blanks are content).

use crate::generated::kind as k;
use core_config::shared;
use core_format::rules::LazyRules;
use core_format::shapes::continuation::{self, Continued};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode, grammars};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct DockerfileFormatter;

/// A keyword in the case the `[case]` row names.
fn keyword(text: &str) -> String {
    let cases = ROWS.get().cases();
    cases
        .fixed(k::KEYWORD, text)
        .unwrap_or_else(|| text.to_string())
}

static ROWS: LazyRules = LazyRules::new(
    || grammars::DOCKERFILE.get(),
    include_str!("../format.rules"),
);

impl NodeFormatter for DockerfileFormatter {
    /// Only case and blanks may change; heredocs without CR.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => match t.kind() {
                k::HEREDOC => t.text().replace('\r', "").into(),
                k::KEYWORD => keyword(t.text()).into(),
                // The instruction name of a raw instruction is upper-cased as well.
                k::RAW => t
                    .text()
                    .split_whitespace()
                    .collect::<String>()
                    .to_ascii_uppercase()
                    .into(),
                _ => t.text().split_whitespace().collect::<String>().into(),
            },
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::INSTRUCTION => instruction(cx, node),
            _ => return Ok(None),
        }))
    }
}

/// The instructions, then the comments after them (owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    core_format::shapes::document(cx, node, cx.options.max_blank_lines(), None)
}

/// All tokens of the instruction, nested instructions (`ONBUILD`, `HEALTHCHECK ... CMD`) and
/// JSON arrays included.
fn instruction(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let mut out = Continued::default();
    let mut heredocs = Vec::new();
    let mut prev = None::<core_syntax::SyntaxToken>;
    let mut blank = false;
    for t in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let kind = t.kind();
        if matches!(kind, k::WHITESPACE | k::NEWLINE) {
            blank = true;
            continue;
        }
        if cx.grammar.is_trivia(kind) {
            continue;
        }
        let leading = cx.leading_comments_doc(&t);
        let text = t.text();
        if kind == k::LINE_CONTINUATION {
            out.end_line(&text[..1]);
            // Comment lines after the continuation are trailing trivia of it.
            for c in comments::trailing_comments(cx.grammar, &t) {
                out.own_line(core_format::context::comment_doc(&c));
            }
            prev = None;
            blank = false;
            continue;
        }
        if prev
            .as_ref()
            .is_some_and(|p| ROWS.get().space().want(p, &t).unwrap_or(blank))
        {
            out.space();
        }
        out.push(leading);
        match kind {
            k::KEYWORD => out.push(doc::text(keyword(text))),
            k::RAW => {
                let (word, rest) = text.split_once([' ', '\t']).unwrap_or((text, ""));
                out.text(&format!(
                    "{} {}",
                    word.to_ascii_uppercase(),
                    rest.trim_start()
                ));
            }
            k::HEREDOC => heredocs.push({
                let body = text.replace('\r', "");
                continuation::heredoc(body.strip_prefix('\n').unwrap_or(&body))
            }),
            _ if text.contains('\n') => out.text(text),
            _ => out.push(cx.token_text(&t)),
        }
        out.push(cx.trailing_comments_doc(&t));
        prev = Some(t.clone());
        blank = false;
    }
    doc::concat(vec![out.finish(), doc::concat(heredocs)])
}
