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
//! concept: new cpp-deep-parse
//! what: The statements, declarations and expressions of C/C++ found by lookahead inside the lines of the structural layer.
//! tools: clang-format

mod decl;
mod expr;
mod stmt;

use crate::generated::kind as k;
use crate::lexer::LTok;
use core_syntax::SyntaxKind;
use provider_api::adapter::Node;

const NONE: u32 = u32::MAX;
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
    /// For an opening token: the index of the last token of its group; else `NONE`.
    last: Vec<u32>,
    /// For an opening token: the structural node of its group.
    group: Vec<u32>,
    /// For a token that ends a group: the innermost group it ends (an unclosed group ends at
    /// the closer of another).
    ender: Vec<u32>,
    nodes: Vec<Node>,
    extra: Vec<Node>,
    /// The scope of each `Block` node, set by the construct that owns it.
    scope: Vec<u8>,
    /// The expression being read: the next token and the end of the range.
    pos: usize,
    lim: usize,
    /// Lookahead: nodes are not made.
    quiet: bool,
    /// The preprocessor directives between code tokens: the number of code tokens before it and
    /// the group it is in (the opening token's index, -1 for none), in order.
    dirs: Vec<(u32, i64)>,
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
        let mut last = vec![NONE; c.len()];
        let mut group = vec![NONE; c.len()];
        for (i, n) in nodes.iter().enumerate() {
            if matches!(
                n.kind,
                k::PAREN_GROUP | k::BRACKET_GROUP | k::BLOCK | k::INIT_LIST
            ) {
                let open = c.partition_point(|t| t.start < n.start);
                let end = c.partition_point(|t| t.end <= n.end);
                if open < c.len() && c[open].start == n.start && end > open {
                    last[open] = (end - 1) as u32;
                    group[open] = i as u32;
                }
            }
        }
        let mut ender = vec![NONE; c.len()];
        for (open, &l) in last.iter().enumerate() {
            if l != NONE {
                ender[l as usize] = open as u32;
            }
        }
        // A brace group the text ends early has no closer: it is no block.
        let mut nodes = nodes;
        for (open, &g) in group.iter().enumerate() {
            if g != NONE
                && matches!(nodes[g as usize].kind, k::BLOCK | k::INIT_LIST)
                && (c[last[open] as usize].kind != k::RBRACE
                    || ender[last[open] as usize] != open as u32)
            {
                nodes[g as usize].kind = k::BRACE_GROUP;
            }
        }
        let dirs = directives(toks, &last);
        let scope = vec![UNSET; nodes.len()];
        Cx {
            src,
            c,
            last,
            group,
            ender,
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
        // The end, the scope and the open `case` node of each enclosing block.
        let mut stack: Vec<(u32, u8, u32)> = Vec::new();
        for i in 0..self.nodes.len() {
            let (n_start, n_end, n_kind) =
                (self.nodes[i].start, self.nodes[i].end, self.nodes[i].kind);
            while let Some(&(end, _, case)) = stack.last()
                && end <= n_start
            {
                self.close_case(case, end);
                stack.pop();
            }
            let scope = stack.last().map_or(SCOPE, |s| s.1);
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
                    stack.push((n_end, own, NONE));
                }
                k::LINE => {
                    let label = self.line(i, scope);
                    if let (Some(top), true) = (stack.last_mut(), label) {
                        self.close_case(top.2, n_start);
                        top.2 = i as u32;
                    }
                }
                _ => {}
            }
        }
        while let Some((end, _, case)) = stack.pop() {
            self.close_case(case, end);
        }
    }

    /// A `case` label covers the lines after it up to the next label or the end of the block.
    fn close_case(&mut self, case: u32, before: u32) {
        if case == NONE {
            return;
        }
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
        let mut keyed: Vec<(u32, u32, u32, SyntaxKind)> =
            Vec::with_capacity(self.nodes.len() + self.extra.len());
        for (i, n) in self.nodes.iter().enumerate() {
            if n.start != GONE {
                keyed.push((n.start, !n.end, i as u32, n.kind));
            }
        }
        // New nodes after the structural ones that cover the same text, the one made last
        // (the outer one) first.
        let base = self.nodes.len() as u32;
        let count = self.extra.len() as u32;
        for (i, n) in std::mem::take(&mut self.extra).into_iter().enumerate() {
            keyed.push((n.start, !n.end, base + (count - i as u32), n.kind));
        }
        keyed.sort_unstable_by_key(|&(s, e, o, _)| (s, e, o));
        keyed
            .into_iter()
            .map(|(start, end, _, kind)| Node {
                start,
                end: !end,
                kind,
            })
            .collect()
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
        match self.last[i] {
            NONE => i + 1,
            l => l as usize + 1,
        }
    }

    /// The tokens between the brackets of the group opened at `i`.
    fn inner(&self, i: usize) -> (usize, usize) {
        let l = self.last[i] as usize;
        (i + 1, if self.closed(i) { l } else { l + 1 })
    }

    fn open(&self, i: usize, b: usize, kind: SyntaxKind) -> bool {
        self.is(i, b, kind) && self.last[i] != NONE
    }

    // ---- nodes

    /// A directive lies between two parts of the tokens `a..=z`, not inside a group of them.
    fn crosses(&self, a: usize, z: usize) -> bool {
        let from = self.dirs.partition_point(|d| d.0 as usize <= a);
        (self.dirs[from..].iter())
            .take_while(|d| d.0 as usize <= z)
            .any(|d| d.1 < a as i64)
    }

    /// A directive lies between two parts of the group opened at `i`.
    fn crosses_in(&self, i: usize) -> bool {
        let last = self.last[i] as usize;
        let from = self.dirs.partition_point(|d| d.0 as usize <= i);
        (self.dirs[from..].iter())
            .take_while(|d| d.0 as usize <= last)
            .any(|d| d.1 == i as i64)
    }

    /// The group opened at `i` has its closing token.
    fn closed(&self, i: usize) -> bool {
        let l = self.last[i];
        l != NONE
            && l as usize > i
            && self.ender[l as usize] == i as u32
            && matches!(self.c[l as usize].kind, k::RPAR | k::RSQB | k::RBRACE)
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
        if let Some(&g) = self.group.get(i)
            && g != NONE
        {
            self.nodes[g as usize].kind = kind;
        }
    }

    fn group_kind(&self, i: usize) -> Option<SyntaxKind> {
        match self.group.get(i) {
            Some(&g) if g != NONE => Some(self.nodes[g as usize].kind),
            _ => None,
        }
    }

    /// A `Block` that a construct owns: its lines are of `scope`.
    fn own_block(&mut self, i: usize, scope: u8) {
        if let Some(&g) = self.group.get(i)
            && g != NONE
        {
            self.scope[g as usize] = scope;
        }
    }
}

/// Where the directives are: before which code token, and in which group.
fn directives(toks: &[LTok], last: &[u32]) -> Vec<(u32, i64)> {
    let mut dirs = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let (mut ci, mut seen) = (0usize, 0u32);
    for t in toks {
        if t.dir == 0 {
            while stack.last().is_some_and(|&o| (last[o] as usize) < ci) {
                stack.pop();
            }
            if last[ci] != NONE {
                stack.push(ci);
            }
            ci += 1;
        } else if t.dir != seen {
            seen = t.dir;
            while stack.last().is_some_and(|&o| (last[o] as usize) < ci) {
                stack.pop();
            }
            dirs.push((ci as u32, stack.last().map_or(-1, |&o| o as i64)));
        }
    }
    dirs
}
