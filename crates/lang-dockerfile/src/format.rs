//! Dockerfile formatting: one instruction per line group, the keyword upper case, one space between
//! arguments where the source has blanks, line continuations ending the line and indenting what
//! follows. Options, paths and pairs are kept as written, and so is shell text when no shell
//! formatter lays it out; only the line breaks of multi-line text are re-indented (never inside a
//! quote, where the blanks are content).
//!
//! concept: token-case, line-continuation-layout

use crate::generated::kind as k;
use core_config::shared;
use core_format::rules::LazyRules;
use core_format::shapes::continuation::{self, Continued};
use core_format::{Doc, EmbedMode, FormatCtx, FormatError, NodeFormatter};
use core_format::{comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, grammars};
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

/// The language of the shell text of `RUN`, `CMD`, `ENTRYPOINT` and `HEALTHCHECK`.
const SHELL: &str = "shell";

/// The options that the Shell formatter takes over from the Dockerfile's for the shell text:
/// the indentation and the layout choices that `lang.yaml` declares for it.
const SHELL_OPTIONS: [core_config::OptId; 5] = [
    core_config::opt::INDENT_STYLE,
    core_config::opt::INDENT_SIZE,
    core_config::opt::BINARY_OPERATOR_LINE,
    core_config::opt::SPACE_REDIRECTS,
    core_config::opt::INDENT_CASE_ARMS,
];

/// The flag after which the flags and the command of a `RUN` stand on lines of their own.
const OWN_LINE_FLAG: &str = "--mount";

impl NodeFormatter for DockerfileFormatter {
    /// Only case and blanks may change; heredocs without CR. The shell text is compared by
    /// [`NodeFormatter::embedded_text`].
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
                k::SHELL_COMMAND => "".into(),
                _ => t.text().split_whitespace().collect::<String>().into(),
            },
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    /// The shell text is read by the Shell provider (its tree, its comparison of values, its
    /// comments); a text the formatter left as written is the same text.
    fn embedded_text(&self) -> Option<core_format::context::EmbeddedText> {
        Some(core_format::context::EmbeddedText {
            kind: k::SHELL_COMMAND,
            language: SHELL,
            mode: EmbedMode::Continued,
            forward: &SHELL_OPTIONS,
            read: continuation::read_lines,
            size: (continuation::EMBED_WIDTH, false),
        })
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

/// What one pass over the tokens of an instruction finds.
#[derive(Default)]
struct Facts {
    /// The flags (`--mount=...`) of the instruction, and whether one of them is `--mount`.
    flags: usize,
    mount: bool,
    heredoc: bool,
    shell: bool,
    /// The character a line continuation token of the instruction starts with.
    continuation: Option<char>,
}

impl Facts {
    fn of(tokens: impl Iterator<Item = SyntaxToken>) -> Facts {
        let mut facts = Facts::default();
        for t in tokens {
            match t.kind() {
                k::WORD if t.text().starts_with("--") => {
                    facts.flags += 1;
                    facts.mount |= t.text().starts_with(OWN_LINE_FLAG);
                }
                k::HEREDOC => facts.heredoc = true,
                k::SHELL_COMMAND => facts.shell = true,
                k::LINE_CONTINUATION if facts.continuation.is_none() => {
                    facts.continuation = t.text().chars().next();
                }
                _ => {}
            }
        }
        facts
    }
}

/// The shell text laid out by the Shell formatter (see [`continuation::embed_continued`]), or
/// `None` when it stays as written: no shell formatter, another escape character than a
/// backslash, a heredoc, or code that does not parse.
fn shell_lines(cx: &FormatCtx<'_>, text: &str, escape: impl Fn() -> char) -> Option<String> {
    if !cx.has_embedder() || escape() != '\\' {
        return None;
    }
    continuation::embed_continued(cx, SHELL, text, &SHELL_OPTIONS)
}

/// All tokens of the instruction, nested instructions (`ONBUILD`, `HEALTHCHECK ... CMD`) and
/// JSON arrays included.
fn instruction(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let tokens = || {
        node.descendants_with_tokens()
            .filter_map(|e| e.into_token())
    };
    let facts = Facts::of(tokens());
    // (a lone flag before a heredoc stays on the line)
    let own_lines = facts.mount && !(facts.flags == 1 && facts.heredoc);
    // The escape character: the one the continuations use, else the file's directive's.
    let escape_cell = std::cell::OnceCell::new();
    let escape = || {
        *escape_cell.get_or_init(|| {
            (facts.continuation).unwrap_or_else(|| crate::rewrite::escape_char(cx.grammar, node))
        })
    };
    let mut out = Continued::default();
    let mut heredocs = Vec::new();
    let mut prev = None::<SyntaxToken>;
    let mut blank = false;
    let mut flag_break = false;
    for token in tokens() {
        let t = &token;
        let kind = t.kind();
        if matches!(kind, k::WHITESPACE | k::NEWLINE) {
            blank = true;
            continue;
        }
        if cx.grammar.is_trivia(kind) {
            continue;
        }
        if flag_break && kind != k::LINE_CONTINUATION {
            out.end_line(if escape() == '`' { "`" } else { "\\" });
            prev = None;
            blank = false;
        }
        flag_break = false;
        let leading = cx.leading_comments_doc(t);
        let text = t.text();
        if kind == k::LINE_CONTINUATION
            && facts.shell
            && prev.as_ref().is_some_and(|p| p.kind() == k::KEYWORD)
            && comments::trailing_comments(cx.grammar, t).is_empty()
        {
            // `RUN \` + line break + command: the command goes on the first line.
            blank = true;
            continue;
        }
        if kind == k::LINE_CONTINUATION {
            out.end_line(&text[..1]);
            // Comment lines after the continuation are trailing trivia of it.
            for c in comments::trailing_comments(cx.grammar, t) {
                out.own_line(core_format::context::comment_doc(&c));
            }
            prev = None;
            blank = false;
            continue;
        }
        if prev
            .as_ref()
            .is_some_and(|p| ROWS.get().space().want(p, t).unwrap_or(blank))
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
            k::SHELL_COMMAND => {
                let laid = shell_lines(cx, text, escape).filter(|_| !facts.heredoc);
                match laid {
                    Some(laid) => out.verbatim(&laid),
                    // (shell text that is not laid out stays as written)
                    None if text.contains('\n') => out.raw(text),
                    None => out.push(cx.token_text(t)),
                }
            }
            _ if text.contains('\n') => out.text(text),
            _ => out.push(cx.token_text(t)),
        }
        flag_break = own_lines && kind == k::WORD && text.starts_with("--");
        out.push(cx.trailing_comments_doc(t));
        prev = Some(token);
        blank = false;
    }
    doc::concat(vec![out.finish(), doc::concat(heredocs)])
}
