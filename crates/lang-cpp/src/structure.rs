//! The structural layer: the nodes of the tree, found from the tokens alone.
//!
//! The way clang-format reads a file (it does not preprocess either): tokens are grouped into
//! unwrapped lines (a declaration or statement; a control statement is one line with its body, an
//! `else` goes on the line of its `if`), `{ }` `( )` `[ ]` pairs are groups (a `{ }` is a `Block`
//! where it holds statements or members and an `InitList` where it holds values; `deep` reads
//! what is inside), a preprocessor line is a
//! `Directive` and its tokens are no part of any line or group. The input is never wrong: a
//! closer with no opener is a plain token, a group the text ends early has no closing token, and
//! only the shape of the tree suffers.
//!
//! `#if` branches are read in one pass. At `#else` / `#elif` the groups the branch opened are
//! closed (the next branch starts from the state at `#if`), at `#endif` the state of the last
//! branch goes on. When the groups around the `#if` are the same at `#endif` as they were at
//! `#if`, the `#if` ... `#endif` is a `PpGroup` of `PpBranch` nodes; when a branch closed a group
//! that was opened before the `#if` (`#if A` / `} else {` / `#endif`), only the `Directive`
//! nodes are there.
//!
//! concept: new unwrapped-line-structure
//! what: Nodes found from tokens alone by grouping unwrapped lines, without preprocessing.
//! tools: clang-format

use crate::generated::kind as k;
use crate::lexer::LTok;
use core_syntax::SyntaxKind;
use provider_api::adapter::Node;

#[derive(Clone, Copy, PartialEq)]
enum Open {
    Line,
    Brace,
    Paren,
    Bracket,
}

/// A line that has `struct`, `class`, `union`, `enum` or `typedef`: its `}` is followed by declarators.
const STRUCTLIKE: u8 = 1;
/// A line that has `namespace` or `extern`.
const SCOPE: u8 = 2;
/// A line with a trailing return type (`-> T`): the `{` after the type is the body.
const TRAILING: u8 = 4;

struct Frame {
    open: Open,
    node: usize,
    /// The first token of a line; the opening token of a group.
    first: usize,
    /// The tokens a line has of its own (a group counts as its opener).
    count: u32,
    flags: u8,
    /// The `do` of the line that wait for their `while`.
    dos: u32,
    /// A `?` not yet answered by its `:`.
    ternary: u32,
    /// A brace group that is a block (a body, a scope), not an initializer.
    block: bool,
}

/// The state of an `#if` that is not closed yet.
struct Snap {
    /// The number of open frames at the `#if`, and the fewest since.
    depth: usize,
    low: usize,
    /// The `PpGroup` node and the `PpBranch` nodes.
    nodes: Vec<usize>,
}

const GONE: u32 = u32::MAX;

struct Builder<'a> {
    src: &'a str,
    toks: &'a [LTok],
    nodes: Vec<Node>,
    frames: Vec<Frame>,
    snaps: Vec<Snap>,
    /// The opening token of the `( )` group closed last.
    paren_open: usize,
}

pub fn build(src: &str, toks: &[LTok]) -> Vec<Node> {
    let mut b = Builder {
        src,
        toks,
        nodes: Vec::with_capacity(toks.len() / 4),
        frames: Vec::new(),
        snaps: Vec::new(),
        paren_open: 0,
    };
    b.run();
    b.nodes.retain(|n| n.start != GONE);
    b.nodes
}

/// Tokens that go on with the line before them.
fn continues(kind: SyntaxKind, text: &str) -> bool {
    match kind {
        k::LBRACE
        | k::COLON
        | k::SEMI
        | k::COMMA
        | k::DOT
        | k::ARROW
        | k::RPAR
        | k::RSQB
        | k::RBRACE
        | k::QUESTION
        | k::LPAR
        | k::LSQB => true,
        k::IDENT | k::KEYWORD => {
            matches!(
                text,
                "const" | "noexcept" | "override" | "final" | "volatile" | "throw" | "requires"
            ) || text.starts_with("__")
        }
        // Operators, assignments and comparisons.
        _ => {
            text.len() <= 3
                && !text.is_empty()
                && text.bytes().all(|c| b"+-*/%^&|<>=!".contains(&c))
                && !matches!(
                    kind,
                    k::AMP | k::STAR | k::MINUS | k::PLUS | k::BANG | k::TILDE
                )
        }
    }
}

impl Builder<'_> {
    fn text(&self, i: usize) -> &str {
        &self.src[self.toks[i].start as usize..self.toks[i].end as usize]
    }

    fn next_code(&self, i: usize) -> Option<usize> {
        (i + 1..self.toks.len()).find(|&j| self.toks[j].dir == 0)
    }

    fn prev_code(&self, i: usize) -> Option<usize> {
        (0..i).rev().find(|&j| self.toks[j].dir == 0)
    }

    /// A line break between the end of token `a` and the start of token `b`.
    fn broken(&self, a: usize, b: usize) -> bool {
        self.src.as_bytes()[self.toks[a].end as usize..self.toks[b].start as usize].contains(&b'\n')
    }

    fn push_node(&mut self, start: u32, end: u32, kind: SyntaxKind) -> usize {
        self.nodes.push(Node { start, end, kind });
        self.nodes.len() - 1
    }

    fn push_frame(&mut self, open: Open, first: usize, block: bool) {
        let kind = match open {
            Open::Line => k::LINE,
            Open::Brace if block => k::BLOCK,
            Open::Brace => k::INIT_LIST,
            Open::Paren => k::PAREN_GROUP,
            Open::Bracket => k::BRACKET_GROUP,
        };
        let node = self.push_node(self.toks[first].start, GONE, kind);
        self.frames.push(Frame {
            open,
            node,
            first,
            count: 0,
            flags: 0,
            dos: 0,
            ternary: 0,
            block,
        });
    }

    fn pop(&mut self, end: u32) -> Frame {
        let f = self.frames.pop().expect("a frame to pop");
        self.nodes[f.node].end = end;
        let len = self.frames.len();
        for s in &mut self.snaps {
            s.low = s.low.min(len);
        }
        f
    }

    /// Closes the frames above the first `depth`, at `end`.
    fn close_to(&mut self, depth: usize, end: u32) {
        while self.frames.len() > depth {
            self.pop(end);
        }
    }

    fn top_is(&self, open: Open) -> bool {
        self.frames.last().is_some_and(|f| f.open == open)
    }

    /// The `->` at `i` follows a parameter list or its qualifiers: a trailing return type.
    fn trailing_arrow(&self, i: usize) -> bool {
        self.prev_code(i).is_some_and(|p| {
            self.toks[p].kind == k::RPAR
                || matches!(
                    self.text(p),
                    "const" | "noexcept" | "override" | "final" | "volatile"
                )
        })
    }

    fn ensure_line(&mut self, i: usize) {
        if self.frames.last().is_none_or(|f| f.open == Open::Brace) {
            self.push_frame(Open::Line, i, false);
        }
    }

    /// The token `i` goes on the line that is open, if the innermost frame is one.
    fn bump(&mut self, i: usize) {
        let (flags, ternary) = match self.toks[i].kind {
            k::IDENT | k::KEYWORD => (
                match self.text(i) {
                    "struct" | "class" | "union" | "enum" | "typedef" => STRUCTLIKE,
                    "namespace" | "extern" => SCOPE,
                    _ => 0,
                },
                0,
            ),
            k::ARROW if self.trailing_arrow(i) => (TRAILING, 0),
            k::QUESTION => (0, 1),
            _ => (0, 0),
        };
        if let Some(f) = self.frames.last_mut().filter(|f| f.open == Open::Line) {
            f.count += 1;
            f.flags |= flags;
            f.ternary += ternary;
            f.dos += u32::from(
                self.toks[i].kind == k::KEYWORD
                    && &self.src[self.toks[i].start as usize..self.toks[i].end as usize] == "do",
            );
        }
    }

    fn run(&mut self) {
        let n = self.toks.len();
        let mut i = 0;
        while i < n {
            if self.toks[i].dir != 0 {
                let mut j = i;
                while j + 1 < n && self.toks[j + 1].dir == self.toks[i].dir {
                    j += 1;
                }
                self.directive(i, j);
                i = j + 1;
                continue;
            }
            self.code(i);
            i += 1;
        }
        if let Some(last) = self.toks.last() {
            self.close_to(0, last.end);
        }
        // An `#if` the file ends in: no group.
        for s in std::mem::take(&mut self.snaps) {
            for n in s.nodes {
                self.nodes[n].start = GONE;
            }
        }
    }

    fn directive(&mut self, i: usize, j: usize) {
        let (start, end) = (self.toks[i].start, self.toks[j].end);
        let name = if j > i { self.text(i + 1) } else { "" };
        let before = self.toks[..i].last().map_or(0, |t| t.end);
        match name {
            "if" | "ifdef" | "ifndef" => {
                let group = self.push_node(start, GONE, k::PP_GROUP);
                let branch = self.push_node(start, GONE, k::PP_BRANCH);
                let depth = self.frames.len();
                self.snaps.push(Snap {
                    depth,
                    low: depth,
                    nodes: vec![group, branch],
                });
            }
            "else" | "elif" | "elifdef" | "elifndef" if !self.snaps.is_empty() => {
                let depth = self.snaps.last().map_or(0, |s| s.depth);
                self.close_to(depth, before);
                let last = self.snaps.last().and_then(|s| s.nodes.last().copied());
                if let Some(branch) = last {
                    self.nodes[branch].end = before;
                }
                let branch = self.push_node(start, GONE, k::PP_BRANCH);
                if let Some(s) = self.snaps.last_mut() {
                    s.nodes.push(branch);
                }
            }
            "endif" if !self.snaps.is_empty() => {
                let s = self.snaps.pop().expect("an open #if");
                let consistent = self.frames.len() == s.depth && s.low >= s.depth;
                let (group, branches) = (s.nodes[0], &s.nodes[1..]);
                if let Some(&last) = branches.last() {
                    self.nodes[last].end = before;
                }
                self.nodes[group].end = end;
                if !consistent {
                    for &n in &s.nodes {
                        self.nodes[n].start = GONE;
                    }
                }
            }
            _ => {}
        }
        self.push_node(start, end, k::DIRECTIVE);
    }

    fn code(&mut self, i: usize) {
        let kind = self.toks[i].kind;
        let before = if i > 0 { self.toks[i - 1].end } else { 0 };
        match kind {
            k::RBRACE => match self.frames.iter().rposition(|f| f.open == Open::Brace) {
                Some(pos) => {
                    self.close_to(pos + 1, before);
                    let f = self.pop(self.toks[i].end);
                    self.after_brace(i, &f);
                }
                None => self.plain(i),
            },
            k::RPAR | k::RSQB => {
                let want = if kind == k::RPAR {
                    Open::Paren
                } else {
                    Open::Bracket
                };
                let found = self
                    .frames
                    .iter()
                    .rposition(|f| f.open == want || f.open == Open::Brace)
                    .filter(|&p| self.frames[p].open == want);
                match found {
                    Some(pos) => {
                        self.close_to(pos + 1, before);
                        let f = self.pop(self.toks[i].end);
                        if kind == k::RPAR {
                            self.paren_open = f.first;
                            self.after_paren(i, &f);
                        }
                    }
                    None => self.plain(i),
                }
            }
            k::LPAR | k::LSQB | k::LBRACE => {
                self.ensure_line(i);
                let block = kind == k::LBRACE && self.block_brace(i);
                self.bump(i);
                let open = match kind {
                    k::LPAR => Open::Paren,
                    k::LSQB => Open::Bracket,
                    _ => Open::Brace,
                };
                self.push_frame(open, i, block);
            }
            k::SEMI => {
                self.plain(i);
                // `if (a) b; else c;` and `do a; while (b);` are one line.
                let next = self.next_code(i).map(|n| self.text(n).to_owned());
                let next = next.as_deref();
                let mut do_while = false;
                if next == Some("while")
                    && let Some(f) = self
                        .frames
                        .last_mut()
                        .filter(|f| f.open == Open::Line && f.dos > 0)
                {
                    f.dos -= 1;
                    do_while = true;
                }
                if self.top_is(Open::Line) && next != Some("else") && !do_while {
                    self.pop(self.toks[i].end);
                }
            }
            k::COLON => {
                self.ensure_line(i);
                let end_line = self.label();
                self.plain(i);
                if end_line && self.top_is(Open::Line) {
                    self.pop(self.toks[i].end);
                }
            }
            _ => self.plain(i),
        }
    }

    /// An ordinary token: on the line that is open or on a new one.
    fn plain(&mut self, i: usize) {
        self.ensure_line(i);
        self.bump(i);
        if !self.top_is(Open::Line) {
            return;
        }
        let kind = self.toks[i].kind;
        let text = self.text(i);
        let count = self.frames.last().map_or(0, |f| f.count);
        let next = self.next_code(i);
        // A macro used as a statement: `Q_OBJECT` alone on a line.
        let ends = match (kind, next) {
            (k::IDENT, Some(n)) if count == 1 && macro_name(text) => {
                self.broken(i, n) && !continues(self.toks[n].kind, self.text(n))
            }
            _ => false,
        };
        if ends {
            self.pop(self.toks[i].end);
        }
    }

    /// Whether the `:` at `i` ends the line it is on: after an access specifier, a `case` or
    /// `default`, or a label.
    fn label(&mut self) -> bool {
        let Some(f) = self.frames.last_mut().filter(|f| f.open == Open::Line) else {
            return false;
        };
        if f.ternary > 0 {
            f.ternary -= 1;
            return false;
        }
        let (first, count) = (f.first, f.count);
        let (text, kind) = (self.text(first), self.toks[first].kind);
        match text {
            "public" | "private" | "protected" | "default" => count == 1,
            "case" => true,
            _ => count == 1 && kind == k::IDENT,
        }
    }

    /// Whether the `{` at `i` opens a block (a body or a scope) rather than an initializer.
    fn block_brace(&self, i: usize) -> bool {
        let Some(p) = self.prev_code(i) else {
            return true;
        };
        let line = self.frames.last().filter(|f| f.open == Open::Line);
        let (flags, count) = line.map_or((0, 0), |f| (f.flags, f.count));
        match self.toks[p].kind {
            // `(T){ ... }` after an operator or `return` is a compound literal.
            k::RPAR => !self.prev_code(self.paren_open).is_some_and(|o| {
                matches!(
                    self.toks[o].kind,
                    k::EQUAL | k::QUESTION | k::COMMA | k::LPAR | k::LSQB
                ) || self.text(o) == "return"
            }),
            k::SEMI | k::RBRACE | k::LBRACE | k::COLON | k::AMP | k::AMP_AMP => true,
            // A macro that takes a block: `SCOPE_EXIT {`.
            k::IDENT if count == 1 && self.text(p).len() >= 4 && macro_name(self.text(p)) => true,
            k::IDENT | k::KEYWORD | k::STRING | k::GREATER | k::GREATER_GREATER => {
                matches!(
                    self.text(p),
                    "else"
                        | "do"
                        | "try"
                        | "const"
                        | "volatile"
                        | "noexcept"
                        | "override"
                        | "final"
                ) || (flags & (STRUCTLIKE | SCOPE) != 0 && self.text(p) != "return")
                    || flags & TRAILING != 0
            }
            _ => false,
        }
    }

    /// The `}` at `i` closed `group`: the line it is on goes on or ends.
    fn after_brace(&mut self, i: usize, group: &Frame) {
        let Some(line) = self.frames.last().filter(|f| f.open == Open::Line) else {
            return;
        };
        let (flags, dos) = (line.flags, line.dos);
        let Some(n) = self.next_code(i) else {
            // The last token of the file.
            self.pop(self.toks[i].end);
            return;
        };
        let text = self.text(n);
        let while_do = text == "while" && dos > 0;
        let always = matches!(
            self.toks[n].kind,
            k::SEMI | k::COMMA | k::RPAR | k::RSQB | k::DOT | k::ARROW | k::COLON | k::QUESTION
        ) || text == "else"
            || text == "catch"
            || while_do
            // The end of an initializer: the expression goes on.
            || (!group.block && continues(self.toks[n].kind, text));
        let same_line = !self.broken(i, n);
        // `[] { ... }()`: the call follows the body.
        let called = self.toks[n].kind == k::LPAR && self.toks[n].start == self.toks[i].end;
        let goes_on = always || called || same_line && (!group.block || flags & STRUCTLIKE != 0);
        if !goes_on {
            self.pop(self.toks[i].end);
        } else if let Some(f) = self.frames.last_mut() {
            // The braces of a class are closed: what follows are its declarators.
            if group.block {
                f.flags &= !STRUCTLIKE;
            }
            if while_do {
                f.dos -= 1;
            }
        }
    }

    /// The `)` at `i` closed `group`: a call used as a statement (`BEGIN_NAMESPACE(x)`) ends its
    /// line when the next token starts a new statement on the next line; the header of a control
    /// statement goes on with its body.
    fn after_paren(&mut self, i: usize, group: &Frame) {
        let Some(line) = self.frames.last().filter(|f| f.open == Open::Line) else {
            return;
        };
        let (first, count) = (line.first, line.count);
        let Some(n) = self.next_code(i) else {
            return;
        };
        let open = group.first;
        let control = self.prev_code(open).is_some_and(|p| {
            let text = self.text(p);
            matches!(text, "if" | "for" | "while" | "switch" | "catch")
                || (matches!(text, "constexpr" | "consteval")
                    && self.prev_code(p).is_some_and(|q| self.text(q) == "if"))
        });
        let ends = if control {
            false
        } else {
            count == 2
                && open == first + 1
                && self.toks[first].kind == k::IDENT
                && self.broken(i, n)
                && !continues(self.toks[n].kind, self.text(n))
        };
        if ends {
            self.pop(self.toks[i].end);
        }
    }
}

/// `Q_OBJECT`, `BEGIN_NAMESPACE`: two or more capitals, digits and underscores.
pub(crate) fn macro_name(s: &str) -> bool {
    s.len() >= 2
        && s.bytes().any(|c| c.is_ascii_uppercase())
        && s.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}
