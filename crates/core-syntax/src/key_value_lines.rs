//! A parser of files that are one `key<separator>value` entry per line, with comment lines and
//! blank lines: a line with a non-empty key before its first separator is an entry (key, separator,
//! value, each trimmed, the blanks between them trivia), a comment line starts with the comment
//! byte, any other line is one stray text. The language gives the token and node kinds and the two
//! bytes; nothing is an error, what a line means is for the rules.
//!
//! concept: key-value-line-parser
//! what: Parses a file of `key: value` lines, comment lines and stray text lines into a lossless tree.
//! tools: foreman, honcho, overmind (Procfile line reading)
//! reads: path:key_value_lines::Lines

use crate::SyntaxKind;
use crate::parse::{Lexer, Parsed, Parser, is_blank, is_eol};

/// What a language gives: the kinds of its grammar and the bytes of its syntax.
pub struct Lines {
    pub document: SyntaxKind,
    pub entry: SyntaxKind,
    pub stray: SyntaxKind,
    pub whitespace: SyntaxKind,
    pub newline: SyntaxKind,
    pub comment: SyntaxKind,
    pub key: SyntaxKind,
    pub separator: SyntaxKind,
    pub value: SyntaxKind,
    pub text: SyntaxKind,
    pub eof: SyntaxKind,
    /// The language's `kind::TRIVIA_MASK`.
    pub trivia: u64,
    /// The byte that starts a comment line (after blanks).
    pub comment_byte: u8,
    /// The byte that ends the key.
    pub separator_byte: u8,
    /// A line that starts with blanks (and is no comment) is one stray text, blanks included: the
    /// format reading it does not take it for an entry.
    pub indented_text: bool,
}

pub fn parse(src: &str, l: &Lines) -> Parsed {
    let mut lx = Lexer::new(src, src.len() / 8 + 8);
    while lx.pos < src.len() {
        let at_start = lx.tokens.last().is_none_or(|t| t.kind == l.newline);
        if lx.blank(l.whitespace, l.newline, is_blank) {
            let content = !is_eol(lx.at(0)) && lx.at(0) != 0 && lx.at(0) != l.comment_byte;
            let ws = lx
                .tokens
                .last()
                .filter(|t| t.kind == l.whitespace && at_start);
            if let (true, true, Some(&ws)) = (l.indented_text, content, ws) {
                lx.tokens.pop();
                lx.eat_line();
                lx.trim_blanks_back(ws.end);
                lx.emit(l.text, ws.start);
            }
        } else {
            line(&mut lx, l);
        }
    }
    let mut p = Parser::new(src, lx.finish(l.eof), l.trivia, l.eof);
    p.start(l.document);
    while !p.at_eof() {
        let entry = p.cur() == l.key;
        p.start(if entry { l.entry } else { l.stray });
        p.bump();
        if entry {
            p.bump(); // the separator
            if p.cur() == l.value {
                p.bump();
            }
        }
        p.finish();
    }
    p.bump();
    p.finish();
    p.done()
}

/// The tokens of the line from its first non-blank byte, up to its trailing blanks.
fn line(lx: &mut Lexer<'_>, l: &Lines) {
    let start = lx.pos;
    lx.eat_line();
    lx.trim_blanks_back(start);
    let end = lx.pos;
    let text = &lx.src[start..end];
    let is_comment = text.as_bytes()[0] == l.comment_byte;
    let sep = text
        .bytes()
        .position(|b| b == l.separator_byte)
        .filter(|&i| i > 0 && !is_comment)
        .map(|i| start + i);
    let emit = |lx: &mut Lexer<'_>, kind: SyntaxKind, from: usize, to: usize| {
        lx.pos = to;
        lx.emit(kind, from);
    };
    let Some(sep) = sep else {
        emit(lx, if is_comment { l.comment } else { l.text }, start, end);
        return;
    };
    let key = start + text[..sep - start].trim_end_matches([' ', '\t']).len();
    emit(lx, l.key, start, key);
    if key < sep {
        emit(lx, l.whitespace, key, sep);
    }
    emit(lx, l.separator, sep, sep + 1);
    let b = lx.src.as_bytes();
    let value = (sep + 1..end).find(|&i| !is_blank(b[i])).unwrap_or(end);
    if value > sep + 1 {
        emit(lx, l.whitespace, sep + 1, value);
    }
    if value < end {
        emit(lx, l.value, value, end);
    }
    lx.pos = end;
}
