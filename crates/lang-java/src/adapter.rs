//! rezel's concrete tree (rezel-lang-java, a Lezer grammar port) as our lossless tree.
//!
//! The tree has a node for every grammar rule and a leaf for most tokens; whitespace is the text
//! between leaves. The adapter walks it once in pre-order: a node with children is a node of our
//! tree (one row of `grammars/java.grammar` per rezel name; names that have none, like the wrapper
//! `Core` nodes of other entry points, are dropped), a leaf is a token, comments are left to
//! `scan`, and a text block is one `String` token. Lezer leaves out the tokens its grammar never
//! names (`<`, `>`, `|`, `&`, `@interface`): they are read from the text between two leaves.
//! `⚠` nodes are the parser's recovery points and become syntax errors.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, grammars};
use provider_api::adapter::{KindNames, Kinds, Node, Tok, replay, whole_file};
use provider_api::{ParseError, ParseResult, SourceFile};
use rezel_common::SyntaxNode;
use std::panic::catch_unwind;

const KINDS: Kinds = Kinds {
    root: k::PROGRAM,
    eof: k::EOF,
    unknown: k::UNKNOWN,
    whitespace: k::WHITESPACE,
    newline: k::NEWLINE,
};

pub fn parse_file(file: &SourceFile) -> ParseResult {
    parse(&file.text)
}

pub fn parse(src: &str) -> ParseResult {
    // rezel asserts on some inputs: a panic is a parse error and the file stays as written.
    let tree = match catch_unwind(|| rezel_lang_java::parser().parse(src)) {
        Ok(Ok(tree)) => tree,
        Ok(Err(e)) => return whole_file(src, KINDS, e.to_string()),
        Err(_) => return whole_file(src, KINDS, "the Java parser failed on this file"),
    };
    let mut walk = Walk {
        src,
        names: KindNames::new(grammars::JAVA.get(), 512, |n| n != "Program"),
        tokens: Vec::with_capacity(src.len() / 4),
        nodes: Vec::with_capacity(src.len() / 8),
        errors: Vec::new(),
        pos: 0,
    };
    for child in tree.top_node().children() {
        walk.visit(child);
    }
    walk.fill(src.len());
    let mut result = replay(src, KINDS, &walk.tokens, &walk.nodes, comments, |t, _| t);
    walk.errors.append(&mut result.errors);
    walk.errors.sort_by_key(|e| e.start);
    result.errors = walk.errors;
    result
}

struct Walk<'a> {
    src: &'a str,
    names: KindNames,
    tokens: Vec<Tok>,
    nodes: Vec<Node>,
    errors: Vec<ParseError>,
    /// The end of the last leaf (comments included).
    pos: usize,
}

impl Walk<'_> {
    fn visit(&mut self, node: SyntaxNode) {
        let (start, end) = (u32::from(node.from()), u32::from(node.to()));
        let name = node.name();
        let mut children = node.children().peekable();
        if &*name == "⚠" {
            self.errors
                .push(ParseError::syntax(start, end - start, "syntax error"));
        } else if &*name == "TextBlock" {
            self.leaf(start, end, k::STRING);
            return;
        } else if children.peek().is_some() {
            let id = node.node_type().id() as usize;
            if let Some(kind) = self.names.get(id, false, || node_name(&name)) {
                self.nodes.push(Node { start, end, kind });
            }
        } else if end > start {
            let text = &self.src[start as usize..end as usize];
            match name.as_ref() {
                "BlockComment" | "LineComment" => self.pos = end as usize,
                _ => self.leaf(start, end, leaf_kind(&name, text)),
            }
            return;
        }
        for child in children {
            self.visit(child);
        }
    }

    fn leaf(&mut self, start: u32, end: u32, kind: SyntaxKind) {
        self.fill(start as usize);
        self.tokens.push(Tok { kind, start, end });
        self.pos = end as usize;
    }

    /// The tokens Lezer leaves out, in the text from the last leaf up to `to`.
    fn fill(&mut self, to: usize) {
        let gap = &self.src[self.pos.min(to)..to];
        if gap.bytes().all(|b| b.is_ascii_whitespace()) {
            return;
        }
        let mut at = self.pos;
        let mut rest = gap;
        while !rest.is_empty() {
            let blank = rest.len() - rest.trim_start().len();
            at += blank;
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            let (kind, len) = if rest.starts_with("@interface") || rest.starts_with("non-sealed") {
                (k::KEYWORD, 10)
            } else {
                match rest.as_bytes()[0] {
                    b'<' => (k::LT, 1),
                    b'>' => (k::GT, 1),
                    b'|' | b'&' => (k::OPERATOR, 1),
                    _ => break,
                }
            };
            self.tokens.push(Tok {
                kind,
                start: at as u32,
                end: (at + len) as u32,
            });
            at += len;
            rest = &rest[len..];
        }
    }
}

/// The grammar's name of a rezel node (`Pattern` clashes with the AST support type).
fn node_name(name: &str) -> String {
    if name == "Pattern" {
        "PatternNode"
    } else {
        name
    }
    .into()
}

/// Our token kind of a rezel leaf: the name of a punctuation leaf is its text.
fn leaf_kind(name: &str, text: &str) -> SyntaxKind {
    match name {
        "(" => k::L_PAREN,
        ")" => k::R_PAREN,
        "{" => k::L_BRACE,
        "}" => k::R_BRACE,
        "[" => k::L_BRACKET,
        "]" => k::R_BRACKET,
        ";" => k::SEMI,
        "," => k::COMMA,
        "." => k::DOT,
        ":" => k::COLON,
        "::" => k::COLON_COLON,
        "->" => k::ARROW,
        "@" => k::AT,
        "..." => k::ELLIPSIS,
        "?" => k::QUESTION,
        "Identifier" | "Definition" | "TypeName" => k::IDENTIFIER,
        "StringLiteral" => k::STRING,
        "CharacterLiteral" => k::CHAR,
        "IntegerLiteral" | "FloatingPointLiteral" => k::NUMBER,
        "BooleanLiteral" => k::BOOLEAN,
        "null" => k::NULL,
        "PrimitiveType" => k::PRIMITIVE_TYPE,
        "Asterisk" | "ArithOp" | "BitOp" | "CompareOp" | "LogicOp" | "UpdateOp" | "AssignOp"
        | "TypeArguments" => match text {
            "=" => k::ASSIGN,
            "<" => k::LT,
            ">" => k::GT,
            _ => k::OPERATOR,
        },
        n if n.starts_with(|c: char| c.is_ascii_lowercase()) => k::KEYWORD,
        _ => k::UNKNOWN,
    }
}

/// A comment at the start of the text between two tokens.
fn comments(rest: &str, _: usize) -> Option<(SyntaxKind, usize)> {
    if rest.starts_with("//") {
        Some((k::COMMENT, rest.find(['\n', '\r']).unwrap_or(rest.len())))
    } else {
        let body = rest.strip_prefix("/*")?;
        Some((k::COMMENT, body.find("*/").map_or(rest.len(), |i| i + 4)))
    }
}
