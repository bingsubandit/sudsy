//! The kinds and bytes of a Procfile for the shared line parser.
//!
//! concept: none (the kinds of Procfile on core_syntax::key_value_lines)

use crate::generated::kind as k;
use core_syntax::key_value_lines::{Lines, parse};
use provider_api::{ParseResult, SourceFile};

const LINES: Lines = Lines {
    document: k::DOCUMENT,
    entry: k::PROCESS,
    stray: k::STRAY,
    whitespace: k::WHITESPACE,
    newline: k::NEWLINE,
    comment: k::COMMENT,
    key: k::NAME,
    separator: k::COLON,
    value: k::COMMAND,
    text: k::TEXT,
    eof: k::EOF,
    trivia: k::TRIVIA_MASK,
    comment_byte: b'#',
    separator_byte: b':',
    indented_text: true,
};

pub fn parse_file(file: &SourceFile) -> ParseResult {
    let r = parse(&file.text, &LINES);
    ParseResult {
        tree: r.tree,
        errors: r.errors,
    }
}
