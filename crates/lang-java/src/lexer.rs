//! The Java lexer: the source as tokens with byte spans. Whitespace and comments are not tokens
//! (the tree builder reads them from the gaps); every other byte is in a token, an unknown
//! character as `T::Unknown`, so the tokens and the gaps tile the source.
//!
//! `>` is always a token of its own (`>>` and `>>>` are joined by the parser in an expression, so
//! `List<List<X>>` needs no splitting); `>=`, `>>=` and `>>>=` are lexed whole.

use crate::generated::kind as k;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum T {
    Eof,
    Unknown,
    Ident,
    NumberLit,
    StringLit,
    CharLit,
    // The primitive types (contiguous).
    Boolean,
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    // Reserved words.
    Abstract,
    Assert,
    Break,
    Case,
    Catch,
    Class,
    Const,
    Continue,
    Default,
    Do,
    Else,
    Enum,
    Extends,
    Final,
    Finally,
    For,
    Goto,
    If,
    Implements,
    Import,
    Instanceof,
    Interface,
    Native,
    New,
    Package,
    Private,
    Protected,
    Public,
    Return,
    Static,
    Strictfp,
    Super,
    Switch,
    Synchronized,
    This,
    Throw,
    Throws,
    Transient,
    Try,
    Void,
    Volatile,
    While,
    True,
    False,
    Null,
    /// `@interface`.
    AtInterface,
    // Contextual keywords: identifiers everywhere else.
    Var,
    Record,
    Sealed,
    Permits,
    Yield,
    When,
    Non,
    Open,
    Module,
    Requires,
    Transitive,
    Exports,
    Opens,
    Uses,
    Provides,
    To,
    With,
    // Punctuation and operators.
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semi,
    Comma,
    Dot,
    Ellipsis,
    At,
    Colon,
    ColonColon,
    Arrow,
    Question,
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    Not,
    Tilde,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    Pipe,
    Caret,
    Shl,
    PlusPlus,
    MinusMinus,
    /// `+=`, `-=`, ..., `>>>=`.
    AssignOp,
}

impl T {
    pub fn is_primitive(self) -> bool {
        (T::Boolean as u8..=T::Short as u8).contains(&(self as u8))
    }

    /// A contextual keyword, which is a name wherever the grammar wants one.
    pub fn is_contextual(self) -> bool {
        (T::Var as u8..=T::With as u8).contains(&(self as u8))
    }

    /// An identifier or a contextual keyword.
    pub fn is_name(self) -> bool {
        self == T::Ident || self.is_contextual()
    }
}

#[derive(Clone, Copy)]
pub struct Token {
    pub kind: T,
    pub start: u32,
    pub end: u32,
}

pub struct Lexed {
    pub tokens: Vec<Token>,
    /// For a `(` token, the index of its `)`; `u32::MAX` when there is none and for other tokens.
    pub close: Vec<u32>,
    /// Unterminated strings, characters and comments: `(start, len, message)`.
    pub errors: Vec<(u32, u32, &'static str)>,
}

fn keyword(s: &str) -> T {
    match s {
        "abstract" => T::Abstract,
        "assert" => T::Assert,
        "boolean" => T::Boolean,
        "break" => T::Break,
        "byte" => T::Byte,
        "case" => T::Case,
        "catch" => T::Catch,
        "char" => T::Char,
        "class" => T::Class,
        "const" => T::Const,
        "continue" => T::Continue,
        "default" => T::Default,
        "do" => T::Do,
        "double" => T::Double,
        "else" => T::Else,
        "enum" => T::Enum,
        "extends" => T::Extends,
        "final" => T::Final,
        "finally" => T::Finally,
        "float" => T::Float,
        "for" => T::For,
        "goto" => T::Goto,
        "if" => T::If,
        "implements" => T::Implements,
        "import" => T::Import,
        "instanceof" => T::Instanceof,
        "int" => T::Int,
        "interface" => T::Interface,
        "long" => T::Long,
        "native" => T::Native,
        "new" => T::New,
        "package" => T::Package,
        "private" => T::Private,
        "protected" => T::Protected,
        "public" => T::Public,
        "return" => T::Return,
        "short" => T::Short,
        "static" => T::Static,
        "strictfp" => T::Strictfp,
        "super" => T::Super,
        "switch" => T::Switch,
        "synchronized" => T::Synchronized,
        "this" => T::This,
        "throw" => T::Throw,
        "throws" => T::Throws,
        "transient" => T::Transient,
        "try" => T::Try,
        "void" => T::Void,
        "volatile" => T::Volatile,
        "while" => T::While,
        "true" => T::True,
        "false" => T::False,
        "null" => T::Null,
        "var" => T::Var,
        "record" => T::Record,
        "sealed" => T::Sealed,
        "permits" => T::Permits,
        "yield" => T::Yield,
        "when" => T::When,
        "non" => T::Non,
        "open" => T::Open,
        "module" => T::Module,
        "requires" => T::Requires,
        "transitive" => T::Transitive,
        "exports" => T::Exports,
        "opens" => T::Opens,
        "uses" => T::Uses,
        "provides" => T::Provides,
        "to" => T::To,
        "with" => T::With,
        _ => T::Ident,
    }
}

fn is_id_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$' || (!c.is_ascii() && c.is_alphabetic())
}

fn is_id_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || (!c.is_ascii() && c.is_alphanumeric())
}

fn is_blank(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

pub fn lex(src: &str) -> Lexed {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = Lexed {
        tokens: Vec::with_capacity(n / 5 + 1),
        close: Vec::with_capacity(n / 5 + 1),
        errors: Vec::new(),
    };
    let mut open: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < n {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\n' | b'\r' | 0x0c => {
                i += 1;
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                i += b[i..]
                    .iter()
                    .position(|&c| c == b'\n' || c == b'\r')
                    .unwrap_or(n - i);
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                match src[i + 2..].find("*/") {
                    Some(e) => i += e + 4,
                    None => {
                        out.errors
                            .push((i as u32, (n - i) as u32, "unterminated comment"));
                        i = n;
                    }
                }
                continue;
            }
            _ if c >= 0x80 => {
                let ch = src[i..].chars().next().unwrap_or(' ');
                if is_blank(ch) {
                    i += ch.len_utf8();
                    continue;
                }
            }
            _ => {}
        }
        let start = i;
        let (kind, len) = token(src, i, &mut out.errors);
        i += len;
        let idx = out.tokens.len() as u32;
        match kind {
            T::LParen => open.push(idx),
            T::RParen => {
                if let Some(o) = open.pop() {
                    out.close[o as usize] = idx;
                }
            }
            _ => {}
        }
        out.close.push(u32::MAX);
        out.tokens.push(Token {
            kind,
            start: start as u32,
            end: i as u32,
        });
    }
    out
}

/// The token at `i` and its length.
fn token(src: &str, i: usize, errors: &mut Vec<(u32, u32, &'static str)>) -> (T, usize) {
    let b = src.as_bytes();
    let at = |k: usize| b.get(i + k).copied().unwrap_or(0);
    let c = b[i];
    match c {
        b'{' => (T::LBrace, 1),
        b'}' => (T::RBrace, 1),
        b'(' => (T::LParen, 1),
        b')' => (T::RParen, 1),
        b'[' => (T::LBracket, 1),
        b']' => (T::RBracket, 1),
        b';' => (T::Semi, 1),
        b',' => (T::Comma, 1),
        b'.' => {
            if at(1).is_ascii_digit() {
                number(b, i)
            } else if at(1) == b'.' && at(2) == b'.' {
                (T::Ellipsis, 3)
            } else {
                (T::Dot, 1)
            }
        }
        b'@' => {
            if src[i + 1..].starts_with("interface")
                && !src[i + 10..].chars().next().is_some_and(is_id_part)
            {
                (T::AtInterface, 10)
            } else {
                (T::At, 1)
            }
        }
        b':' => {
            if at(1) == b':' {
                (T::ColonColon, 2)
            } else {
                (T::Colon, 1)
            }
        }
        b'?' => (T::Question, 1),
        b'~' => (T::Tilde, 1),
        b'=' => {
            if at(1) == b'=' {
                (T::EqEq, 2)
            } else {
                (T::Eq, 1)
            }
        }
        b'!' => {
            if at(1) == b'=' {
                (T::Ne, 2)
            } else {
                (T::Not, 1)
            }
        }
        b'<' => match (at(1), at(2)) {
            (b'=', _) => (T::Le, 2),
            (b'<', b'=') => (T::AssignOp, 3),
            (b'<', _) => (T::Shl, 2),
            _ => (T::Lt, 1),
        },
        b'>' => match (at(1), at(2), at(3)) {
            (b'=', _, _) => (T::Ge, 2),
            (b'>', b'=', _) => (T::AssignOp, 3),
            (b'>', b'>', b'=') => (T::AssignOp, 4),
            _ => (T::Gt, 1),
        },
        b'&' => match at(1) {
            b'&' => (T::AndAnd, 2),
            b'=' => (T::AssignOp, 2),
            _ => (T::Amp, 1),
        },
        b'|' => match at(1) {
            b'|' => (T::OrOr, 2),
            b'=' => (T::AssignOp, 2),
            _ => (T::Pipe, 1),
        },
        b'+' => match at(1) {
            b'+' => (T::PlusPlus, 2),
            b'=' => (T::AssignOp, 2),
            _ => (T::Plus, 1),
        },
        b'-' => match at(1) {
            b'-' => (T::MinusMinus, 2),
            b'=' => (T::AssignOp, 2),
            b'>' => (T::Arrow, 2),
            _ => (T::Minus, 1),
        },
        b'*' => {
            if at(1) == b'=' {
                (T::AssignOp, 2)
            } else {
                (T::Star, 1)
            }
        }
        b'/' => {
            if at(1) == b'=' {
                (T::AssignOp, 2)
            } else {
                (T::Slash, 1)
            }
        }
        b'%' => {
            if at(1) == b'=' {
                (T::AssignOp, 2)
            } else {
                (T::Percent, 1)
            }
        }
        b'^' => {
            if at(1) == b'=' {
                (T::AssignOp, 2)
            } else {
                (T::Caret, 1)
            }
        }
        b'0'..=b'9' => number(b, i),
        b'"' => quoted(src, i, k::STRING, errors),
        b'\'' => quoted(src, i, k::CHAR, errors),
        _ => {
            let ch = src[i..].chars().next().unwrap_or(' ');
            if is_id_start(ch) {
                let len = src[i..]
                    .find(|c: char| !is_id_part(c))
                    .unwrap_or(src.len() - i);
                (keyword(&src[i..i + len]), len)
            } else {
                errors.push((i as u32, ch.len_utf8() as u32, "unexpected character"));
                (T::Unknown, ch.len_utf8())
            }
        }
    }
}

/// An integer or floating-point literal of any base, with its suffix.
fn number(b: &[u8], i: usize) -> (T, usize) {
    let at = |k: usize| b.get(k).copied().unwrap_or(0);
    let digits = |mut k: usize, ok: fn(u8) -> bool| {
        while ok(at(k)) || at(k) == b'_' {
            k += 1;
        }
        k
    };
    let dec: fn(u8) -> bool = |c| c.is_ascii_digit();
    let hex: fn(u8) -> bool = |c| c.is_ascii_hexdigit();
    let mut k = i;
    if at(k) == b'0' && matches!(at(k + 1), b'x' | b'X') {
        k = digits(k + 2, hex);
        if at(k) == b'.' && (at(k + 1).is_ascii_hexdigit() || matches!(at(k + 1), b'p' | b'P')) {
            k = digits(k + 1, hex);
        }
        if matches!(at(k), b'p' | b'P') {
            k += 1;
            if matches!(at(k), b'+' | b'-') {
                k += 1;
            }
            k = digits(k, dec);
        }
    } else if at(k) == b'0' && matches!(at(k + 1), b'b' | b'B' | b'o' | b'O') {
        k = digits(k + 2, dec);
    } else {
        k = digits(k, dec);
        if at(k) == b'.' && at(k + 1) != b'.' {
            let next = at(k + 1);
            if next.is_ascii_digit()
                || matches!(next, b'e' | b'E' | b'f' | b'F' | b'd' | b'D')
                || !(next.is_ascii_alphabetic() || next == b'_' || next == b'$' || next >= 0x80)
            {
                k = digits(k + 1, dec);
            }
        }
        if matches!(at(k), b'e' | b'E')
            && (at(k + 1).is_ascii_digit()
                || (matches!(at(k + 1), b'+' | b'-') && at(k + 2).is_ascii_digit()))
        {
            k = digits(k + 2, dec);
        }
    }
    if matches!(at(k), b'l' | b'L' | b'f' | b'F' | b'd' | b'D') {
        k += 1;
    }
    (T::NumberLit, k - i)
}

/// A string literal, a text block or a character literal, scanned by the kind's `string` row; one
/// that is not closed ends at the line break (a text block: at the end of the file).
fn quoted(
    src: &str,
    i: usize,
    kind: core_syntax::SyntaxKind,
    errors: &mut Vec<(u32, u32, &'static str)>,
) -> (T, usize) {
    let row = core_syntax::grammars::JAVA
        .get()
        .string_rows(kind)
        .next()
        .expect("the Java grammar has a string row");
    let (end, closed) = row.scan(src, i, false);
    if !closed {
        let message = if kind == k::CHAR {
            "unterminated character"
        } else if src[i..].starts_with("\"\"\"") {
            "unterminated text block"
        } else {
            "unterminated string"
        };
        errors.push((i as u32, (end - i) as u32, message));
    }
    (
        if kind == k::CHAR {
            T::CharLit
        } else {
            T::StringLit
        },
        end - i,
    )
}
