//! A lossless recursive-descent parser for JSON and JSONC with error recovery.
//!
//! Every token ends up in the tree. Trivia are placed by the shared attachment rule (see
//! `core_syntax::trivia`): trailing trivia right after their token, leading trivia right before
//! theirs, inside the node that token belongs to. Unexpected tokens are wrapped in `ERROR` nodes.

use crate::generated::kind as k;
use crate::lexer::lex;
use core_syntax::SyntaxKind;
use core_syntax::parse;

pub use core_syntax::parse::{MAX_DEPTH, Parsed};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Json,
    /// JSON with comments and trailing commas.
    Jsonc,
}

impl Dialect {
    pub fn from_name(name: &str) -> Option<Dialect> {
        match name {
            "json" => Some(Dialect::Json),
            "jsonc" => Some(Dialect::Jsonc),
            _ => None,
        }
    }
}

pub fn parse(src: &str, dialect: Dialect) -> Parsed {
    let mut p = Parser {
        p: parse::Parser::new(src, lex(src, dialect == Dialect::Jsonc), is_trivia, k::EOF),
        dialect,
    };
    p.document();
    p.p.done()
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHITESPACE | k::NEWLINE | k::LINE_COMMENT | k::BLOCK_COMMENT
    )
}

struct Parser<'a> {
    p: parse::Parser<'a>,
    dialect: Dialect,
}

impl<'a> std::ops::Deref for Parser<'a> {
    type Target = parse::Parser<'a>;
    fn deref(&self) -> &Self::Target {
        &self.p
    }
}

impl std::ops::DerefMut for Parser<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.p
    }
}

impl Parser<'_> {
    fn error_node(&mut self) {
        self.start(SyntaxKind::ERROR);
        self.bump();
        self.finish();
    }

    fn starts_value(&self) -> bool {
        matches!(
            self.cur(),
            k::L_BRACE | k::L_BRACKET | k::STRING | k::NUMBER | k::TRUE | k::FALSE | k::NULL
        )
    }

    fn document(&mut self) {
        self.start(k::DOCUMENT);
        if self.starts_value() {
            self.value();
        } else if self.at_eof() {
            self.error("expected-value", "expected a value");
        }
        if !self.at_eof() {
            let message = if self.starts_value() {
                "unexpected content after the top-level value"
            } else {
                "unexpected token"
            };
            self.error("unexpected-token", message);
            // Everything after the first stray token is one error site.
            self.error_rest();
        }
        self.bump(); // Eof
        self.finish();
    }

    /// At a token that starts a value.
    fn value(&mut self) {
        match self.cur() {
            k::L_BRACE | k::L_BRACKET => {
                if !self.enter(MAX_DEPTH) {
                    self.too_deep();
                } else {
                    if self.cur() == k::L_BRACE {
                        self.object();
                    } else {
                        self.array();
                    }
                    self.leave();
                }
            }
            _ => {
                self.start(k::LITERAL);
                self.bump();
                self.finish();
            }
        }
    }

    fn too_deep(&mut self) {
        self.error(
            "nesting-too-deep",
            format!("nesting is deeper than {MAX_DEPTH} levels"),
        );
        self.error_rest();
    }

    fn object(&mut self) {
        self.start(k::OBJECT);
        self.bump(); // {
        loop {
            match self.cur() {
                k::R_BRACE => break,
                k::EOF => {
                    self.error("unclosed-object", "expected `}` before the end of the file");
                    break;
                }
                k::STRING => {
                    self.member();
                    match self.cur() {
                        k::COMMA => self.comma(k::R_BRACE),
                        k::R_BRACE | k::EOF => {}
                        _ => self.error("expected-comma", "expected `,` or `}`"),
                    }
                }
                k::COMMA => {
                    self.error("unexpected-comma", "unexpected `,`");
                    self.error_node();
                }
                _ => {
                    self.error("expected-key", "expected a string key or `}`");
                    self.recover(k::R_BRACE);
                }
            }
        }
        if self.cur() == k::R_BRACE {
            self.bump();
        }
        self.finish();
    }

    /// Consumes the `,` after an item.
    fn comma(&mut self, closer: SyntaxKind) {
        self.bump();
        if self.cur() == closer {
            self.trailing_comma_allowed();
        }
    }

    /// Skips the rest of a bad item without further reports: everything up to the next `,` or
    /// the closing delimiter at this level (brackets in between are balanced).
    fn recover(&mut self, closer: SyntaxKind) {
        self.start(SyntaxKind::ERROR);
        let mut nesting = 0usize;
        while !self.at_eof() {
            match self.cur() {
                k::L_BRACE | k::L_BRACKET => nesting += 1,
                k::R_BRACE | k::R_BRACKET if nesting > 0 => nesting -= 1,
                k::COMMA if nesting == 0 => break,
                kind if kind == closer && nesting == 0 => break,
                _ => {}
            }
            self.bump();
        }
        self.finish();
        if self.cur() == k::COMMA {
            self.comma(closer);
        }
    }

    fn member(&mut self) {
        self.start(k::MEMBER);
        self.bump(); // key
        if self.cur() == k::COLON {
            self.bump();
        } else {
            self.error("expected-colon", "expected `:` after the key");
        }
        if self.starts_value() {
            self.value();
        } else {
            self.error("expected-value", "expected a value");
        }
        self.finish();
    }

    fn array(&mut self) {
        self.start(k::ARRAY);
        self.bump(); // [
        loop {
            match self.cur() {
                k::R_BRACKET => break,
                k::EOF => {
                    self.error("unclosed-array", "expected `]` before the end of the file");
                    break;
                }
                k::COMMA => {
                    self.error("unexpected-comma", "unexpected `,`");
                    self.error_node();
                }
                _ if self.starts_value() => {
                    self.value();
                    match self.cur() {
                        k::COMMA => self.comma(k::R_BRACKET),
                        k::R_BRACKET | k::EOF => {}
                        _ => self.error("expected-comma", "expected `,` or `]`"),
                    }
                }
                _ => {
                    self.error("expected-value", "expected a value or `]`");
                    self.recover(k::R_BRACKET);
                }
            }
        }
        if self.cur() == k::R_BRACKET {
            self.bump();
        }
        self.finish();
    }

    /// Called after a comma that is followed by the closing delimiter. The comma is the previous
    /// real token.
    fn trailing_comma_allowed(&mut self) {
        if self.dialect == Dialect::Json {
            let comma = self.pos - 1;
            self.error_at(
                comma,
                "trailing-comma",
                "trailing commas are not allowed in JSON (use the jsonc dialect)",
            );
        }
    }
}
