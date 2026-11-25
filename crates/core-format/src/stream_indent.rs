//! Block indentation of a line-preserving formatter: the first line of each statement of a body
//! stands one indent step in from the line its body opens on, and the closing and branch keywords
//! (`end`, `else`, `rescue`, `when`) stand where that line stands. A line that is neither moves by
//! the same amount as the statement it belongs to, so a multi-line call or literal keeps its
//! shape. A body whose opening keyword is not the first token of its line (`x = if c`) is left as
//! it is. The kinds and closing words are the plain facts of the language (`format-rows` of
//! `lang.yaml`).
//!
//! concept: stream-block-indent
//! what: Statement lines indented one step inside the body that opens them, closing and branch keywords aligned with the opening line.
//! tools: RuboCop Layout IndentationWidth, EndAlignment, DefEndAlignment, ElseAlignment, CaseIndentation, RescueEnsureAlignment
//! reads: path:stream_indent::IndentFacts

use crate::FormatCtx;
use crate::stream::Tk;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken};

/// The kind names and words of a language's block structure.
#[derive(Clone, Copy)]
pub struct IndentFacts {
    /// The kind of a list of statements.
    pub statements: &'static [&'static str],
    /// Kinds whose body stands in from the line of the statement that holds them.
    pub frames: &'static [&'static str],
    /// Kinds of operations with two operands (`&&`, `||`).
    pub operations: &'static [&'static str],
    /// Kinds whose condition lines up its operations.
    pub conditions: &'static [&'static str],
    /// Kinds of argument and element lists, whose chains stay as they are.
    pub opaque: &'static [&'static str],
    /// Kinds of the key-value pairs of a literal.
    pub pairs: &'static [&'static str],
    /// Kinds of calls that make a chain.
    pub chains: &'static [&'static str],
    /// Kinds of the dots that start a continued call on a line of its own.
    pub leaders: &'static [&'static str],
    /// Kinds that start where their parent starts (a block starts with its call).
    pub attached: &'static [&'static str],
    /// Kinds whose body stands in from their own keyword, which is the first token of its line.
    pub keyword_frames: &'static [&'static str],
    /// Kinds that continue a frame: their keyword stands where the frame's line stands.
    pub branches: &'static [&'static str],
    /// Words that close a frame.
    pub closers: &'static [&'static str],
    /// Kinds of the opener of a heredoc (`<<~EOS`) and of its body, which holds its closing label.
    pub heredoc_openers: &'static [&'static str],
    pub heredoc_bodies: &'static [&'static str],
    /// Kinds between a call and its heredoc argument (the argument list).
    pub heredoc_wrappers: &'static [&'static str],
    /// Kinds of the brackets that open and close a list of elements, in the same order.
    pub open_brackets: &'static [&'static str],
    pub close_brackets: &'static [&'static str],
    /// Kinds whose children are the elements of a list (arguments, array and hash elements).
    pub elements: &'static [&'static str],
}

impl IndentFacts {
    pub const NONE: IndentFacts = IndentFacts {
        statements: &[],
        frames: &[],
        operations: &[],
        conditions: &[],
        opaque: &[],
        pairs: &[],
        chains: &[],
        leaders: &[],
        attached: &[],
        keyword_frames: &[],
        branches: &[],
        closers: &[],
        heredoc_openers: &[],
        heredoc_bodies: &[],
        heredoc_wrappers: &[],
        open_brackets: &[],
        close_brackets: &[],
        elements: &[],
    };
}

#[derive(Default)]
pub struct Indents {
    closers: &'static [&'static str],
    statements: Vec<SyntaxKind>,
    frames: Vec<SyntaxKind>,
    operations: Vec<SyntaxKind>,
    conditions: Vec<SyntaxKind>,
    opaque: Vec<SyntaxKind>,
    pairs: Vec<SyntaxKind>,
    chains: Vec<SyntaxKind>,
    leaders: Vec<SyntaxKind>,
    attached: Vec<SyntaxKind>,
    keyword_frames: Vec<SyntaxKind>,
    branches: Vec<SyntaxKind>,
    heredoc_openers: Vec<SyntaxKind>,
    heredoc_bodies: Vec<SyntaxKind>,
    heredoc_wrappers: Vec<SyntaxKind>,
    open_brackets: Vec<SyntaxKind>,
    close_brackets: Vec<SyntaxKind>,
    elements: Vec<SyntaxKind>,
}

/// A bracket that is open at the current token.
struct Open {
    /// The source line of the opener, and whether it is the last token of that line.
    at: usize,
    line: usize,
    hanging: bool,
    /// The source line of the first element, and how far the line of the last element moved.
    first: Option<usize>,
    shift: isize,
}

/// For the first token of each line: the nodes that start with it, innermost first, and the node
/// above them.
#[derive(Default)]
pub struct Leads {
    nodes: Vec<SyntaxNode>,
    /// By token: where its nodes start in `nodes`, how many start with it, and whether the node
    /// above them follows.
    at: Vec<(usize, usize, bool)>,
}

/// The nodes that start with one token.
#[derive(Clone, Copy)]
pub struct Lead<'a> {
    pub nodes: &'a [SyntaxNode],
    pub above: Option<&'a SyntaxNode>,
}

impl Leads {
    pub fn new(cx: &FormatCtx<'_>, tks: &[Tk]) -> Leads {
        let mut nodes = Vec::new();
        let mut at = Vec::with_capacity(tks.len());
        let eof = cx.grammar.kind_by_name("Eof");
        for (i, tk) in tks.iter().enumerate() {
            let start = nodes.len();
            if !(i == 0 || tk.newline || Some(tk.kind) == eof) {
                at.push((start, 0, false));
                continue;
            }
            let mut cur = tk.tok.parent();
            let mut above = false;
            let at_start = tk.tok.text_range().start();
            while let Some(n) = cur {
                // A node that starts at the token starts with it; one that starts earlier is looked at.
                let starts = match n.text_range().start().cmp(&at_start) {
                    std::cmp::Ordering::Equal => true,
                    std::cmp::Ordering::Greater => false,
                    std::cmp::Ordering::Less => cx.first_token(&n).as_ref() == Some(&tk.tok),
                };
                if starts {
                    cur = n.parent();
                    nodes.push(n);
                } else {
                    nodes.push(n);
                    above = true;
                    cur = None;
                }
            }
            at.push((start, nodes.len() - start - usize::from(above), above));
        }
        Leads { nodes, at }
    }

    pub fn of(&self, i: usize) -> Lead<'_> {
        let (start, len, above) = self.at[i];
        Lead {
            nodes: &self.nodes[start..start + len],
            above: above.then(|| &self.nodes[start + len]),
        }
    }
}

impl Lead<'_> {
    /// The outermost of the nodes whose parent is of one of the kinds.
    pub fn outermost(&self, kinds: &[SyntaxKind]) -> Option<&SyntaxNode> {
        (0..self.nodes.len()).rev().find_map(|k| {
            let up = self.nodes.get(k + 1).or(self.above);
            up.filter(|p| kinds.contains(&p.kind()))?;
            Some(&self.nodes[k])
        })
    }
}

/// The state of one pass over the tokens.
struct Pass<'a, 'b> {
    cx: &'a FormatCtx<'b>,
    tks: &'a [Tk],
    leads: &'a Leads,
    width: usize,
    /// The indentation each source line gets, by its line number.
    indent: Lines<usize>,
    /// How far each source line moved.
    moved: Lines<isize>,
    /// Where the statements of a body stand, by the body's frame (the statements of a body share it).
    bodies: std::cell::RefCell<Vec<(rowan::TextRange, SyntaxKind, Option<usize>)>>,
}

/// A value for some of the source lines, by line number.
struct Lines<T>(Vec<Option<T>>);

impl<T> Lines<T> {
    fn new(lines: usize) -> Lines<T> {
        Lines((0..lines).map(|_| None).collect())
    }

    fn get(&self, line: &usize) -> Option<&T> {
        self.0.get(*line)?.as_ref()
    }

    fn insert(&mut self, line: usize, value: T) {
        self.0[line] = Some(value);
    }
}

impl Indents {
    pub fn new(g: &Grammar, facts: IndentFacts) -> Indents {
        let kinds = |names: &[&str]| -> Vec<SyntaxKind> {
            (names.iter())
                .map(|n| {
                    g.kind_by_name(n)
                        .unwrap_or_else(|| panic!("format-rows: unknown kind `{n}`"))
                })
                .collect()
        };
        Indents {
            closers: facts.closers,
            statements: kinds(facts.statements),
            frames: kinds(facts.frames),
            operations: kinds(facts.operations),
            conditions: kinds(facts.conditions),
            opaque: kinds(facts.opaque),
            pairs: kinds(facts.pairs),
            chains: kinds(facts.chains),
            leaders: kinds(facts.leaders),
            attached: kinds(facts.attached),
            keyword_frames: kinds(facts.keyword_frames),
            branches: kinds(facts.branches),
            heredoc_openers: kinds(facts.heredoc_openers),
            heredoc_bodies: kinds(facts.heredoc_bodies),
            heredoc_wrappers: kinds(facts.heredoc_wrappers),
            open_brackets: kinds(facts.open_brackets),
            close_brackets: kinds(facts.close_brackets),
            elements: kinds(facts.elements),
        }
    }

    /// The first token of the outermost call that has the heredoc of `opener` as an argument or
    /// receiver (through calls and argument lists only).
    fn heredoc_outer(&self, opener: &SyntaxToken) -> Option<SyntaxToken> {
        let mut outer = None;
        for n in opener.parent()?.ancestors().skip(1) {
            if self.chains.contains(&n.kind()) {
                outer = Some(n);
            } else if !self.heredoc_wrappers.contains(&n.kind()) {
                break;
            }
        }
        outer?.first_token()
    }

    /// The text that replaces the body of the heredocs of the file, by token index
    /// (`shifts`: how far the line of each token moves). A body that is not at `width` in from its opener's line is moved there when
    /// its opener is `<<~` (the value stays: every line loses the same indentation); the closing label stands at the opener's
    /// line, or where it is when the line of the call that holds the heredoc has it.
    pub fn heredocs(
        &self,
        tks: &[Tk],
        shifts: &[isize],
        width: usize,
    ) -> std::collections::HashMap<usize, String> {
        let mut out = std::collections::HashMap::new();
        let mut openers: std::collections::VecDeque<(usize, usize)> = Default::default();
        let mut line_indents = Lines::new(tks.last().map_or(0, |t| t.last_line + 1));
        let mut line_indent = 0;
        for (i, tk) in tks.iter().enumerate() {
            let is_body = self.heredoc_bodies.contains(&tk.kind);
            if !is_body && (i == 0 || tk.newline) {
                let shift = shifts.get(i).copied().unwrap_or(0);
                line_indent = (tk.gap.len() as isize + shift).max(0) as usize;
                line_indents.insert(tk.first_line, line_indent);
            }
            if self.heredoc_openers.contains(&tk.kind) {
                openers.push_back((i, line_indent));
            } else if is_body && let Some((o, base)) = openers.pop_front() {
                let outer = self.heredoc_outer(&tks[o].tok).and_then(|t| {
                    let at = tks
                        .partition_point(|k| k.tok.text_range().start() < t.text_range().start());
                    line_indents.get(&tks.get(at)?.first_line).copied()
                });
                if let Some(body) = reindent(tks[o].text(), tk.text(), (base, outer), width) {
                    out.insert(i, body);
                }
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    /// How many columns the line of each token moves (the comments in front of it move with it).
    pub fn plan(&self, cx: &FormatCtx<'_>, tks: &[Tk], leads: &Leads, width: usize) -> Vec<isize> {
        let mut shifts = vec![0; tks.len()];
        if self.is_empty() || tks.iter().any(|t| t.gap.contains('\t')) {
            return shifts;
        }
        let eof = cx.grammar.kind_by_name("Eof");
        let lines = tks.last().map_or(0, |t| t.last_line + 1);
        let mut pass = Pass {
            cx,
            tks,
            leads,
            width,
            indent: Lines::new(lines),
            moved: Lines::new(lines),
            bodies: Default::default(),
        };
        let mut open: Vec<Open> = Vec::new();
        for i in 0..tks.len() {
            if (i == 0 || tks[i].newline) && Some(tks[i].kind) != eof {
                let line = tks[i].first_line;
                let src = tks[i].gap.len() as isize;
                let shift = match self.target(&pass, i) {
                    Some(t) => t as isize - src,
                    None => match self.in_list(&pass, i, &mut open) {
                        Some(t) => t as isize - src,
                        None => open
                            .iter()
                            .rev()
                            .find(|o| o.hanging && o.first.is_some())
                            .map_or_else(|| self.inherited(&pass, i), |o| o.shift),
                    },
                };
                pass.indent.insert(line, (src + shift).max(0) as usize);
                pass.moved.insert(line, shift);
                shifts[i] = shift;
            }
            let kind = tks[i].kind;
            if self.open_brackets.contains(&kind) {
                open.push(Open {
                    at: i,
                    line: tks[i].first_line,
                    hanging: tks.get(i + 1).is_some_and(|n| n.newline),
                    first: None,
                    shift: 0,
                });
            } else if self.close_brackets.contains(&kind) {
                open.pop();
            }
        }
        shifts
    }

    /// Where the line that starts with token `i` stands in the list the innermost open bracket
    /// holds, when the bracket is the last token of its line: the first element one step in from
    /// the line of the bracket, later elements under it, the closer at the line of the bracket.
    fn in_list(&self, pass: &Pass<'_, '_>, i: usize, open: &mut [Open]) -> Option<usize> {
        let (top, outer) = open.split_last_mut()?;
        // A list in the arguments of a call whose first argument is on the line of its bracket
        // stands where the call's arguments stand.
        if !top.hanging || outer.iter().any(|o| !o.hanging && o.line == top.line) {
            return None;
        }
        if self.close_brackets.contains(&pass.tks[i].kind) {
            return pass.indent.get(&top.line).copied();
        }
        let lead = pass.leads.of(i);
        // The element belongs to the list that the bracket opens.
        let list = lead.outermost(&self.elements)?.parent()?;
        let o = top.at;
        let j = pass
            .cx
            .first_token(&list)
            .and_then(|t| self.index(pass, &t))?;
        if j != o && !(j == o + 1 && pass.tks[o].tok.parent() == list.parent()) {
            return None;
        }
        let at = match top.first {
            None => {
                top.first = Some(pass.tks[i].first_line);
                *pass.indent.get(&top.line)? + pass.width
            }
            Some(first) => *pass.indent.get(&first)?,
        };
        top.shift = at as isize - pass.tks[i].gap.len() as isize;
        Some(at)
    }

    /// The line the token is on, of a token of the stream.
    fn index(&self, pass: &Pass<'_, '_>, t: &SyntaxToken) -> Option<usize> {
        let at = t.text_range().start();
        pass.tks
            .binary_search_by_key(&at, |k| k.tok.text_range().start())
            .ok()
    }

    /// The shift of the closest statement that holds the token and starts on an earlier line.
    fn inherited(&self, pass: &Pass<'_, '_>, i: usize) -> isize {
        let line = pass.tks[i].first_line;
        std::iter::successors(pass.tks[i].tok.parent(), |n| n.parent())
            .filter(|n| {
                n.parent()
                    .is_some_and(|p| self.statements.contains(&p.kind()))
            })
            .filter_map(|n| pass.cx.first_token(&n))
            .filter_map(|t| self.index(pass, &t))
            .map(|j| pass.tks[j].first_line)
            .find(|l| *l < line)
            .and_then(|l| pass.moved.get(&l).copied())
            .unwrap_or(0)
    }

    fn is_frame(&self, k: SyntaxKind) -> bool {
        self.frames.contains(&k) || self.keyword_frames.contains(&k) || self.branches.contains(&k)
    }

    /// Where the line that starts with token `i` has to stand, when a frame decides it.
    fn target(&self, pass: &Pass<'_, '_>, i: usize) -> Option<usize> {
        let tok = &pass.tks[i].tok;
        let parent = tok.parent()?;
        let lead = pass.leads.of(i);
        // The second operand of an operation in the condition of `if`: under the first operand.
        if let Some(t) = lead
            .outermost(&self.operations)
            .and_then(|n| self.operand(pass, n))
        {
            return Some(t);
        }
        // The dot that starts a line of a call chain.
        if self.leaders.contains(&tok.kind()) && self.chains.contains(&parent.kind()) {
            return self.chain(pass, &parent, tok);
        }
        // The closing word of a frame.
        if self.closers.contains(&tok.text())
            && self.is_frame(parent.kind())
            && pass.cx.last_token(&parent).as_ref() == Some(tok)
        {
            return self.base(pass, &parent);
        }
        // The keyword of a branch: `else`, `rescue`, `when`, `elsif`.
        let branch = self.branches.contains(&parent.kind())
            || (self.keyword_frames.contains(&parent.kind())
                && parent
                    .parent()
                    .is_some_and(|p| self.keyword_frames.contains(&p.kind())));
        if branch && !lead.nodes.is_empty() {
            let owner = self.owner(pass, &parent)?;
            return self.base(pass, &owner);
        }
        // The first token of a statement of a body.
        let stmt = lead.outermost(&self.statements)?;
        let owner = stmt.parent()?.parent()?;
        if !self.is_frame(owner.kind()) {
            return None;
        }
        let key = (owner.text_range(), owner.kind());
        let known: Option<Option<usize>> = None;
        let _ = (pass.bodies.borrow().iter())
            .find(|(r, k, _)| (*r, *k) == key)
            .map(|b| b.2);
        if let Some(at) = known {
            return at;
        }
        let at = self.body(pass, owner);
        pass.bodies.borrow_mut().push((key.0, key.1, at));
        at
    }

    /// Where the statements of the body of a frame stand.
    fn body(&self, pass: &Pass<'_, '_>, owner: SyntaxNode) -> Option<usize> {
        // A modifier form (`x if y`) has no body; the body of a bare frame belongs to the frame above.
        let owner = if self.starts_with_body(pass, &owner) {
            if !self.frames.contains(&owner.kind()) {
                return None;
            }
            self.owner(pass, &owner)?
        } else {
            owner
        };
        Some(self.base(pass, &owner)? + pass.width)
    }

    /// Does the node start with its body, with no keyword in front?
    fn starts_with_body(&self, pass: &Pass<'_, '_>, n: &SyntaxNode) -> bool {
        n.children_with_tokens()
            .find(|e| match e {
                rowan::NodeOrToken::Token(t) => !pass.cx.grammar.is_trivia(t.kind()),
                rowan::NodeOrToken::Node(c) => !pass.cx.grammar.is_trivia(c.kind()),
            })
            .is_some_and(|e| matches!(&e, rowan::NodeOrToken::Node(c) if self.statements.contains(&c.kind())))
    }

    /// The closest frame above the node that has a keyword.
    fn owner(&self, pass: &Pass<'_, '_>, n: &SyntaxNode) -> Option<SyntaxNode> {
        n.ancestors()
            .skip(1)
            .find(|a| self.is_frame(a.kind()) && !self.starts_with_body(pass, a))
    }

    /// Where a line that starts with the second operand of an operation in a condition stands
    /// (`operand`: the node that starts it, whose parent is an operation).
    fn operand(&self, pass: &Pass<'_, '_>, operand: &SyntaxNode) -> Option<usize> {
        let first = |n: &SyntaxNode| pass.cx.first_token(n);
        // Not the first operand.
        operand.prev_sibling()?;
        let top = std::iter::successors(operand.parent(), |n| n.parent())
            .take_while(|n| self.operations.contains(&n.kind()))
            .last()?;
        let owner = top.parent()?;
        if !self.conditions.contains(&owner.kind()) {
            return None;
        }
        let start = self.index(pass, &first(&top)?)?;
        let moved = pass
            .moved
            .get(&pass.tks[start].first_line)
            .copied()
            .unwrap_or(0);
        Some((self.column(&pass.tks[start].tok) as isize + moved).max(0) as usize)
    }

    /// The receiver of a call: the node in front of its dot.
    fn receiver(&self, call: &SyntaxNode) -> Option<SyntaxNode> {
        let dot = call
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| self.leaders.contains(&t.kind()))?;
        std::iter::successors(dot.prev_sibling_or_token(), |e| e.prev_sibling_or_token())
            .find_map(|e| e.into_node())
    }

    /// Where a dot that starts a line stands: under the first dot of the chain when that is on the
    /// line the chain starts on, else under the start of the chain.
    fn chain(&self, pass: &Pass<'_, '_>, call: &SyntaxNode, dot: &SyntaxToken) -> Option<usize> {
        // A chain inside the arguments or elements of a call or literal is not moved.
        let top = std::iter::successors(Some(call.clone()), |n| n.parent())
            .take_while(|n| !self.pairs.contains(&n.kind()))
            .take_while(|n| {
                !n.parent()
                    .is_some_and(|p| self.statements.contains(&p.kind()))
            });
        if top.into_iter().any(|n| self.opaque.contains(&n.kind())) {
            return None;
        }
        let mut first = call.clone();
        let mut root = call.clone();
        while let Some(object) = self.receiver(&root) {
            if !self.chains.contains(&object.kind()) {
                root = object;
                break;
            }
            if self.receiver(&object).is_some() {
                first = object.clone();
            }
            root = object;
        }
        let start = self.index(pass, &pass.cx.first_token(&root)?)?;
        let line = pass.tks[start].first_line;
        let first_dot = first
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| self.leaders.contains(&t.kind()))?;
        let at = self.index(pass, &first_dot)?;
        if &first_dot != dot && pass.tks[at].first_line == line {
            // The dot of the first call stands on the line the chain starts on.
            let moved = pass.moved.get(&line).copied().unwrap_or(0);
            return Some((self.column(&first_dot) as isize + moved).max(0) as usize);
        }
        // A chain that starts in the middle of a line (`x = foo`) stands under its start.
        if !pass.tks[start].newline && start != 0 {
            let moved = pass.moved.get(&line).copied().unwrap_or(0);
            return Some((self.column(&pass.tks[start].tok) as isize + moved).max(0) as usize);
        }
        // Else one step in from the line of the statement or pair the chain is a part of.
        let holder = call.ancestors().find(|n| {
            self.pairs.contains(&n.kind())
                || n.parent()
                    .is_some_and(|p| self.statements.contains(&p.kind()))
        })?;
        let at = self.index(pass, &pass.cx.first_token(&holder)?)?;
        Some(pass.indent.get(&pass.tks[at].first_line).copied()? + pass.width)
    }

    /// The column of a token in its source line.
    fn column(&self, t: &SyntaxToken) -> usize {
        let mut col = 0;
        let mut prev = t.prev_token();
        while let Some(p) = prev {
            match p.text().rfind('\n') {
                Some(i) => return col + p.text().len() - i - 1,
                None => col += p.text().len(),
            }
            prev = p.prev_token();
        }
        col
    }

    /// The indentation of the line a frame opens on.
    fn base(&self, pass: &Pass<'_, '_>, frame: &SyntaxNode) -> Option<usize> {
        let line = if self.frames.contains(&frame.kind()) {
            // The line the frame starts on, a block with its call.
            let mut start = frame.clone();
            while self.attached.contains(&start.kind()) {
                start = start.parent()?;
            }
            // A call that continues on a line of its own starts at its dot.
            let dot = start
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| self.leaders.contains(&t.kind()))
                .last()
                .and_then(|t| self.index(pass, &t))
                .filter(|j| pass.tks[*j].newline);
            match dot {
                Some(j) => pass.tks[j].first_line,
                None => pass.tks[self.index(pass, &pass.cx.first_token(&start)?)?].first_line,
            }
        } else {
            // The keyword must start its line.
            let j = self.index(pass, &pass.cx.first_token(frame)?)?;
            (j == 0 || pass.tks[j].newline).then_some(pass.tks[j].first_line)?
        };
        pass.indent.get(&line).copied()
    }
}

/// The body with its closing label of one heredoc: see
/// [`Indents::heredocs`].
fn reindent(
    opener: &str,
    body: &str,
    (base, outer): (usize, Option<usize>),
    width: usize,
) -> Option<String> {
    let squiggly = opener.starts_with("<<~");
    if body.contains(['\r', '\t']) {
        return None;
    }
    let (text, label) = body.rsplit_once('\n')?;
    let blank = |l: &str| l.trim().is_empty();
    let lead = |l: &str| l.len() - l.trim_start().len();
    let want = base + width;
    let min = text.split('\n').filter(|l| !blank(l)).map(lead).min();
    let mut closing = lead(label);
    let mut out = text.to_string();
    if let Some(min) = min
        && squiggly
        && min != want
    {
        out = text
            .split('\n')
            .map(|l| match () {
                // A line of blanks no longer than the indentation is empty in the value.
                _ if blank(l) && l.len() <= min => String::new(),
                _ => format!("{}{}", " ".repeat(want), &l[min..]),
            })
            .collect::<Vec<_>>()
            .join("\n");
        closing = closing.max(base);
    }
    let dashed = squiggly || opener.starts_with("<<-");
    if dashed && closing != base && outer != Some(closing) {
        closing = base;
    }
    let label = format!(
        "{}{}",
        " ".repeat(if dashed { closing } else { lead(label) }),
        label.trim_start()
    );
    Some(format!("{out}\n{label}"))
}
