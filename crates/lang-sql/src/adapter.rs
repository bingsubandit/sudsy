//! sqruff's segment tree as our lossless tree.
//!
//! sqruff lexes and parses a whole file into segments: raw leaves (whitespace, comments, words,
//! symbols) and nodes named by sqlfluff's segment types. The adapter flattens the leaves in
//! order, keeps the zero-width `Indent`, `ImplicitIndent` and `Dedent` (where the dialect opens and
//! closes an indented block; the layout reads the indentation structure from them) and drops
//! `EndOfFile`, keeps the nodes the grammar names as spans of the other non-trivia tokens, and
//! rebuilds the tree with the shared trivia attachment rule, because sqruff puts whitespace and
//! comments wherever its grammar matched them.
//!
//! sqruff panics on some inputs (an `unwrap` in a dialect), so the parse runs under
//! `catch_unwind`: a panic is a parse error and the file is one `Unknown` token, left unchanged.

use crate::generated::kind as k;
use core_syntax::{Grammar, SyntaxKind, grammars};
use provider_api::adapter::{Kinds, Node, Tok, replay, whole_file};
use provider_api::{ParseError, ParseResult, SourceFile};
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind as Sq;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};
use sqruff_lib_core::parser::{IndentationConfig, Parser};
use sqruff_lib_dialects::kind_to_dialect;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use strum::IntoEnumIterator;

/// The most bytes of one file; sqruff parses 0.3 to 1 MB/s and holds about 650 bytes per input byte at its peak: this bounds one parse to 2 s and 350 MB.
pub const MAX_BYTES: usize = 512 << 10;

const KINDS: Kinds = Kinds {
    root: k::FILE,
    eof: k::EOF,
    unknown: k::UNKNOWN,
    whitespace: k::WHITESPACE,
    newline: k::NEWLINE,
};

/// The dialects compiled in, as `dialect` option values.
pub const DIALECTS: [&str; 4] = ["ansi", "postgres", "mysql", "tsql"];

fn dialect_kind(name: &str) -> DialectKind {
    match name {
        "postgres" => DialectKind::Postgres,
        "mysql" => DialectKind::Mysql,
        "tsql" => DialectKind::Tsql,
        _ => DialectKind::Ansi,
    }
}

thread_local! {
    // sqruff's segments are `Rc`: a dialect is built once per thread, on its first file.
    static BUILT: RefCell<Vec<(DialectKind, Dialect)>> = const { RefCell::new(Vec::new()) };
}

/// A file of the provider: the dialect is the `dialect` option.
pub fn parse_file(file: &SourceFile) -> ParseResult {
    parse(
        &file.text,
        file.language_version.as_deref().unwrap_or("ansi"),
    )
}

pub fn parse(src: &str, dialect: &str) -> ParseResult {
    let leaves = if src.len() > MAX_BYTES {
        Err(format!("file over {} KB", MAX_BYTES >> 10))
    } else {
        catch_unwind(AssertUnwindSafe(|| {
            sqruff_parse(src, dialect_kind(dialect))
        }))
        .unwrap_or_else(|_| Err("the SQL parser failed on this file".into()))
    };
    match leaves {
        Ok(flat) => build(src, flat),
        Err(message) => whole_file(src, KINDS, message),
    }
}

struct Leaf {
    kind: SyntaxKind,
    start: u32,
    end: u32,
}

/// A grammar node over the leaves `first..end`.
struct Span {
    kind: SyntaxKind,
    first: usize,
    end: usize,
}

/// What the walk over sqruff's tree collects.
struct Flat {
    leaves: Vec<Leaf>,
    nodes: Vec<Span>,
    errors: Vec<ParseError>,
}

fn sqruff_parse(src: &str, kind: DialectKind) -> Result<Flat, String> {
    let root = BUILT.with_borrow_mut(|built| {
        let at = match built.iter().position(|(k, _)| *k == kind) {
            Some(at) => at,
            None => {
                let dialect = kind_to_dialect(&kind, None).ok_or("dialect not compiled in")?;
                built.push((kind, dialect));
                built.len() - 1
            }
        };
        let dialect = &built[at].1;
        let tables = Tables::default();
        let (segments, lex_errors) = dialect.lexer().lex(&tables, src);
        // sqlfluff's defaults: joins and CTEs stay flush, the contents of `ON`, `USING` and `THEN`
        // are indented.
        let indents = IndentationConfig::from_bool_lookup(|key| {
            matches!(
                key,
                "indented_using_on"
                    | "indented_on_contents"
                    | "indented_then"
                    | "indented_then_contents"
            )
        });
        let parser = Parser::new_with_limits(
            dialect,
            indents,
            sqruff_lib_core::parser::DEFAULT_MAX_PARSE_DEPTH,
            0,
        );
        let root = parser
            .parse(&tables, &segments)
            .map_err(|e| e.description)?;
        Ok::<_, String>((root, lex_errors.len()))
    })?;
    let (Some(root), _) = root else {
        return Ok(Flat {
            leaves: Vec::new(),
            nodes: Vec::new(),
            errors: Vec::new(),
        });
    };
    let mut walk = Walk {
        src,
        kinds: kind_table(grammars::SQL.get()),
        flat: Flat {
            leaves: Vec::new(),
            nodes: Vec::new(),
            errors: Vec::new(),
        },
        pos: 0,
        parent: (Sq::File, 0),
    };
    walk.visit(&root);
    if walk.pos != src.len() {
        return Err("the SQL parser dropped text".into());
    }
    Ok(walk.flat)
}

/// Our node kind by sqruff's segment type: the type's name in Pascal case, or `Statement` for
/// any other `*_statement`; `None` is a type with no node (transparent).
fn kind_table(grammar: &'static Grammar) -> &'static [Option<SyntaxKind>] {
    static TABLE: OnceLock<Vec<Option<SyntaxKind>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = vec![None; Sq::iter().count()];
        for sq in Sq::iter() {
            let name: &'static str = sq.into();
            let pascal: String = name
                .split('_')
                .flat_map(|w| {
                    w.chars()
                        .next()
                        .map(|c| c.to_ascii_uppercase())
                        .into_iter()
                        .chain(w.chars().skip(1))
                })
                .collect();
            let ours = grammar
                .kind_by_name(&pascal)
                .filter(|k| !k.is_token() && *k != k::FILE)
                .or_else(|| name.ends_with("_statement").then_some(k::STATEMENT));
            table[sq as usize] = ours;
        }
        table
    })
}

struct Walk<'a> {
    src: &'a str,
    kinds: &'static [Option<SyntaxKind>],
    flat: Flat,
    pos: usize,
    /// The type of the segment whose children are visited, and how many it has.
    parent: (Sq, usize),
}

impl Walk<'_> {
    fn visit(&mut self, seg: &ErasedSegment) {
        let children = seg.segments();
        let ty = seg.get_type();
        if children.is_empty() {
            self.leaf(seg, ty);
            return;
        }
        let first = self.flat.leaves.len();
        let ours = match ty {
            Sq::Unparsable => Some(SyntaxKind::ERROR),
            _ => self.kinds[ty as usize].or_else(|| {
                seg.class_types()
                    .contains(Sq::ObjectReference)
                    .then_some(k::OBJECT_REFERENCE)
            }),
        };
        let start = self.pos;
        let outer = std::mem::replace(&mut self.parent, (ty, children.len()));
        for child in children {
            self.visit(child);
        }
        self.parent = outer;
        let end = self.flat.leaves.len();
        if ty == Sq::Unparsable && self.pos > start {
            let len = (self.pos - start) as u32;
            (self.flat.errors).push(ParseError::syntax(start as u32, len, "syntax error"));
        }
        if let Some(kind) = ours.filter(|_| end > first) {
            self.flat.nodes.push(Span { kind, first, end });
        }
    }

    fn leaf(&mut self, seg: &ErasedSegment, ty: Sq) {
        let raw = seg.raw();
        let meta = match ty {
            Sq::Indent => Some(k::INDENT),
            Sq::Implicit => Some(k::IMPLICIT_INDENT),
            Sq::Dedent => Some(k::DEDENT),
            _ => None,
        };
        if let Some(kind) = meta {
            let at = self.pos as u32;
            self.flat.leaves.push(Leaf {
                kind,
                start: at,
                end: at,
            });
            return;
        }
        if raw.is_empty() || seg.is_meta() {
            return;
        }
        let start = self.pos;
        let end = start + raw.len();
        // A leaf that is not the text at its place would break the round trip.
        if self.src.get(start..end) != Some(raw.as_str()) {
            self.pos = usize::MAX / 2;
            return;
        }
        self.pos = end;
        let kind = token_kind(ty, raw);
        let kind = match (kind, self.parent) {
            // Keywords of a data type are types for the capitalisation, and the last part of a
            // qualified function name (a user's function, whose case matters) is not a function; bare
            // functions (`CURRENT_DATE`) are functions.
            (k::KEYWORD, (Sq::DataType | Sq::PrimitiveType | Sq::DatetimeTypeIdentifier, _)) => {
                k::TYPE_IDENTIFIER
            }
            (k::KEYWORD, (Sq::BareFunction, _)) => k::FUNCTION_IDENTIFIER,
            (k::FUNCTION_IDENTIFIER, (Sq::FunctionName, n)) if n > 1 => k::WORD,
            _ => kind,
        };
        if kind == k::UNKNOWN {
            let len = raw.len() as u32;
            (self.flat.errors).push(ParseError::syntax(start as u32, len, "unexpected text"));
        }
        // `pos` order is source order, so a node's leaves are consecutive.
        self.flat.leaves.push(Leaf {
            kind,
            start: start as u32,
            end: end as u32,
        });
    }
}

/// Our token kind for a sqruff leaf; a type this table does not know is classified by its text.
fn token_kind(ty: Sq, raw: &str) -> SyntaxKind {
    let word = raw.starts_with(|c: char| c.is_alphabetic() || c == '_');
    match ty {
        Sq::Whitespace => k::WHITESPACE,
        Sq::Newline => k::NEWLINE,
        Sq::InlineComment => k::LINE_COMMENT,
        Sq::BlockComment => k::BLOCK_COMMENT,
        Sq::Unlexable => k::UNKNOWN,
        Sq::Comma => k::COMMA,
        Sq::Dot => k::DOT,
        Sq::Semicolon => k::SEMICOLON,
        Sq::StatementTerminator if raw == ";" => k::SEMICOLON,
        Sq::StartBracket => k::L_PAREN,
        Sq::EndBracket => k::R_PAREN,
        Sq::StartSquareBracket => k::L_SQUARE,
        Sq::EndSquareBracket => k::R_SQUARE,
        Sq::Keyword | Sq::DatePart => k::KEYWORD,
        Sq::BareFunction => k::FUNCTION_IDENTIFIER,
        Sq::NakedIdentifier => k::IDENTIFIER,
        Sq::FunctionNameIdentifier => k::FUNCTION_IDENTIFIER,
        Sq::DataTypeIdentifier => k::TYPE_IDENTIFIER,
        Sq::BooleanLiteral | Sq::NullLiteral => k::WORD_LITERAL,
        Sq::QuotedIdentifier => k::QUOTED_IDENTIFIER,
        Sq::QuotedLiteral => k::STRING,
        Sq::NumericLiteral => k::NUMBER,
        Sq::CastingOperator => k::CAST_OP,
        Sq::SignIndicator => k::SIGN,
        Sq::Colon => k::COLON,
        Sq::Slice => k::SLICE,
        Sq::PsqlVariableColon => k::PREFIX,
        Sq::BinaryOperator if word => k::BINARY_KEYWORD,
        Sq::BinaryOperator => k::BINARY_OPERATOR,
        Sq::Word => k::WORD,
        Sq::PropertiesNakedIdentifier => k::IDENTIFIER,
        _ if word => k::WORD,
        _ => match raw.chars().next() {
            Some('\'') => k::STRING,
            Some('"' | '`') => k::QUOTED_IDENTIFIER,
            Some(c) if c.is_ascii_digit() => k::NUMBER,
            _ => k::OPERATOR,
        },
    }
}

/// Replays the leaves and nodes into a tree (`provider_api::adapter::replay`): the tokens are the
/// leaves that are not trivia, a node spans its first to its last token, and the trivia between
/// two tokens are the leaves sqruff lexed there.
fn build(src: &str, flat: Flat) -> ParseResult {
    let Flat {
        leaves,
        nodes,
        errors,
    } = flat;
    let grammar = grammars::SQL.get();
    let trivia = |l: &Leaf| grammar.is_trivia(l.kind);
    let meta = |l: &Leaf| l.end == l.start;
    let tokens: Vec<Tok> = (leaves.iter().filter(|l| !trivia(l)))
        .map(|l| Tok {
            kind: l.kind,
            start: l.start,
            end: l.end,
        })
        .collect();
    // `nodes` is in post-order; opening order is by start, outer (later end) first. A node with no
    // token is dropped.
    let mut spans: Vec<(usize, Node)> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let covered = &leaves[n.first..n.end];
            let first = covered.iter().find(|l| !trivia(l) && !meta(l))?;
            let last = covered.iter().rfind(|l| !trivia(l) && !meta(l))?;
            let (start, end, kind) = (first.start, last.end, n.kind);
            Some((i, Node { start, end, kind }))
        })
        .collect();
    spans.sort_by_key(|(i, n)| (n.start, std::cmp::Reverse(n.end), std::cmp::Reverse(*i)));
    let spans: Vec<Node> = spans.into_iter().map(|(_, n)| n).collect();
    // The zero-width leaves share their start with the leaf after them: they are not looked up.
    let lexed = |_: &str, pos: usize| {
        let at = leaves.partition_point(|l| (l.start as usize) < pos);
        let l = leaves[at..].iter().find(|l| !meta(l))?;
        (l.start as usize == pos && trivia(l)).then_some((l.kind, (l.end - l.start) as usize))
    };
    let mut result = replay(src, KINDS, &tokens, &spans, lexed, |t, _| t);
    result.errors = errors;
    result
}
