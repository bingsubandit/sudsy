//! ktrs-parser's tree (a port of the Kotlin compiler's PSI parser) as our lossless tree.
//!
//! The tree is one flat pre-order array of nodes and tokens, whitespace and comments included,
//! and its tokens tile the text exactly. The adapter walks it once: a node is a row of
//! `grammars/kotlin.grammar` named like the PSI element type, a token is our token of its kind
//! (every keyword is `Keyword`), and whitespace, comments and KDoc are left to `replay`, which
//! splits the text between tokens by the grammar's `trivia` row.
//!
//! What ktrs needs from its caller is declared in the grammar (`limits`: the parse thread's stack,
//! the largest file, the nesting it cannot recurse through and the nested annotation arguments it
//! is exponential in) and done by `provider_api::adapter::guarded`; it requires `\n` line breaks
//! (`Lf` maps the offsets back).

use crate::generated::kind as k;
use core_syntax::codegen::from_screaming;
use core_syntax::{SyntaxKind, grammars};
use ktrs_parser::FileKind;
use ktrs_syntax::SyntaxKind as P;
use provider_api::adapter::{KindNames, Lf, Node, Tok, guarded, replay_grammar};
use provider_api::{ParseError, ParseResult, SourceFile};

pub fn parse_file(file: &SourceFile) -> ParseResult {
    let kind = if file.id.ends_with(".kts") {
        FileKind::Script
    } else {
        FileKind::Source
    };
    parse(&file.text, kind)
}

pub fn parse(src: &str, kind: FileKind) -> ParseResult {
    // ktrs wants `\n`.
    let lf = Lf::new(src);
    let grammar = grammars::KOTLIN.get();
    let walked = guarded(src, grammar, || walk(&lf, kind));
    let (tokens, nodes, mut errors) = match walked {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    let mut result = replay_grammar(src, grammar, &tokens, &nodes, |t, _| t);
    errors.append(&mut result.errors);
    errors.sort_by_key(|e| e.start);
    result.errors = errors;
    result
}

/// Parses the text and collects our tokens, our nodes and the parse errors, with offsets in the
/// original text.
fn walk(lf: &Lf, kind: FileKind) -> (Vec<Tok>, Vec<Node>, Vec<ParseError>) {
    let text = &*lf.text;
    let parsed = ktrs_parser::parse_file(text, kind);
    let to_src = |n: usize| lf.to_src(n);
    let tree = &parsed.tree;
    let grammar = grammars::KOTLIN.get();
    let mut names = KindNames::new(grammar, 512, |_| true);
    // Our token kind per ktrs kind (`Some(None)`: a token that is not ours), found on first use.
    let mut token_kinds: Vec<Option<Option<SyntaxKind>>> = vec![None; 512];
    let mut tokens = Vec::with_capacity(tree.len() / 2);
    let mut nodes = Vec::with_capacity(tree.len() / 2);
    let mut errors = Vec::new();
    let mut messages = parsed.error_messages.iter();
    let mut e = 1;
    while e < tree.len() as u32 {
        let kind = tree.kind(e);
        let range = tree.text_range(e);
        let (start, end) = (usize::from(range.start()), usize::from(range.end()));
        if tree.is_token(e) {
            e += 1;
            let ours = *token_kinds[kind as usize].get_or_insert_with(|| {
                token_name(kind).map(|n| {
                    let found = match n {
                        Some(name) => grammar.kind_by_name(name),
                        None => (grammar.tokens().iter())
                            .find(|d| d.text.as_deref() == Some(&text[start..end]))
                            .map(|d| d.kind),
                    };
                    found.filter(|k| k.is_token()).unwrap_or(k::UNKNOWN)
                })
            });
            if kind == P::BLOCK_COMMENT && grammar.trivia.block_end(&text[start..end]).is_none() {
                errors.push(unclosed(to_src(start), to_src(end)));
            }
            if let Some(kind) = ours.filter(|_| start < end) {
                tokens.push(Tok {
                    kind,
                    start: to_src(start),
                    end: to_src(end),
                });
            }
        } else if kind == P::DOC_COMMENT {
            if grammar.trivia.block_end(&text[start..end]).is_none() {
                errors.push(unclosed(to_src(start), to_src(end)));
            }
            e = tree.subtree_end(e);
        } else {
            e += 1;
            if kind == P::ERROR_ELEMENT {
                let message = messages.next().map_or("syntax error", String::as_str);
                errors.push(ParseError::syntax(
                    to_src(start),
                    to_src(end) - to_src(start),
                    message,
                ));
            }
            if let Some(kind) = names.get(kind as usize, false, || {
                from_screaming(&format!("{kind:?}"))
            }) {
                nodes.push(Node {
                    start: to_src(start),
                    end: to_src(end),
                    kind,
                });
            }
        }
    }
    (tokens, nodes, errors)
}

fn unclosed(start: u32, end: u32) -> ParseError {
    ParseError::syntax(start, end - start, "Unclosed comment")
}

/// The grammar's token name of a ktrs token (`Some(None)`: the grammar token with its text), `None` for the ones that are not tokens of ours: trivia (left to `replay`) and
/// the parser's zero-width markers.
fn token_name(kind: P) -> Option<Option<&'static str>> {
    Some(Some(match kind {
        P::WHITE_SPACE
        | P::EOL_COMMENT
        | P::BLOCK_COMMENT
        | P::SHEBANG_COMMENT
        | P::EOF
        | P::EOL_OR_SEMICOLON
        | P::DANGLING_NEWLINE => return None,
        P::IDENTIFIER | P::FIELD_IDENTIFIER => "Identifier",
        P::INTEGER_LITERAL => "Integer",
        P::FLOAT_LITERAL => "Float",
        P::CHARACTER_LITERAL => "Character",
        P::OPEN_QUOTE => "OpenQuote",
        P::CLOSING_QUOTE => "ClosingQuote",
        P::REGULAR_STRING_PART => "StringPart",
        P::ESCAPE_SEQUENCE => "EscapeSequence",
        P::INTERPOLATION_PREFIX => "InterpolationPrefix",
        P::SHORT_TEMPLATE_ENTRY_START => "ShortTemplateEntryStart",
        P::LONG_TEMPLATE_ENTRY_START => "LongTemplateEntryStart",
        P::LONG_TEMPLATE_ENTRY_END => "LongTemplateEntryEnd",
        P::RESERVED => "Unknown",
        P::AS_SAFE | P::NOT_IN | P::NOT_IS => "Keyword",
        _ if format!("{kind:?}").ends_with("_KEYWORD") => "Keyword",
        _ => return Some(None),
    }))
}
