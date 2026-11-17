//! ruff's AST and tokens as our lossless tree.
//!
//! ruff's AST has no punctuation nodes, no parenthesis information and no trivia. The lexer's
//! tokens and the AST nodes' spans (one `SourceOrderVisitor` walk) go to
//! `provider_api::adapter::replay`, which builds the tree; this file knows ruff's kinds and what
//! lies between tokens: comments and line continuations.
//!
//! - Logical and non-logical newlines, indents and dedents are not tokens of the tree: the bytes
//!   they cover are `Newline` and `Whitespace` trivia, comments are `Comment` trivia.
//! - ruff's expression ranges exclude parentheses: a `Parenthesized` node is made around each
//!   parenthesized expression or pattern, one per pair, nested for `((x))`. The parentheses of a
//!   call or a class's bases belong to their `Arguments` node and make none.
//! - A node with no token (`lambda:` has an empty `Parameters`, error recovery adds names of no
//!   length) is an empty node, so a kind is present wherever ruff's AST has it.
//! - ruff's errors cascade: a file reports the first error of each statement.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, grammars};
use provider_api::adapter::{KindNames, Kinds, Node, Tok, replay};
use provider_api::{ParseError, ParseResult, SourceFile};
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::{AnyNodeRef, NodeKind, PythonVersion};
use ruff_python_parser::{Mode, ParseOptions, parse_unchecked};
use ruff_text_size::Ranged;

/// The newest stable version unless the file's `target_version` names another.
pub fn parse_file(file: &SourceFile) -> ParseResult {
    let target = (file.language_version.as_deref())
        .and_then(|v| v.parse().ok())
        .unwrap_or(PythonVersion::latest());
    parse(&file.text, target)
}

pub fn parse(src: &str, target: PythonVersion) -> ParseResult {
    let result = tree(src, target);
    crate::semantic::note_parse(src, !result.errors.is_empty(), u32::from(target.minor));
    result
}

fn tree(src: &str, target: PythonVersion) -> ParseResult {
    let parsed = parse_unchecked(
        src,
        ParseOptions::from(Mode::Module).with_target_version(target),
    );
    let grammar = grammars::PYTHON.get();
    // Our token kinds by `TokenKind`, our node kinds by `NodeKind`.
    let mut tokens_by_kind = KindNames::new(grammar, 128, |_| false);
    let nodes_by_kind = KindNames::new(grammar, 128, |n| n.contains("Ipy") || n.starts_with("Mod"));
    let tokens: Vec<Tok> = (parsed.tokens().iter())
        .filter(|t| {
            t.start() != t.end()
                && !matches!(
                    t.kind(),
                    TokenKind::Comment
                        | TokenKind::Newline
                        | TokenKind::NonLogicalNewline
                        | TokenKind::Indent
                        | TokenKind::Dedent
                        | TokenKind::EndOfFile
                )
        })
        .map(|t| Tok {
            kind: tokens_by_kind
                .get(t.kind() as usize, true, || format!("{:?}", t.kind()))
                .unwrap_or(k::UNKNOWN),
            start: t.start().to_u32(),
            end: t.end().to_u32(),
        })
        .collect();
    let mut collector = Collector {
        names: nodes_by_kind,
        tokens: &tokens,
        nodes: Vec::with_capacity(tokens.len() / 2),
        open: Vec::new(),
        claimed: Vec::new(),
        stmts: Vec::new(),
    };
    collector.visit_mod(parsed.syntax());
    let (nodes, stmts) = (collector.nodes, collector.stmts);
    let mut errors = first_per_statement(
        (parsed.errors().iter())
            .map(|e| (e.range(), e.error.to_string()))
            .chain((parsed.unsupported_syntax_errors().iter()).map(|e| (e.range(), e.to_string()))),
        &stmts,
    );
    let kinds = Kinds {
        root: k::MOD_MODULE,
        eof: k::END_OF_FILE,
        unknown: k::UNKNOWN,
        whitespace: k::WHITESPACE,
        newline: k::NEWLINE,
    };
    let mut result = replay(src, kinds, &tokens, &nodes, comments, |t, _| t);
    errors.append(&mut result.errors);
    errors.sort_by_key(|e| e.start);
    result.errors = errors;
    result
}

/// The parse of a piece of code found in a string (a quoted annotation), for the newest version.
pub fn parse_snippet(src: &str) -> ParseResult {
    tree(src, PythonVersion::latest())
}

/// A comment or a backslash that ends a line, at the start of the text between two tokens.
fn comments(rest: &str, _: usize) -> Option<(SyntaxKind, usize)> {
    if rest.starts_with('#') {
        Some((k::COMMENT, rest.find(['\n', '\r']).unwrap_or(rest.len())))
    } else {
        let after = rest.strip_prefix('\\')?;
        let len = if after.starts_with("\r\n") { 3 } else { 2 };
        after
            .starts_with(['\n', '\r'])
            .then_some((k::CONTINUATION, len))
    }
}

/// The errors of the parse, one per statement (the innermost one that contains it): the first
/// error of a statement makes the rest of its line noise.
fn first_per_statement(
    errors: impl Iterator<Item = (ruff_text_size::TextRange, String)>,
    stmts: &[(u32, u32)],
) -> Vec<ParseError> {
    let mut all: Vec<_> = errors.collect();
    all.sort_by_key(|(r, _)| r.start());
    let (mut open, mut next): (Vec<(u32, u32)>, usize) = (Vec::new(), 0);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (range, message) in all {
        let at = range.start().to_u32();
        while let Some(&s) = stmts.get(next).filter(|s| s.0 <= at) {
            next += 1;
            while open.last().is_some_and(|o| o.1 <= s.0) {
                open.pop();
            }
            open.push(s);
        }
        while open.last().is_some_and(|o| o.1 <= at) {
            open.pop();
        }
        if open.last().is_none_or(|o| seen.insert(o.0)) {
            out.push(ParseError::syntax(at, range.len().to_u32(), message));
        }
    }
    out
}

struct Collector<'t> {
    names: KindNames,
    tokens: &'t [Tok],
    nodes: Vec<Node>,
    /// The enclosing AST nodes: whether each owns the parentheses at the ends of its range
    /// (`Arguments`), and its end.
    open: Vec<(bool, u32)>,
    /// The range of each enclosing node that took the parentheses around it.
    claimed: Vec<Option<(u32, u32)>>,
    stmts: Vec<(u32, u32)>,
}

impl Collector<'_> {
    /// The ranges of the parentheses around the expression or pattern `start..end`, outermost
    /// last. The closing parenthesis that ends a parent that owns its parentheses is not one.
    fn parens(&self, start: u32, end: u32, limit: Option<u32>) -> Vec<(u32, u32)> {
        let toks = self.tokens;
        let first = toks.partition_point(|t| t.start < start);
        let last = toks.partition_point(|t| t.end <= end);
        let left = toks[..first].iter().rev().take_while(|t| t.kind == k::LPAR);
        let right = toks[last..]
            .iter()
            .take_while(|t| t.kind == k::RPAR && limit.is_none_or(|l| t.end < l));
        left.zip(right).map(|(l, r)| (l.start, r.end)).collect()
    }
}

impl<'a> SourceOrderVisitor<'a> for Collector<'_> {
    fn enter_node(&mut self, node: AnyNodeRef<'a>) -> TraversalSignal {
        let kind = node.kind();
        let range = node.range();
        let (start, end) = (range.start().to_u32(), range.end().to_u32());
        let name = format!("{kind:?}");
        let owns = matches!(kind, NodeKind::Arguments | NodeKind::PatternArguments);
        let parent_limit = self.open.last().filter(|p| p.0).map(|p| p.1);
        self.open.push((owns, end));
        self.claimed.push(None);
        let ours = (self.names.get(kind as usize, false, || name.clone()))
            .filter(|_| kind != NodeKind::ModModule);
        let Some(ours) = ours else {
            return TraversalSignal::Traverse;
        };
        if name.starts_with("Stmt") {
            self.stmts.push((start, end));
        }
        // A pattern and the expression that is all of it share one pair of parentheses.
        let shared = self
            .claimed
            .iter()
            .rev()
            .find_map(|c| *c)
            .is_some_and(|c| c == (start, end));
        if start < end && !shared && (name.starts_with("Expr") || name.starts_with("PatternMatch"))
        {
            if let Some(c) = self.claimed.last_mut() {
                *c = Some((start, end));
            }
            for (s, e) in self.parens(start, end, parent_limit).into_iter().rev() {
                self.nodes.push(Node {
                    start: s,
                    end: e,
                    kind: k::PARENTHESIZED,
                });
            }
        }
        self.nodes.push(Node {
            start,
            end,
            kind: ours,
        });
        // `del a, b` has the targets of `del (a, b)` in a tuple, so that the formatter may add
        // the parentheses.
        if let AnyNodeRef::StmtDelete(d) = node
            && let [first, .., last] = d.targets.as_slice()
        {
            let outer = |s: u32, e: u32, left: bool| {
                let all = self.parens(s, e, None);
                all.last()
                    .map_or(if left { s } else { e }, |p| if left { p.0 } else { p.1 })
            };
            let (fs, fe) = (first.range().start().to_u32(), first.range().end().to_u32());
            let (ls, le) = (last.range().start().to_u32(), last.range().end().to_u32());
            self.nodes.push(Node {
                start: outer(fs, fe, true),
                end: outer(ls, le, false),
                kind: k::EXPR_TUPLE,
            });
        }
        TraversalSignal::Traverse
    }

    fn leave_node(&mut self, _: AnyNodeRef<'a>) {
        self.open.pop();
        self.claimed.pop();
    }
}
