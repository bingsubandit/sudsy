//! Dockerfile formatting: one instruction per line group, the keyword upper case, one space between
//! arguments where the source has blanks, line continuations ending the line and indenting what
//! follows. Shell text, options, paths and pairs are kept as written; only the line breaks of
//! multi-line text are re-indented (never inside a quote, where the blanks are content).
//!
//! concept: token-case, line-continuation-layout

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
                // The shell text is compared as the shell reads it: continuations joined, the
                // `;` that ends the last statement and the one before a `;;` or `)` left out, comment lines dropped.
                k::SHELL_COMMAND => t
                    .text()
                    .split('\n')
                    .enumerate()
                    .filter(|(i, l)| *i == 0 || !l.trim_start().starts_with('#'))
                    .map(|(_, l)| l)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .replace("\\\r\n", "")
                    .replace("\\\n", "")
                    .split_whitespace()
                    .collect::<String>()
                    .replace(";;;", ";;")
                    .replace(";)", ")")
                    .trim_end_matches(';')
                    .to_string()
                    .into(),
                _ => t.text().split_whitespace().collect::<String>().into(),
            },
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &core_syntax::Grammar,
        options: &core_config::ResolvedOptions,
    ) -> Option<String> {
        crate::rewrite::rewrite(
            tree,
            text,
            grammar,
            (
                options.bool(core_config::opt::KEY_VALUE_PAIRS),
                options.bool(core_config::opt::JSON_COPY_AS_WORDS),
                options.bool(core_config::opt::COMMENT_SPACE),
            ),
        )
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

/// Operators that start a line of shell text laid out with `binary_operator_line: start`, and the
/// words that close a block at the column of its head.
const SHELL_OPERATORS: &[&str] = &["&&", "||", "|&", "|"];
const SHELL_CLOSERS: &[&str] = &["fi", "done", "esac", "}", ")", "elif", "else", ";;"];
/// The flag after which the flags and the command of a `RUN` stand on lines of their own.
const OWN_LINE_FLAG: &str = "--mount";

/// The escape character of the instruction: the one its continuations use, else the one the
/// file's `# escape=` directive sets.
fn escape_char(cx: &FormatCtx<'_>, node: &SyntaxNode) -> char {
    let used = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::LINE_CONTINUATION);
    if let Some(t) = used {
        return t.text().chars().next().unwrap_or('\\');
    }
    let directive = node
        .ancestors()
        .last()
        .and_then(|root| cx.first_token(&root))
        .is_some_and(|first| {
            comments::leading_comments(cx.grammar, &first)
                .iter()
                .any(|c| c.text().replace(' ', "").starts_with("#escape=`"))
        });
    if directive { '`' } else { '\\' }
}

/// The lines of the shell text `text` laid out by the Shell formatter (statement separators
/// stay, line breaks are continuations) and its comment lines (the number of code lines before
/// each, the line), or `None` when the text stays as written: another escape character, a
/// heredoc, a quote that goes over lines, comment lines that do not keep their place, or code that
/// does not parse.
fn shell_lines(
    cx: &FormatCtx<'_>,
    text: &str,
    escape: char,
) -> Option<(String, Vec<(usize, String)>)> {
    if escape != '\\' || text.contains("<<") {
        return None;
    }
    let overrides = [
        core_config::opt::INDENT_STYLE,
        core_config::opt::INDENT_SIZE,
        core_config::opt::BINARY_OPERATOR_LINE,
        core_config::opt::SPACE_REDIRECTS,
        core_config::opt::INDENT_CASE_ARMS,
    ]
    .into_iter()
    .filter_map(|id| Some((id, cx.options.get(id).cloned()?)))
    // (a blank continuation line stays, however many follow each other)
    .chain([(
        core_config::opt::MAX_BLANK_LINES,
        core_config::OptionValue::Int(10),
    )])
    .collect::<Vec<_>>();
    // (`RUN \` + line break + command: the command goes on the first line)
    let mut source = text.replace('\r', "");
    while let Some(rest) = source.strip_prefix("\\\n") {
        source = rest.trim_start().to_string();
    }
    let code = source
        .split('\n')
        .enumerate()
        .filter(|(i, l)| *i == 0 || !l.trim_start().starts_with('#'))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");
    // (a line that starts inside a quote is text, not a comment)
    let inside = continuation::quote_starts(&source);
    if source
        .split('\n')
        .zip(inside)
        .any(|(l, q)| q && l.trim_start().starts_with('#'))
    {
        return None;
    }
    let laid = cx.embed_with(
        "shell",
        core_format::EmbedMode::Continued,
        &code,
        (80, false),
        &overrides,
    )?;
    let comments = Continued::place_comments(&source, &laid)?;
    Some((laid, comments))
}

/// All tokens of the instruction, nested instructions (`ONBUILD`, `HEALTHCHECK ... CMD`) and
/// JSON arrays included.
fn instruction(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let mut out = Continued::default();
    let mut heredocs = Vec::new();
    let mut prev = None::<core_syntax::SyntaxToken>;
    let mut blank = false;
    let tokens = || {
        node.descendants_with_tokens()
            .filter_map(|e| e.into_token())
    };
    let flags = tokens()
        .filter(|t| t.kind() == k::WORD && t.text().starts_with("--"))
        .count();
    // (a lone flag before a heredoc stays on the line)
    let own_lines = tokens().any(|t| t.kind() == k::WORD && t.text().starts_with(OWN_LINE_FLAG))
        && !(flags == 1 && tokens().any(|t| t.kind() == k::HEREDOC));
    let escape = escape_char(cx, node);
    let unit = if cx.options.is(core_config::opt::INDENT_STYLE, "tab") {
        "\t".to_string()
    } else {
        " ".repeat(cx.options.int(core_config::opt::INDENT_SIZE).max(1) as usize)
    };
    let mut flag_break = false;
    let shell_form = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| t.kind() == k::SHELL_COMMAND);
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
        if flag_break && kind != k::LINE_CONTINUATION {
            out.end_line(&escape.to_string());
            prev = None;
            blank = false;
        }
        flag_break = false;
        let leading = cx.leading_comments_doc(&t);
        let text = t.text();
        if kind == k::LINE_CONTINUATION
            && shell_form
            && prev.as_ref().is_some_and(|p| p.kind() == k::KEYWORD)
            && comments::trailing_comments(cx.grammar, &t).is_empty()
        {
            // `RUN \` + line break + command: the command goes on the first line.
            blank = true;
            continue;
        }
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
            k::SHELL_COMMAND => match shell_lines(cx, text, escape) {
                Some((code, comments)) => {
                    out.code(&code, &unit, SHELL_OPERATORS, SHELL_CLOSERS, &comments)
                }
                // (shell text that is not laid out stays as written)
                None if text.contains('\n') => out.raw(text),
                None => out.push(cx.token_text(&t)),
            },
            _ if text.contains('\n') => out.text(text),
            _ => out.push(cx.token_text(&t)),
        }
        flag_break = own_lines && kind == k::WORD && text.starts_with("--");
        out.push(cx.trailing_comments_doc(&t));
        prev = Some(t.clone());
        blank = false;
    }
    doc::concat(vec![out.finish(), doc::concat(heredocs)])
}
