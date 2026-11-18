//! The lossless lexer: every byte of the input ends up in exactly one token. CSS Syntax Level 3
//! tokens, plus the extras of the dialects (see [`Dialect`]).

use crate::Dialect;
use crate::generated::kind as k;
use core_syntax::SyntaxKind;
use core_syntax::parse::{self, ParseError, Token};
use core_syntax::strings::StringSyntax;

pub fn lex(src: &str, dialect: Dialect) -> (Vec<Token>, Vec<ParseError>) {
    let mut lx = Lexer {
        base: parse::Lexer::new(src, src.len() / 4),
        dialect,
        string: core_syntax::grammars::CSS
            .get()
            .string_rows(k::STRING)
            .next()
            .expect("the CSS grammar has a string row"),
    };
    while lx.pos < src.len() {
        lx.next();
    }
    lx.base.finish(k::EOF)
}

struct Lexer<'a> {
    base: parse::Lexer<'a>,
    dialect: Dialect,
    string: &'static StringSyntax,
}

impl<'a> std::ops::Deref for Lexer<'a> {
    type Target = parse::Lexer<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for Lexer<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_name(b: u8) -> bool {
    is_name_start(b) || b.is_ascii_digit() || b == b'-'
}

impl Lexer<'_> {
    /// Whether a backslash at `i` starts an escape (it is not followed by a line break or the end).
    fn escape_at(&self, i: usize) -> bool {
        self.at(i) == b'\\' && !matches!(self.at(i + 1), 0 | b'\n' | b'\r' | 0x0c)
    }

    fn ident_at(&self, i: usize) -> bool {
        let c = self.at(i);
        is_name_start(c)
            || self.escape_at(i)
            || c == b'-'
                && (is_name_start(self.at(i + 1))
                    || self.at(i + 1) == b'-'
                    || self.escape_at(i + 1))
    }

    fn number_at(&self, i: usize) -> bool {
        let digit_or_dot = |j| {
            self.at(j).is_ascii_digit() || self.at(j) == b'.' && self.at(j + 1).is_ascii_digit()
        };
        digit_or_dot(i) || matches!(self.at(i), b'+' | b'-') && digit_or_dot(i + 1)
    }

    /// Does a number, dimension or identifier end right here? A sign after it is an operator
    /// (`n+3` is the same three tokens as `n + 3`), not the sign of a number.
    fn after_operand(&self) -> bool {
        self.tokens.last().is_some_and(|t| {
            t.end == self.pos
                && matches!(t.kind, k::IDENT | k::NUMBER | k::DIMENSION | k::PERCENTAGE)
        })
    }

    /// After a backslash: up to six hex digits and one blank, or any one character.
    fn escape(&mut self) {
        self.pos += self.string.escape_len(self.src, self.pos, false);
    }

    fn name(&mut self) {
        while self.pos < self.src.len() {
            if is_name(self.at(0)) {
                self.pos += 1;
            } else if self.escape_at(0) {
                self.escape();
            } else {
                break;
            }
        }
    }

    fn next(&mut self) {
        let start = self.pos;
        let c = self.at(0);
        let kind = match c {
            b' ' | b'\t' | 0x0c => {
                while matches!(self.at(0), b' ' | b'\t' | 0x0c) {
                    self.pos += 1;
                }
                k::WHITESPACE
            }
            b'\n' | b'\r' => {
                self.pos += if self.at(0) == b'\r' && self.at(1) == b'\n' {
                    2
                } else {
                    1
                };
                k::NEWLINE
            }
            b'/' if self.at(1) == b'*' => match self.src[start + 2..].find("*/") {
                Some(i) => {
                    self.pos = start + 2 + i + 2;
                    k::COMMENT
                }
                None => {
                    self.pos = self.src.len();
                    self.error(start, "unterminated-comment", "unterminated comment");
                    k::UNKNOWN
                }
            },
            b'/' if self.at(1) == b'/' && self.dialect != Dialect::Css => {
                while self.pos < self.src.len() && !matches!(self.at(0), b'\n' | b'\r') {
                    self.pos += 1;
                }
                k::LINE_COMMENT
            }
            b'"' | b'\'' => self.string(),
            b'#' if self.at(1) == b'{' && self.dialect == Dialect::Scss => {
                self.pos += 2;
                k::INTERP_OPEN
            }
            b'#' if is_name(self.at(1)) || self.escape_at(1) => {
                self.pos += 1;
                self.name();
                k::HASH
            }
            b'@' if self.at(1) == b'{' && self.dialect == Dialect::Less => {
                self.pos += 2;
                k::INTERP_OPEN
            }
            b'@' if self.ident_at(1) => {
                self.pos += 1;
                self.name();
                k::AT_KEYWORD
            }
            b'$' if self.dialect == Dialect::Scss && self.ident_at(1) => {
                self.pos += 1;
                self.name();
                k::VARIABLE
            }
            _ if self.number_at(0) && !(matches!(c, b'+' | b'-') && self.after_operand()) => {
                self.number()
            }
            b'u' | b'U'
                if self.at(1) == b'+' && (self.at(2).is_ascii_hexdigit() || self.at(2) == b'?') =>
            {
                self.pos += 2;
                while self.at(0).is_ascii_hexdigit() || matches!(self.at(0), b'?' | b'-') {
                    self.pos += 1;
                }
                k::UNICODE_RANGE
            }
            _ if self.ident_at(0) => {
                self.name();
                if self.at(0) == b'(' && self.src[start..self.pos].eq_ignore_ascii_case("url") {
                    self.url()
                } else {
                    k::IDENT
                }
            }
            b':' => self.one(k::COLON),
            b';' => self.one(k::SEMICOLON),
            b',' => self.one(k::COMMA),
            b'{' => self.one(k::L_BRACE),
            b'}' => self.one(k::R_BRACE),
            b'(' => self.one(k::L_PAREN),
            b')' => self.one(k::R_PAREN),
            b'[' => self.one(k::L_BRACKET),
            b']' => self.one(k::R_BRACKET),
            _ => {
                self.pos += self.src[start..].chars().next().map_or(1, char::len_utf8);
                k::DELIM
            }
        };
        self.emit(kind, start);
    }

    fn one(&mut self, kind: SyntaxKind) -> SyntaxKind {
        self.pos += 1;
        kind
    }

    /// A quoted string; one that is not closed on its line is `Unknown` up to the line's end.
    fn string(&mut self) -> SyntaxKind {
        let start = self.pos;
        let scss = self.dialect == Dialect::Scss;
        let (end, closed) = self.string.scan(self.src, start, scss);
        self.pos = end;
        if closed {
            return k::STRING;
        }
        self.error(start, "unterminated-string", "unterminated string");
        k::UNKNOWN
    }

    /// `url(` and an unquoted body: one token. Anything else (a quoted string, a function call,
    /// a body that starts with an interpolation or a `$variable`) is a function call of the
    /// ordinary tokens. In SCSS an interpolation inside the body belongs to the token.
    fn url(&mut self) -> SyntaxKind {
        let interp = self.dialect == Dialect::Scss && !matches!(self.at(1), b'$' | b'#');
        let mut i = 1;
        while self.pos + i < self.src.len() {
            match self.at(i) {
                b')' | b'"' | b'\'' | b'(' => break,
                b'{' if interp && self.at(i - 1) == b'#' => {
                    while self.pos + i < self.src.len() && self.at(i) != b'}' {
                        i += 1;
                    }
                }
                b'{' => break,
                b'\\' => i += 1,
                _ => {}
            }
            i += 1;
        }
        let end = (self.pos + i).min(self.src.len());
        let mut depth = 0;
        let spaced = self.src[self.pos + 1..end].trim().chars().any(|c| {
            depth += i32::from(c == '{') - i32::from(c == '}');
            depth == 0 && c.is_whitespace()
        });
        if self.src.as_bytes().get(end) == Some(&b')') && self.at(1) != b'$' && !spaced {
            self.pos = end + 1;
            return k::URL;
        }
        k::IDENT
    }

    fn number(&mut self) -> SyntaxKind {
        if matches!(self.at(0), b'+' | b'-') {
            self.pos += 1;
        }
        while self.at(0).is_ascii_digit() {
            self.pos += 1;
        }
        if self.at(0) == b'.' && self.at(1).is_ascii_digit() {
            self.pos += 1;
            while self.at(0).is_ascii_digit() {
                self.pos += 1;
            }
        }
        // `1.e1` is the number `1e1`.
        let dot = usize::from(
            self.at(0) == b'.'
                && matches!(self.at(1), b'e' | b'E')
                && (self.at(2).is_ascii_digit()
                    || matches!(self.at(2), b'+' | b'-') && self.at(3).is_ascii_digit()),
        );
        if matches!(self.at(dot), b'e' | b'E') {
            self.pos += dot;
            let sign = usize::from(matches!(self.at(1), b'+' | b'-'));
            if self.at(1 + sign).is_ascii_digit() {
                self.pos += 1 + sign;
                while self.at(0).is_ascii_digit() {
                    self.pos += 1;
                }
            }
        }
        if self.at(0) == b'%' {
            self.pos += 1;
            k::PERCENTAGE
        } else if self.ident_at(0) {
            self.name();
            k::DIMENSION
        } else {
            k::NUMBER
        }
    }
}
