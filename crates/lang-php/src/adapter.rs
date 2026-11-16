//! mago-syntax's CST and lexer as our lossless tree.
//!
//! The lexer gives every token of the file (inline HTML, tags, the parts of interpolated strings
//! and heredocs included); whitespace and comments are the text between the tokens that `replay`
//! splits, so the adapter drops those tokens. The CST gives the node spans: one row of
//! `grammars/php.grammar` per struct of mago's CST, walked once in pre-order. The CST's enum
//! wrappers (`Statement`, `Expression`, ...) and its one-token nodes have no row (`UNMAPPED`).
//! A panic in the parser, which is pinned but young, is a parse error and the file stays as
//! written.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, grammars};
use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_syntax::cst::Program;
use mago_syntax::cst::node::Node as Cst;
use mago_syntax::lexer::Lexer;
use mago_syntax::parser::parse_file_content;
use mago_syntax::settings::LexerSettings;
use mago_syntax::token::TokenKind as T;
use mago_syntax_core::input::Input;
use provider_api::adapter::{KindNames, Kinds, Node, Tok, replay, whole_file};
use provider_api::{ParseError, ParseResult, SourceFile};
use std::panic::catch_unwind;

const KINDS: Kinds = Kinds {
    root: k::PROGRAM,
    eof: k::EOF,
    unknown: k::UNKNOWN,
    whitespace: k::WHITESPACE,
    newline: k::NEWLINE,
};

/// CST kinds with no node of ours: enum wrappers around the struct that follows them, and
/// single-token nodes (their token is in the parent).
const UNMAPPED: &[&str] = &[
    "Program",
    "Access",
    "Argument",
    "PartialArgument",
    "ArrayElement",
    "Call",
    "PartialApplication",
    "EnumCaseItem",
    "ClassLikeConstantSelector",
    "ClassLikeMember",
    "ClassLikeMemberSelector",
    "MethodBody",
    "Property",
    "PropertyHookBody",
    "PropertyHookConcreteBody",
    "PropertyItem",
    "TraitUseAdaptation",
    "TraitUseMethodReference",
    "TraitUseSpecification",
    "Construct",
    "IfBody",
    "MatchArm",
    "SwitchBody",
    "SwitchCase",
    "SwitchCaseSeparator",
    "DeclareBody",
    "Expression",
    "BinaryOperator",
    "UnaryPrefixOperator",
    "UnaryPostfixOperator",
    "FullyQualifiedIdentifier",
    "Identifier",
    "LocalIdentifier",
    "QualifiedIdentifier",
    "Keyword",
    "Literal",
    "MagicConstant",
    "Modifier",
    "NamespaceBody",
    "AssignmentOperator",
    "ForeachBody",
    "ForeachTarget",
    "ForBody",
    "WhileBody",
    "StaticItem",
    "UseItems",
    "UseType",
    "Yield",
    "Statement",
    "CompositeString",
    "StringPart",
    "OpeningTag",
    "Terminator",
    "Hint",
    "Variable",
    "Error",
    "MissingTerminator",
    "ClassLikeMemberMissingSelector",
    "ClassLikeConstantMissingSelector",
];

pub fn parse_file(file: &SourceFile) -> ParseResult {
    parse(&file.text)
}

pub fn parse(src: &str) -> ParseResult {
    match catch_unwind(|| build(src)) {
        Ok(result) => result,
        Err(_) => whole_file(src, KINDS, "the PHP parser failed on this file"),
    }
}

fn build(src: &str) -> ParseResult {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), src.as_bytes());
    let mut walk = Walk {
        names: KindNames::new(grammars::PHP.get(), 256, |n| UNMAPPED.contains(&n)),
        nodes: Vec::with_capacity(src.len() / 8),
    };
    for statement in program.statements.iter() {
        walk.visit(Cst::Statement(statement));
    }
    // Children come in source order but for a few nodes: a stable sort keeps a parent before a
    // child of the same start.
    walk.nodes
        .sort_by_key(|n| (n.start, std::cmp::Reverse(n.end)));
    let mut result = replay(src, KINDS, &tokens(src), &walk.nodes, comments, |t, _| t);
    let mut errors = syntax_errors(program);
    errors.append(&mut result.errors);
    errors.sort_by_key(|e| e.start);
    result.errors = errors;
    result
}

fn syntax_errors(program: &Program<'_>) -> Vec<ParseError> {
    program
        .errors
        .iter()
        .map(|e| {
            let span = e.span();
            ParseError::syntax(span.start.offset, span.length(), e.to_string())
        })
        .collect()
}

struct Walk {
    names: KindNames,
    nodes: Vec<Node>,
}

impl Walk {
    fn visit(&mut self, node: Cst<'_, '_>) {
        let span = node.span();
        let kind = node.kind();
        let found = self.names.get(kind as usize, false, || kind.to_string());
        if let Some(kind) = found {
            self.nodes.push(Node {
                start: span.start.offset,
                end: span.end.offset,
                kind,
            });
        }
        node.visit_children(|child| self.visit(child));
    }
}

/// The lexer's tokens but whitespace and comments. The lexer stops at its first error, which the
/// parser reports too; the text after it becomes an `Unknown` token in `replay`.
fn tokens(src: &str) -> Vec<Tok> {
    let mut lexer = Lexer::new(
        Input::new(FileId::zero(), src.as_bytes()),
        LexerSettings::default(),
    );
    let mut out = Vec::with_capacity(src.len() / 4);
    while let Some(Ok(token)) = lexer.advance() {
        let kind = match token.kind {
            T::Whitespace
            | T::SingleLineComment
            | T::HashComment
            | T::MultiLineComment
            | T::DocBlockComment => continue,
            kind => token_kind(kind, token.value.first().copied().unwrap_or(0)),
        };
        let start = token.start.offset;
        out.push(Tok {
            kind,
            start,
            end: start + token.value.len() as u32,
        });
    }
    out
}

/// Our token kind of a lexer token; `first` is the first byte of its text, which tells keywords
/// (`foreach`, `__CLASS__`), casts (`(int)`) and operators apart.
fn token_kind(kind: T, first: u8) -> SyntaxKind {
    match kind {
        T::InlineText | T::InlineShebang => k::INLINE_TEXT,
        T::OpenTag | T::EchoTag | T::ShortOpenTag => k::OPEN_TAG,
        T::CloseTag => k::CLOSE_TAG,
        T::Variable => k::VARIABLE,
        T::LiteralString | T::PartialLiteralString => k::STRING,
        T::StringPart => k::STRING_PART,
        T::LiteralInteger | T::LiteralFloat | T::OffsetNumber => k::NUMBER,
        T::Identifier
        | T::QualifiedIdentifier
        | T::FullyQualifiedIdentifier
        | T::OffsetString
        | T::StringVariableName => k::IDENTIFIER,
        T::True | T::False => k::BOOLEAN,
        T::Null => k::NULL,
        T::LeftBrace => k::L_BRACE,
        T::RightBrace => k::R_BRACE,
        T::LeftParenthesis => k::L_PAREN,
        T::RightParenthesis => k::R_PAREN,
        T::LeftBracket => k::L_BRACKET,
        T::RightBracket => k::R_BRACKET,
        T::Semicolon => k::SEMI,
        T::Comma => k::COMMA,
        T::Colon => k::COLON,
        T::ColonColon => k::COLON_COLON,
        T::MinusGreaterThan | T::QuestionMinusGreaterThan => k::ARROW,
        T::EqualGreaterThan => k::DOUBLE_ARROW,
        T::Question => k::QUESTION,
        T::DotDotDot => k::ELLIPSIS,
        T::At => k::AT,
        T::Dollar => k::DOLLAR,
        T::DollarLeftBrace => k::DOLLAR_L_BRACE,
        T::HashLeftBracket => k::ATTR_OPEN,
        T::DoubleQuote | T::Backtick => k::QUOTE,
        T::DocumentStart(_) => k::DOC_START,
        T::DocumentEnd => k::DOC_END,
        T::Equal => k::ASSIGN,
        _ if first == b'(' => k::CAST,
        _ if first.is_ascii_alphabetic() || first == b'_' => k::KEYWORD,
        _ => k::OPERATOR,
    }
}

/// A comment at the start of the text between two tokens. A line comment ends at the line break
/// or before `?>`, which is the next token and so the end of `rest`.
fn comments(rest: &str, _: usize) -> Option<(SyntaxKind, usize)> {
    if rest.starts_with("//") || (rest.starts_with('#') && !rest.starts_with("#[")) {
        Some((k::COMMENT, rest.find(['\n', '\r']).unwrap_or(rest.len())))
    } else {
        let body = rest.strip_prefix("/*")?;
        Some((k::COMMENT, body.find("*/").map_or(rest.len(), |i| i + 4)))
    }
}
