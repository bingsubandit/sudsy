//! The lexer: the whole file as tokens that tile it (trivia included), one pass, dialect modes.
//!
//! concept: source-lexer

use super::dialect::Dialect;
use crate::generated::kind as k;

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
    /// The rows after `COPY .. FROM STDIN;`, up to the `\\.` line (PostgreSQL).
    Data,
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

const OPS5: &[&str] = &["<<->>", "<<#>>"];
const OPS4: &[&str] = &["!~~*"];
const OPS3: &[&str] = &[
    "-|-", "|=|", "->>", "#>>", "<=>", "!~*", "~~*", "<->", "<#>", "<+>", "&&&", "|>>", "<<|",
    "&<|", "|&>", "!~~", "<<=", ">>=", "||/",
];
const OPS2: &[&str] = &[
    "|/", "<>", "<=", ">=", "&>", "&<", "@?", "!=", "||", "->", "#>", "@>", "<@", "?|", "?&", "&&",
    "!!", "=>", ":=", "~*", "!~", "~~", "<<", ">>", "@@", "#-", "~=", "!<", "!>", "+=", "-=", "*=",
    "/=", "%=", "&=", "|=", "^=", "||", "..",
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

/// Lexes `src`; the tokens cover it without gaps. The second result lists the tokens left open
/// at the end of the file (strings, quoted names, comments).
pub fn lex(src: &str, d: Dialect) -> (Vec<Lex>, Vec<u32>) {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(src.len() / 4 + 8);
    let mut open = Vec::new();
    let mut i = 0;
    // mysql `DELIMITER ~`: the delimiter token ends statements, `DELIMITER ;` restores `;`.
    let mut delim: Option<&str> = None;
    let mut declared = usize::MAX;
    let mut stmt_start = 0usize;
    while i < b.len() {
        let mut left_open = false;
        let (kind, end) = match delim {
            _ if i == declared => (
                T::Op,
                i + src[i..].find(char::is_whitespace).unwrap_or(src.len() - i),
            ),
            Some(x) if src[i..].starts_with(x) => (T::Semi, i + x.len()),
            _ => token(src, i, d, &mut left_open),
        };
        // A word ends where the custom delimiter starts (`t$$$` with delimiter `$$$`).
        let end = match (delim, kind) {
            (Some(x), T::Word) => src[i + 1..end].find(x).map_or(end, |p| i + 1 + p),
            _ => end,
        };
        if d == Dialect::Mysql
            && kind == T::Word
            && stmt_start == out.len()
            && src[i..end].eq_ignore_ascii_case("delimiter")
        {
            let rest = &src[end..end + rest_line_end(&src[end..])];
            let skip = rest.len() - rest.trim_start_matches([' ', '\t']).len();
            let word = rest[skip..].split_whitespace().next().unwrap_or("");
            if !word.is_empty() && !rest[..skip].contains('\n') {
                delim = (word != ";").then_some(word);
                if delim.is_some() {
                    declared = end + skip;
                }
            }
        }
        if left_open {
            open.push(out.len() as u32);
        }
        debug_assert!(
            end > i,
            "zero token at {i}: {kind:?} {:?} declared {declared}",
            &src[i..src.len().min(i + 12)]
        );
        out.push(Lex {
            kind,
            start: i as u32,
            end: end as u32,
        });
        i = end;
        if kind == T::Semi {
            // `COPY .. FROM STDIN;` and its data rows are one token, as the statement is taken as written.
            if d == Dialect::Postgres && is_copy_stdin(src, &out[stmt_start..]) {
                let (at, to) = copy_data(src, i);
                if at != usize::MAX {
                    let first = out[stmt_start].start;
                    out.truncate(stmt_start);
                    out.push(Lex {
                        kind: T::Data,
                        start: first,
                        end: to as u32,
                    });
                    i = to;
                }
            }
            stmt_start = out.len();
        }
    }
    (out, open)
}

/// Is the statement (its tokens, ending in `;`) a `COPY .. FROM STDIN`?
fn is_copy_stdin(src: &str, toks: &[Lex]) -> bool {
    let mut words = toks
        .iter()
        .filter(|t| t.kind == T::Word)
        .map(|t| &src[t.start as usize..t.end as usize]);
    words.next().is_some_and(|w| w.eq_ignore_ascii_case("copy")) && {
        let rest: Vec<&str> = words.collect();
        rest.windows(2)
            .any(|p| p[0].eq_ignore_ascii_case("from") && p[1].eq_ignore_ascii_case("stdin"))
    }
}

/// The data rows from the line after `from`: start and end, the end before the `\\.` line's newline.
fn copy_data(src: &str, from: usize) -> (usize, usize) {
    let Some(nl) = src[from..].find('\n') else {
        return (usize::MAX, 0);
    };
    let start = from + nl + 1;
    let mut pos = start;
    while pos < src.len() {
        let end = src[pos..].find('\n').map_or(src.len(), |n| pos + n);
        if src[pos..end].trim_end() == "\\." {
            return (start, end);
        }
        pos = end + 1;
    }
    (start, src.len())
}

/// A psql meta-command runs to the end of its line, or past it while a `(` is open.
fn meta_command_end(b: &[u8], i: usize) -> usize {
    let mut end = line_end_cr(b, i);
    let depth = |from: usize, to: usize| {
        b[from..to].iter().fold(0i32, |d, &c| {
            d + i32::from(c == b'(') - i32::from(c == b')')
        })
    };
    let mut open = depth(i, end);
    while open > 0 && end < b.len() {
        let next = line_end_cr(b, end + 1);
        open += depth(end, next);
        end = next;
    }
    end
}

fn rest_line_end(s: &str) -> usize {
    s.find('\n').unwrap_or(s.len())
}

fn find(b: &[u8], from: usize, byte: u8) -> Option<usize> {
    b[from..].iter().position(|&x| x == byte).map(|p| from + p)
}

fn line_end(b: &[u8], from: usize) -> usize {
    find(b, from, b'\n').unwrap_or(b.len())
}

/// The end of the quoted run opened at `i`, read by the `string` row of the dialect's escape
/// (`escape`: a backslash escapes). Unterminated: the rest of the file.
fn quoted(src: &str, i: usize, escape: bool, open: &mut bool) -> usize {
    let row = (core_syntax::grammars::SQL.get())
        .string_rows(k::STRING)
        .nth(usize::from(escape))
        .expect("the SQL grammar has two string rows");
    let (end, closed) = row.scan(src, i, false);
    *open |= !closed;
    end
}

fn token(src: &str, i: usize, d: Dialect, open: &mut bool) -> (T, usize) {
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
        b'/' if next == Some(b'*') => return (T::BlockComment, block_comment(b, i, d, open)),
        b'\\' if d == Dialect::Postgres => return (T::LineComment, meta_command_end(b, i)),
        b'\'' => {
            let end = quoted(src, i, d == Dialect::Mysql || d == Dialect::Ansi, open);
            return (
                T::Str,
                if d == Dialect::Postgres {
                    uescape(src, end)
                } else {
                    end
                },
            );
        }
        b'"' => {
            let esc = matches!(d, Dialect::Mysql | Dialect::Ansi);
            let end = quoted(src, i, esc, open);
            return (
                if d == Dialect::Mysql {
                    T::Str
                } else {
                    T::Quoted
                },
                end,
            );
        }
        b'`' => {
            return (
                T::Quoted,
                find(b, i + 1, b'`').map_or_else(
                    || {
                        *open = true;
                        b.len()
                    },
                    |p| p + 1,
                ),
            );
        }
        b'[' if d == Dialect::Tsql => {
            let end = find(b, i + 1, b']').map_or_else(
                || {
                    *open = true;
                    b.len()
                },
                |p| p + 1,
            );
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
        b':' if d == Dialect::Postgres && matches!(next, Some(b'\'' | b'"')) => {
            return (T::Var, quoted(src, i + 1, false, open));
        }
        b'U' | b'u'
            if d == Dialect::Postgres
                && next == Some(b'&')
                && matches!(b.get(i + 2), Some(b'\'' | b'"')) =>
        {
            let q = b[i + 2];
            let end = quoted(src, i + 2, false, open);
            return (
                if q == b'"' { T::Quoted } else { T::Str },
                uescape(src, end),
            );
        }
        b':' if next == Some(b'=') => return (T::Op, i + 2),
        b':' => return (T::Colon, i + 1),
        b'.' if !next.is_some_and(|n| n.is_ascii_digit())
            || (i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) =>
        {
            return (T::Dot, i + 1);
        }
        _ => {}
    }
    if c.is_ascii_digit() || (c == b'.' && next.is_some_and(|n| n.is_ascii_digit())) {
        return number(src, i, d, open);
    }
    if c == b'$' {
        if let Some(end) = dollar_string(src, i, d, open) {
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
    if (c == b'@' && matches!(d, Dialect::Tsql | Dialect::Mysql))
        || (c == b'#' && matches!(d, Dialect::Tsql | Dialect::Mysql))
    {
        let mut j = i + 1;
        if b.get(j) == Some(&c) {
            j += 1;
        }
        let start = j;
        while j < b.len() && is_word_char(src[j..].chars().next().unwrap_or(' '), d) {
            j += src[j..].chars().next().map_or(1, char::len_utf8);
        }
        // `@@global.time_zone`: a dotted system variable
        if d == Dialect::Mysql && start == i + 2 && b.get(j) == Some(&b'.') && j > start {
            let mut e = j + 1;
            while e < b.len() && is_word_char(src[e..].chars().next().unwrap_or(' '), d) {
                e += src[e..].chars().next().map_or(1, char::len_utf8);
            }
            if e > j + 1 {
                j = e;
            }
        }
        if j > start {
            return (T::Var, j);
        }
    }
    let ch = src[i..].chars().next().unwrap_or(' ');
    if is_word_start(ch) || (ch.is_ascii_digit()) {
        return word(src, i, ch, d, open);
    }
    if d == Dialect::Tsql && matches!(ch, '#' | '@' | '$') {
        return word(src, i, ch, d, open);
    }
    for (ops, n) in [(OPS5, 5), (OPS4, 4), (OPS3, 3), (OPS2, 2)] {
        if let Some(s) = src.get(i..i + n)
            && ops.contains(&s)
            && op_allowed(s, d)
        {
            return (T::Op, i + n);
        }
    }
    if d == Dialect::Tsql && matches!(ch, '<' | '>' | '!') {
        // T-SQL lets blanks sit inside `> =`, `< >` and `! =`: one operator.
        let b = src.as_bytes();
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j > i + 1 && matches!(b.get(j), Some(b'=' | b'<' | b'>')) {
            return (T::Op, j + 1);
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
        "&&" | "||" | "!!" | "->" | "->>" | "#>" | "#>>" | "@>" | "<@" | "?|" | "?&" | "<->"
        | "<#>" | "<=>" | "@@" | "~=" | "#-" | "&&&" | "|>>" | "<<|" | "&<|" | "|&>" | "!~~"
        | "||/" | "~~*" | "!~*" | "!~~*" | "@?" | "-|-" => d != Dialect::Tsql || op == "||",
        ".." => false,
        _ => true,
    }
}

fn line_end_cr(b: &[u8], from: usize) -> usize {
    let e = line_end(b, from);
    if e > from && b[e - 1] == b'\r' {
        e - 1
    } else {
        e
    }
}

fn block_comment(b: &[u8], i: usize, d: Dialect, open: &mut bool) -> usize {
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
    *open = true;
    b.len()
}

/// `$tag$ ... $tag$` (the tag may be empty); `None` when `i` does not open one.
fn dollar_string(src: &str, i: usize, d: Dialect, open: &mut bool) -> Option<usize> {
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
    Some(src[j + 1..].find(tag).map_or_else(
        || {
            *open = true;
            b.len()
        },
        |p| j + 1 + p + tag.len(),
    ))
}

fn number(src: &str, i: usize, d: Dialect, open: &mut bool) -> (T, usize) {
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
    if b[i] != b'.' && src[j..].chars().next().is_some_and(is_word_start) {
        return word(src, i, ' ', d, open);
    }
    (T::Number, j)
}

/// `U&'..' UESCAPE '!'`: the escape clause belongs to the literal.
fn uescape(src: &str, end: usize) -> usize {
    let rest = &src[end..];
    let body = rest.trim_start();
    let Some(after) = body
        .get(..7)
        .filter(|w| w.eq_ignore_ascii_case("uescape"))
        .map(|_| &body[7..])
    else {
        return end;
    };
    let lit = after.trim_start();
    if !lit.starts_with('\'') || lit.len() < 3 || lit.as_bytes()[2] != b'\'' {
        return end;
    }
    src.len() - lit.len() + 3
}

fn word(src: &str, i: usize, first: char, d: Dialect, open: &mut bool) -> (T, usize) {
    let b = src.as_bytes();
    // Prefixed strings: `N'..'`, `E'..'`, `X'..'`, `B'..'`, `U&'..'`.
    if b.get(i + 1) == Some(&b'\'')
        && matches!(b[i], b'n' | b'N' | b'e' | b'E' | b'x' | b'X' | b'b' | b'B')
    {
        let escape = matches!(b[i], b'e' | b'E') || d == Dialect::Mysql;
        return (T::Str, quoted(src, i + 1, escape, open));
    }
    if d == Dialect::Postgres
        && matches!(b[i], b'u' | b'U')
        && b.get(i + 1) == Some(&b'&')
        && matches!(b.get(i + 2), Some(b'\'' | b'"'))
    {
        let q = b[i + 2];
        let end = quoted(src, i + 2, false, open);
        return (
            if q == b'"' { T::Quoted } else { T::Str },
            uescape(src, end),
        );
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
