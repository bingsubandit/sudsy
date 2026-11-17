//! The lexer: the whole file as tokens that tile it (trivia included), one pass, dialect modes.

use super::dialect::Dialect;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum T {
    Ws,
    Nl,
    LineComment,
    BlockComment,
    Word,
    Number,
    /// A string literal (`'..'`, `N'..'`, `E'..'`, `$$..$$`, `X'..'`).
    Str,
    /// A quoted identifier (`".."`, `` `..` ``, `[..]` in T-SQL).
    Quoted,
    /// `@x`, `@@x` (T-SQL, MySQL), `$1`, `##tmp`.
    Var,
    LParen,
    RParen,
    LSquare,
    RSquare,
    LCurly,
    RCurly,
    Comma,
    Semi,
    Dot,
    /// `::`
    Cast,
    Colon,
    /// Any other operator, its text tells which.
    Op,
    /// A character no token starts with.
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub struct Lex {
    pub kind: T,
    pub start: u32,
    pub end: u32,
}

impl T {
    pub fn is_trivia(self) -> bool {
        matches!(self, T::Ws | T::Nl | T::LineComment | T::BlockComment)
    }
}

const OPS3: &[&str] = &[
    "->>", "#>>", "<=>", "!~*", "~~*", "<->", "<#>", "&&&", "|>>", "<<|", "&<|", "|&>", "!~~", "<<=",
    ">>=", "||/", "|/ ",
];
const OPS2: &[&str] = &[
    "<>", "<=", ">=", "!=", "||", "->", "#>", "@>", "<@", "?|", "?&", "&&", "!!", "=>", ":=", "~*",
    "!~", "~~", "<<", ">>", "@@", "#-", "~=", "!<", "!>", "+=", "-=", "*=", "/=", "%=", "&=", "|=",
    "^=", "||", "..",
];
const OP1: &str = "+-*/%<>=!~^&|@#?$\\";

fn is_word_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_word_char(c: char, d: Dialect) -> bool {
    c.is_alphanumeric()
        || c == '_'
        || (c == '$' && d != Dialect::Tsql)
        || (matches!(c, '#' | '@') && d == Dialect::Tsql)
}

/// Lexes `src`; the tokens cover it without gaps.
pub fn lex(src: &str, d: Dialect) -> Vec<Lex> {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(src.len() / 4 + 8);
    let mut i = 0;
    while i < b.len() {
        let (kind, end) = token(src, i, d);
        debug_assert!(end > i);
        out.push(Lex {
            kind,
            start: i as u32,
            end: end as u32,
        });
        i = end;
    }
    out
}

fn find(b: &[u8], from: usize, byte: u8) -> Option<usize> {
    b[from..].iter().position(|&x| x == byte).map(|p| from + p)
}

fn line_end(b: &[u8], from: usize) -> usize {
    find(b, from, b'\n').unwrap_or(b.len())
}

/// The end of a quoted run opened at `i`: `close` ends it, a doubled `close` (and a backslash when
/// `escape`) stays inside. Unterminated: the rest of the file.
fn quoted(b: &[u8], i: usize, close: u8, escape: bool) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        let c = b[j];
        if c == close {
            if b.get(j + 1) == Some(&close) {
                j += 2;
                continue;
            }
            return j + 1;
        }
        j += if escape && c == b'\\' { 2 } else { 1 };
    }
    b.len()
}

fn token(src: &str, i: usize, d: Dialect) -> (T, usize) {
    let b = src.as_bytes();
    let c = b[i];
    let next = b.get(i + 1).copied();
    match c {
        b'\n' => return (T::Nl, i + 1),
        b'\r' => {
            return (T::Nl, if next == Some(b'\n') { i + 2 } else { i + 1 });
        }
        b' ' | b'\t' | 0x0b | 0x0c => {
            let mut j = i + 1;
            while j < b.len() && matches!(b[j], b' ' | b'\t' | 0x0b | 0x0c) {
                j += 1;
            }
            return (T::Ws, j);
        }
        b'-' if next == Some(b'-') => return (T::LineComment, line_end_cr(b, i)),
        b'#' if matches!(d, Dialect::Mysql | Dialect::Ansi) => {
            return (T::LineComment, line_end_cr(b, i));
        }
        b'/' if next == Some(b'*') => return (T::BlockComment, block_comment(b, i, d)),
        b'\\' if d == Dialect::Postgres => return (T::LineComment, line_end_cr(b, i)),
        b'\'' => return (T::Str, quoted(b, i, b'\'', d == Dialect::Mysql || d == Dialect::Ansi)),
        b'"' => {
            let esc = matches!(d, Dialect::Mysql | Dialect::Ansi);
            let end = quoted(b, i, b'"', esc);
            return (if d == Dialect::Mysql { T::Str } else { T::Quoted }, end);
        }
        b'`' => return (T::Quoted, find(b, i + 1, b'`').map_or(b.len(), |p| p + 1)),
        b'[' if d == Dialect::Tsql => {
            let end = find(b, i + 1, b']').map_or(b.len(), |p| p + 1);
            return (T::Quoted, end);
        }
        b'(' => return (T::LParen, i + 1),
        b')' => return (T::RParen, i + 1),
        b'[' => return (T::LSquare, i + 1),
        b']' => return (T::RSquare, i + 1),
        b'{' => return (T::LCurly, i + 1),
        b'}' => return (T::RCurly, i + 1),
        b',' => return (T::Comma, i + 1),
        b';' => return (T::Semi, i + 1),
        b':' if next == Some(b':') => return (T::Cast, i + 2),
        b':' if next == Some(b'=') => return (T::Op, i + 2),
        b':' => return (T::Colon, i + 1),
        b'.' if !next.is_some_and(|n| n.is_ascii_digit()) => return (T::Dot, i + 1),
        _ => {}
    }
    if c.is_ascii_digit() || (c == b'.' && next.is_some_and(|n| n.is_ascii_digit())) {
        return number(src, i, d);
    }
    if c == b'$' {
        if let Some(end) = dollar_string(src, i, d) {
            return (T::Str, end);
        }
        if d == Dialect::Postgres && next.is_some_and(|n| n.is_ascii_digit()) {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            return (T::Var, j);
        }
    }
    if matches!(c, b'@' | b'#') && matches!(d, Dialect::Tsql | Dialect::Mysql) {
        let mut j = i + 1;
        if b.get(j) == Some(&c) {
            j += 1;
        }
        let start = j;
        while j < b.len() && is_word_char(src[j..].chars().next().unwrap_or(' '), Dialect::Mysql) {
            j += src[j..].chars().next().map_or(1, char::len_utf8);
        }
        if j > start {
            return (T::Var, j);
        }
    }
    let ch = src[i..].chars().next().unwrap_or(' ');
    if is_word_start(ch) || (ch.is_ascii_digit()) {
        return word(src, i, ch, d);
    }
    if d == Dialect::Tsql && matches!(ch, '#' | '@' | '$') {
        return word(src, i, ch, d);
    }
    for (ops, n) in [(OPS3, 3), (OPS2, 2)] {
        if let Some(s) = src.get(i..i + n) {
            if ops.contains(&s) && op_allowed(s, d) {
                return (T::Op, i + n);
            }
        }
    }
    if OP1.contains(ch) {
        return (T::Op, i + 1);
    }
    (T::Unknown, i + ch.len_utf8())
}

/// Operators only some dialects have; the other ones lex as their single characters.
fn op_allowed(op: &str, d: Dialect) -> bool {
    match op {
        "&&" | "||" | "!!" | "->" | "->>" | "#>" | "#>>" | "@>" | "<@" | "?|" | "?&" | "<->" | "<#>"
        | "<=>" | "@@" | "~=" | "#-" | "&&&" | "|>>" | "<<|" | "&<|" | "|&>" | "!~~" | "||/"
        | "~~*" | "!~*" => d != Dialect::Tsql || op == "||",
        ".." => false,
        _ => true,
    }
}

fn line_end_cr(b: &[u8], from: usize) -> usize {
    let e = line_end(b, from);
    if e > from && b[e - 1] == b'\r' { e - 1 } else { e }
}

fn block_comment(b: &[u8], i: usize, d: Dialect) -> usize {
    let nested = matches!(d, Dialect::Postgres | Dialect::Tsql);
    let (mut j, mut depth) = (i + 2, 1);
    while j < b.len() {
        if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
            depth -= 1;
            j += 2;
            if depth == 0 || !nested {
                return j;
            }
        } else if nested && b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
            depth += 1;
            j += 2;
        } else {
            j += 1;
        }
    }
    b.len()
}

/// `$tag$ ... $tag$` (the tag may be empty); `None` when `i` does not open one.
fn dollar_string(src: &str, i: usize, d: Dialect) -> Option<usize> {
    if d == Dialect::Tsql {
        return None;
    }
    let b = src.as_bytes();
    let mut j = i + 1;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    if b.get(j) != Some(&b'$') || (j > i + 1 && b[i + 1].is_ascii_digit()) {
        return None;
    }
    let tag = &src[i..=j];
    Some(src[j + 1..].find(tag).map_or(b.len(), |p| j + 1 + p + tag.len()))
}

fn number(src: &str, i: usize, d: Dialect) -> (T, usize) {
    let b = src.as_bytes();
    let digits = |mut j: usize| {
        while j < b.len() && (b[j].is_ascii_digit() || (b[j] == b'_' && d == Dialect::Postgres)) {
            j += 1;
        }
        j
    };
    if b[i] == b'0' && matches!(b.get(i + 1), Some(b'x' | b'X')) && d != Dialect::Postgres {
        let mut j = i + 2;
        while j < b.len() && b[j].is_ascii_hexdigit() {
            j += 1;
        }
        return (T::Number, j);
    }
    let mut j = digits(i);
    if b.get(j) == Some(&b'.') && b.get(j + 1) != Some(&b'.') {
        j = digits(j + 1);
    }
    if matches!(b.get(j), Some(b'e' | b'E')) {
        let mut k = j + 1;
        if matches!(b.get(k), Some(b'+' | b'-')) {
            k += 1;
        }
        if b.get(k).is_some_and(u8::is_ascii_digit) {
            j = digits(k);
        }
    }
    // A word that starts with digits (`1abc`) is one word.
    if src[j..].chars().next().is_some_and(is_word_start) {
        return word(src, i, ' ', d);
    }
    (T::Number, j)
}

fn word(src: &str, i: usize, first: char, d: Dialect) -> (T, usize) {
    let b = src.as_bytes();
    // Prefixed strings: `N'..'`, `E'..'`, `X'..'`, `B'..'`, `U&'..'`.
    if b.get(i + 1) == Some(&b'\'') && matches!(b[i], b'n' | b'N' | b'e' | b'E' | b'x' | b'X' | b'b' | b'B') {
        let escape = matches!(b[i], b'e' | b'E') || d == Dialect::Mysql;
        return (T::Str, quoted(b, i + 1, b'\'', escape));
    }
    if d == Dialect::Postgres
        && matches!(b[i], b'u' | b'U')
        && b.get(i + 1) == Some(&b'&')
        && matches!(b.get(i + 2), Some(b'\'' | b'"'))
    {
        let q = b[i + 2];
        return (if q == b'"' { T::Quoted } else { T::Str }, quoted(b, i + 2, q, false));
    }
    let mut j = i + first.len_utf8().max(1);
    if first == ' ' {
        j = i;
    }
    while j < src.len() {
        let ch = src[j..].chars().next().unwrap_or(' ');
        if !is_word_char(ch, d) {
            break;
        }
        j += ch.len_utf8();
    }
    if j == i {
        j = i + first.len_utf8().max(1);
    }
    (T::Word, j)
}
