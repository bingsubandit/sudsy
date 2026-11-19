//! The deep layer: statements, declarations and expressions inside the unwrapped lines and
//! bracket groups of `structure`.
//!
//! Every `Line` is one statement or declaration; it is read by lookahead on the tokens (no
//! checkpoint or rollback): what the lookahead cannot decide stays a token or an opaque node
//! (`Line`, `Parenthesized` with loose parts). A node that covers a structural node exactly takes
//! its place (the line becomes `IfStatement`, the `( )` group `Arguments`); every other node is
//! new, merged into the list by start, then longest first, then outer first.
//!
//! The ambiguities are settled the way clang-format settles them: `<` opens template arguments
//! when a `>` that closes them follows over tokens a template argument can have; a name followed
//! by another name, or by `*`/`&` and a name, starts a declaration; a `( )` group after a name is
//! a parameter list in a declaration scope and a constructor call in a function body.
//!
//! concept: hand-written-parser-kit

mod decl;
mod expr;
mod stmt;

use crate::generated::kind as k;
use crate::lexer::LTok;
use core_syntax::SyntaxKind;
use core_syntax::spans::{self, Directives, Extents, Groups};
use provider_api::adapter::Node;

const NONE: u32 = spans::NONE;
const GONE: u32 = u32::MAX;

/// What the lines of a `Block` are: unset (the enclosing block's), a namespace, a class, a function.
const UNSET: u8 = 0;
const SCOPE: u8 = 1;
const CLASS: u8 = 2;
const BODY: u8 = 3;
/// The values of an initializer list, the enumerators of an enum.
const INIT: u8 = 4;
const ENUM: u8 = 5;

#[derive(Clone, Copy)]
struct Tk {
    kind: SyntaxKind,
    start: u32,
    end: u32,
}

struct Cx<'a> {
    src: &'a str,
    /// The tokens outside preprocessor directives.
    c: Vec<Tk>,
    /// The bracket groups of the tokens, by their structural nodes.
    groups: Groups,
    nodes: Vec<Node>,
    extra: Vec<Node>,
    /// The scope of each `Block` node, set by the construct that owns it.
    scope: Vec<u8>,
    /// The expression being read: the next token and the end of the range.
    pos: usize,
    lim: usize,
    /// Lookahead: nodes are not made.
    quiet: bool,
    /// The preprocessor directives between code tokens.
    dirs: Directives,
}

/// The nodes of `structure` with the statements, declarations and expressions inside them.
pub fn build(src: &str, toks: &[LTok], nodes: Vec<Node>) -> Vec<Node> {
    let mut cx = Cx::new(src, toks, nodes);
    cx.run();
    cx.finish()
}

impl<'a> Cx<'a> {
    fn new(src: &'a str, toks: &[LTok], nodes: Vec<Node>) -> Self {
        let c: Vec<Tk> = (toks.iter())
            .filter(|t| t.dir == 0)
            .map(|t| Tk {
                kind: t.kind,
                start: t.start,
                end: t.end,
            })
            .collect();
        let spans: Vec<(u32, u32)> = c.iter().map(|t| (t.start, t.end)).collect();
        let is_group = |n: &Node| {
            matches!(
                n.kind,
                k::PAREN_GROUP | k::BRACKET_GROUP | k::BLOCK | k::INIT_LIST
            )
        };
        let groups = Groups::new(
            &spans,
            (nodes.iter().enumerate())
                .filter(|(_, n)| is_group(n))
                .map(|(i, n)| (i, n.start, n.end)),
        );
        // A brace group the text ends early has no closer: it is no block.
        let mut nodes = nodes;
        for open in 0..c.len() {
            if let (Some(g), Some(l)) = (groups.node(open), groups.last(open))
                && matches!(nodes[g].kind, k::BLOCK | k::INIT_LIST)
                && (c[l].kind != k::RBRACE || groups.ender(l) != Some(open))
            {
                nodes[g].kind = k::BRACE_GROUP;
            }
        }
        let dirs = Directives::new(toks.iter().map(|t| t.dir), &groups);
        let scope = vec![UNSET; nodes.len()];
        Cx {
            src,
            c,
            groups,
            nodes,
            extra: Vec::new(),
            scope,
            pos: 0,
            lim: 0,
            quiet: false,
            dirs,
        }
    }

    /// Reads every line of the file, in order: the scope a line is in is the one of the innermost
    /// `Block` around it, which the owner of the block set when its own line was read.
    fn run(&mut self) {
        let mut stack: Extents<u8> = Extents::default();
        for i in 0..self.nodes.len() {
            let (n_start, n_end, n_kind) =
                (self.nodes[i].start, self.nodes[i].end, self.nodes[i].kind);
            stack.leave(n_start, |case, end| self.close_case(case, end));
            let scope = stack.top().copied().unwrap_or(SCOPE);
            match n_kind {
                k::BLOCK | k::INIT_LIST | k::ARGUMENTS | k::PARENTHESIZED | k::LAMBDA => {
                    let own = match (self.scope[i], n_kind) {
                        (UNSET, k::INIT_LIST) => INIT,
                        (UNSET, _) => scope,
                        (own, _) => own,
                    };
                    // A `{ }` of values is not a block, wherever the structure took it for one.
                    if own == INIT && n_kind == k::BLOCK {
                        self.nodes[i].kind = k::INIT_LIST;
                    }
                    stack.enter(n_end, own);
                }
                // A line that is a label closes the one before it.
                k::LINE if self.line(i, scope) => {
                    stack.label(i as u32, n_start, |case, end| self.close_case(case, end));
                }
                k::LINE => {}
                _ => {}
            }
        }
        stack.finish(|case, end| self.close_case(case, end));
    }

    /// A `case` label covers the lines after it up to the next label or the end of the block.
    fn close_case(&mut self, case: u32, before: u32) {
        let at = self.c.partition_point(|t| t.end <= before);
        let mut e = at;
        // The `}` of the block is not part of the last case.
        if e > 0 && self.c[e - 1].kind == k::RBRACE && self.c[e - 1].end == before {
            e -= 1;
        }
        if e > 0 && self.c[e - 1].end > self.nodes[case as usize].end {
            self.nodes[case as usize].end = self.c[e - 1].end;
        }
    }

    fn finish(mut self) -> Vec<Node> {
        let base = self.nodes.len() as u32;
        let mut keyed: Vec<(Node, u32)> = Vec::with_capacity(self.nodes.len() + self.extra.len());
        for (i, n) in self.nodes.drain(..).enumerate() {
            if n.start != GONE {
                keyed.push((n, i as u32));
            }
        }
        // New nodes after the structural ones that cover the same text, the one made last
        // (the outer one) first.
        let count = self.extra.len() as u32;
        for (i, n) in std::mem::take(&mut self.extra).into_iter().enumerate() {
            keyed.push((n, base + (count - i as u32)));
        }
        spans::sort_nested(&mut keyed, |(n, o)| (n.start, n.end, *o));
        keyed.into_iter().map(|(n, _)| n).collect()
    }

    // ---- tokens

    fn text(&self, i: usize) -> &'a str {
        let t = self.c[i];
        &self.src[t.start as usize..t.end as usize]
    }

    fn is(&self, i: usize, b: usize, kind: SyntaxKind) -> bool {
        i < b && self.c[i].kind == kind
    }

    /// The token at `i` is the word `s` (a keyword, or a name for the contextual ones).
    fn word(&self, i: usize, b: usize, s: &str) -> bool {
        i < b && matches!(self.c[i].kind, k::KEYWORD | k::IDENT) && self.text(i) == s
    }

    fn name_like(&self, i: usize, b: usize) -> bool {
        i < b && matches!(self.c[i].kind, k::IDENT | k::KEYWORD)
    }

    /// The token after the one at `i`, or after its whole group.
    fn skip(&self, i: usize) -> usize {
        self.groups.skip(i)
    }

    /// The tokens between the brackets of the group opened at `i`.
    fn inner(&self, i: usize) -> (usize, usize) {
        self.groups.inner(i, |l| self.closer(l))
    }

    /// The token at `l` is a closing bracket.
    fn closer(&self, l: usize) -> bool {
        matches!(self.c[l].kind, k::RPAR | k::RSQB | k::RBRACE)
    }

    fn open(&self, i: usize, b: usize, kind: SyntaxKind) -> bool {
        self.is(i, b, kind) && self.groups.is_open(i)
    }

    // ---- nodes

    /// A directive lies between two parts of the tokens `a..=z`, not inside a group of them.
    fn crosses(&self, a: usize, z: usize) -> bool {
        self.dirs.crosses(a, z)
    }

    /// A directive lies between two parts of the group opened at `i`.
    fn crosses_in(&self, i: usize) -> bool {
        self.dirs.crosses_in(i, &self.groups)
    }

    /// The group opened at `i` has its closing token.
    fn closed(&self, i: usize) -> bool {
        self.groups.closed(i, |l| self.closer(l))
    }

    /// A node over the tokens `a..=z`; none when a directive splits them.
    fn emit(&mut self, a: usize, z: usize, kind: SyntaxKind) {
        if self.quiet || self.crosses(a, z) {
            return;
        }
        let n = Node {
            start: self.c[a].start,
            end: self.c[z].end,
            kind,
        };
        self.extra.push(n);
    }

    /// The structural node of the group at `i` becomes `kind`.
    fn retag(&mut self, i: usize, kind: SyntaxKind) {
        if let Some(g) = self.groups.node(i) {
            self.nodes[g].kind = kind;
        }
    }

    fn group_kind(&self, i: usize) -> Option<SyntaxKind> {
        self.groups.node(i).map(|g| self.nodes[g].kind)
    }

    /// A `Block` that a construct owns: its lines are of `scope`.
    fn own_block(&mut self, i: usize, scope: u8) {
        if let Some(g) = self.groups.node(i) {
            self.scope[g] = scope;
        }
    }
}
