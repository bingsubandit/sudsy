//! Prism's lexer tokens and node spans as our lossless tree.
//!
//! `ffi` parses once and returns the tokens of the lex callback and the node spans (the C visitor).
//! Newlines, comments and the separators of `%w[]` are not tokens here: they are the text between
//! tokens, which `replay` splits (the grammar's `trivia` rows know `#` and `=begin` blocks and `\` line
//! continuations). The text after `__END__` is one `Data` token. A node kind of Prism
//! (`PM_CALL_NODE`) is the row `Call` of `grammars/ruby.grammar`. A heredoc's body is lexed after
//! the rest of its opening line and Prism ends the nodes of that line before it: the body (text,
//! interpolations, label) is one `HeredocBody` token, without nodes of its own, and the nodes that
//! hold the whole line are extended over it, so it is a child of the innermost of them.
//!
//! concept: adapter-replay

mod ffi;

use crate::generated::kind as k;
use core_syntax::codegen::from_screaming;
use core_syntax::{SyntaxKind, grammars};
use provider_api::adapter::{KindNames, Node, Tok, replay_grammar};
use provider_api::{ParseError, ParseResult, SourceFile};

/// Prism node types with no row: the root (`replay` makes it), and the implicit parameters of a
/// block (`it`, `_1`), which Prism spans over the whole block.
const EXEMPT: &[&str] = &["Program", "ItParameters", "NumberedParameters"];

pub fn parse_file(file: &SourceFile) -> ParseResult {
    parse(&file.text)
}

pub fn parse(src: &str) -> ParseResult {
    let parsed = ffi::parse(src);
    let (tokens, heredocs) = tokens(src, &parsed.tokens);
    let mut names = KindNames::new(grammars::RUBY.get(), 512, |n| EXEMPT.contains(&n));
    let mut nodes: Vec<Node> = parsed
        .nodes
        .iter()
        .filter(|n| !in_bodies(&heredocs, n.start))
        .filter_map(|n| {
            let kind = names.get(n.kind as usize, false, || node_kind_name(n.kind))?;
            Some(Node {
                start: n.start,
                end: n.end,
                kind,
            })
        })
        .collect();
    extend_over_heredocs(&mut nodes, &heredocs);
    // The visitor goes by field, not by position (`a if b` visits `b` first); a stable sort keeps a
    // parent before a child of the same span.
    core_syntax::spans::sort_nested(&mut nodes, |n| (n.start, n.end, 0u8));
    let mut result = replay_grammar(src, grammars::RUBY.get(), &tokens, &nodes, |t, _| t);
    let mut errors: Vec<ParseError> = parsed
        .errors
        .iter()
        .map(|e| ParseError::syntax(e.start, e.len, e.message.clone()))
        .collect();
    errors.append(&mut result.errors);
    errors.sort_by_key(|e| e.start);
    result.errors = errors;
    result
}

/// `PM_MULTI_TARGET_NODE` is `MultiTarget`.
fn node_kind_name(kind: u16) -> String {
    let name = ffi::node_name(kind);
    let name = name.trim_start_matches("PM_").trim_end_matches("_NODE");
    if name == "SELF" {
        return "This".into(); // `Self` is not a Rust type name
    }
    from_screaming(name)
}

/// Our tokens of the lexer's: the ones that are real text, in source order, and the data after
/// `__END__`.
fn tokens(src: &str, raw: &[ffi::Token]) -> (Vec<Tok>, Vec<Heredoc>) {
    let mut kinds: Vec<Option<Option<SyntaxKind>>> = vec![None; 512];
    let mut out: Vec<Tok> = Vec::with_capacity(raw.len());
    let mut data_from = None;
    let (mut openers, mut heredocs) = (std::collections::VecDeque::new(), Vec::new());
    for t in raw {
        let kind =
            *kinds[t.kind as usize].get_or_insert_with(|| token_kind(&ffi::token_name(t.kind)));
        let Some(kind) = kind else { continue };
        if t.start == t.end {
            continue;
        }
        // Heredocs of one line close in the order they open.
        if kind == k::DOC_START {
            openers.push_back((t.start, t.end));
        } else if kind == k::DOC_END
            && let Some((start, end)) = openers.pop_front()
        {
            let line = src[end as usize..]
                .find('\n')
                .map_or(src.len(), |i| end as usize + i + 1);
            // Another heredoc of the opener's line has its body first.
            let previous = heredocs.last().map_or(0, |h: &Heredoc| h.body.end);
            let from = (line as u32).max(if previous > end { previous } else { 0 });
            heredocs.push(Heredoc {
                opener: start..end,
                body: from..t.end,
                line_end: end,
            });
        }
        if kind == k::KEYWORD && src[t.start as usize..].starts_with("__END__") {
            data_from = Some(t.end);
        }
        out.push(Tok {
            kind,
            start: t.start,
            end: t.end,
        });
    }
    // A heredoc's body is one token: its text, its interpolations and its label.
    if !heredocs.is_empty() {
        out.sort_by_key(|t| t.start);
        out.retain(|t| !in_bodies(&heredocs, t.start));
        for h in &mut heredocs {
            let i = out.partition_point(|t| t.start < h.body.start);
            h.line_end = out[..i]
                .last()
                .map_or(h.line_end, |t| t.end.max(h.opener.end));
        }
        out.extend(heredocs.iter().map(|h| Tok {
            kind: k::HEREDOC_BODY,
            start: h.body.start,
            end: h.body.end,
        }));
    }
    // The lexer reports a heredoc's body after the rest of its opening line.
    if !out.is_sorted_by_key(|t| t.start) {
        out.sort_by_key(|t| t.start);
    }
    if let Some(from) = data_from.filter(|&f| (f as usize) < src.len()) {
        out.push(Tok {
            kind: k::DATA,
            start: from,
            end: src.len() as u32,
        });
    }
    (out, heredocs)
}

/// A heredoc: its opener (`<<~EOS`) and its body.
struct Heredoc {
    opener: std::ops::Range<u32>,
    /// From the end of the opener's line (or of the heredoc before it on that line) to the end of
    /// the closing label.
    body: std::ops::Range<u32>,
    /// The end of the last token before the body: the rest of the opener's line.
    line_end: u32,
}

/// Whether `pos` is inside the body of one of `heredocs` (in source order).
fn in_bodies(heredocs: &[Heredoc], pos: u32) -> bool {
    let i = heredocs.partition_point(|h| h.body.start <= pos);
    i > 0 && pos < heredocs[i - 1].body.end
}

/// Prism ends the node of a statement at its opener's line, before the heredoc body that is
/// written after it: every node that holds an opener ends after the body instead (the string of
/// the opener itself does not: the rest of the line is not its text).
fn extend_over_heredocs(nodes: &mut [Node], heredocs: &[Heredoc]) {
    for h in heredocs {
        for n in nodes.iter_mut() {
            // The string of the opener keeps out of the rest of the line, if there is some.
            let is_string =
                n.start == h.opener.start && n.end == h.opener.end && h.line_end > h.opener.end;
            if n.start <= h.opener.start && n.end >= h.line_end && n.end < h.body.end && !is_string
            {
                n.end = h.body.end;
            }
        }
    }
}

/// Our token kind of the lexer's token type; `None` for what is not a token of ours (newlines,
/// comments, the separators of word lists, the end of the file).
fn token_kind(name: &str) -> Option<SyntaxKind> {
    Some(match name {
        "EOF" | "NEWLINE" | "NEWLINE_TERMINATOR" | "IGNORED_NEWLINE" | "COMMENT"
        | "EMBDOC_BEGIN" | "EMBDOC_LINE" | "EMBDOC_END" | "WORDS_SEP" | "WORDS_SEP_IMPLICIT" => {
            return None;
        }
        "IDENTIFIER" | "METHOD_NAME" => k::IDENTIFIER,
        "CONSTANT" => k::CONSTANT,
        "INSTANCE_VARIABLE" | "CLASS_VARIABLE" | "GLOBAL_VARIABLE" | "BACK_REFERENCE"
        | "NUMBERED_REFERENCE" => k::VARIABLE,
        "LABEL" => k::LABEL,
        "STRING_CONTENT" | "CHARACTER_LITERAL" => k::STRING_PART,
        "STRING_BEGIN" | "STRING_END" | "XSTRING_BEGIN" | "BACKTICK" | "SYMBOL_BEGIN"
        | "REGEXP_BEGIN" | "REGEXP_END" | "LABEL_END" => k::QUOTE,
        "HEREDOC_START" => k::DOC_START,
        "HEREDOC_END" => k::DOC_END,
        "EMBEXPR_BEGIN" => k::EMB_BEGIN,
        "EMBVAR" => k::EMB_VAR,
        "BRACE_LEFT" | "BRACE_LEFT_ARGUMENT" | "BRACE_LEFT_HASH" | "LAMBDA_BEGIN" => k::L_BRACE,
        "BRACE_RIGHT" | "EMBEXPR_END" => k::R_BRACE,
        "BRACKET_LEFT" | "BRACKET_LEFT_ARRAY" => k::L_BRACKET,
        "BRACKET_RIGHT" => k::R_BRACKET,
        "SEMICOLON" => k::SEMI,
        "COMMA" => k::COMMA,
        "COLON" => k::COLON,
        "COLON_COLON" | "UCOLON_COLON" => k::COLON_COLON,
        "DOT" | "AMPERSAND_DOT" => k::DOT,
        "EQUAL" => k::ASSIGN,
        "__END__" => k::KEYWORD,
        _ if name.starts_with("KEYWORD_") => k::KEYWORD,
        _ if name.starts_with("PARENTHESIS_LEFT") => k::L_PAREN,
        _ if name == "PARENTHESIS_RIGHT" => k::R_PAREN,
        _ if name.starts_with("INTEGER") || name.starts_with("FLOAT") => k::NUMBER,
        _ if name.starts_with("PERCENT_") && name != "PERCENT_EQUAL" => k::QUOTE,
        _ => k::OPERATOR,
    })
}
