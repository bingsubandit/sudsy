//! The flat output of a parse (leaves and node spans) and its replay into the lossless tree.
//!
//! concept: adapter-replay

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, grammars};
use provider_api::adapter::{Kinds, Node, Tok, replay};
use provider_api::{ParseError, ParseResult};

pub(crate) const KINDS: Kinds = Kinds {
    root: k::FILE,
    eof: k::EOF,
    unknown: k::UNKNOWN,
    whitespace: k::WHITESPACE,
    newline: k::NEWLINE,
};

pub(crate) struct Leaf {
    pub kind: SyntaxKind,
    pub start: u32,
    pub end: u32,
}

/// A grammar node over the leaves `first..end`.
pub(crate) struct Span {
    pub kind: SyntaxKind,
    pub first: usize,
    pub end: usize,
}

/// What a parse collects.
pub(crate) struct Flat {
    pub leaves: Vec<Leaf>,
    pub nodes: Vec<Span>,
    pub errors: Vec<ParseError>,
}

/// Replays the leaves and nodes into a tree (`provider_api::adapter::replay`): the tokens are the
/// leaves that are not trivia, a node spans its first to its last token, and the trivia between
/// two tokens are the tokens the lexer made there.
pub(crate) fn build(src: &str, flat: Flat) -> ParseResult {
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
    core_syntax::spans::sort_nested(&mut spans, |(i, n)| (n.start, n.end, std::cmp::Reverse(*i)));
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
