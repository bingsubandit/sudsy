//! The flow graph of a body built from statement roles (`#block`, `#conditional`, `#loop`,
//! `#switch`, `#case`, `#try`, `#catch`, `#return`, `#throw`, `#break`, `#continue`, `#labeled`),
//! so a language gets reachability from its grammar's roles alone. Every statement the walk
//! passes becomes one operation (its source span) in the block it runs in; a statement that no
//! operation of a reachable block names is unreachable. A loop with no test or a literal `true`
//! test never ends by itself, a `finally` block is built once per way of entering it, and the
//! calls, accesses and `throw`s of a `try` body lead to its handlers.

use crate::graph::{Block, BodyGraph, Edge, Op};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, code_span};
use rowan::WalkEvent;

/// The graph of a body or statement, and where the answers to the common questions sit in it.
pub struct Flow {
    pub graph: BodyGraph,
    /// The block control is in after the last statement: reachable when the body can run off its end.
    pub end: usize,
    /// Per `case` of a `switch` built: the start of its code and the block control is in after its statements.
    pub case_ends: Vec<(u32, usize)>,
}

/// What the graph says can run.
pub struct Reached {
    /// Per block.
    pub block: Vec<bool>,
    /// Starts of the statements none of whose operations can run, sorted.
    dead: Vec<u32>,
}

impl Reached {
    /// Is the statement starting at `start` (its first code byte) built, and can it never run?
    pub fn is_dead(&self, start: u32) -> bool {
        self.dead.binary_search(&start).is_ok()
    }
}

impl Flow {
    pub fn reach(&self) -> Reached {
        let blocks = &self.graph.blocks;
        let mut block = vec![false; blocks.len()];
        let mut stack = vec![0usize];
        block[0] = true;
        while let Some(b) = stack.pop() {
            for e in blocks[b].succ.iter().filter(|e| blocks[b].feasible(e)) {
                let to = e.to as usize;
                if !block[to] {
                    block[to] = true;
                    stack.push(to);
                }
            }
        }
        let starts = |live: bool| {
            let mut v: Vec<u32> = (0..blocks.len())
                .filter(|&i| block[i] == live)
                .flat_map(|i| blocks[i].ops.iter().map(|o| o.start))
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let live = starts(true);
        let mut dead = starts(false);
        dead.retain(|s| live.binary_search(s).is_err());
        Reached { block, dead }
    }
}

/// Builds the graph of `root`: a function (its body), or any statement (a block, a `switch`, the
/// file). `None` for a function without a body. Control that leaves `root` (`return`, a `break` to
/// an outer loop, a throw nothing catches) ends the path.
pub fn build(grammar: &Grammar, text: &str, root: &SyntaxNode) -> Option<Flow> {
    let mut b = Builder {
        g: grammar,
        text,
        blocks: vec![Block::default()],
        cur: 0,
        frames: Vec::new(),
        guards: 0,
        case_ends: Vec::new(),
    };
    if role(grammar, root) == Role::Function {
        b.stmt(&grammar.slot(root, "body")?.into_node()?);
    } else {
        b.stmt(root);
    }
    Some(Flow {
        end: b.cur,
        case_ends: b.case_ends,
        graph: BodyGraph {
            blocks: b.blocks,
            ..BodyGraph::default()
        },
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Block,
    Conditional,
    Loop,
    Switch,
    Try,
    Return,
    Throw,
    Break,
    Continue,
    Labeled,
    Function,
    /// Reads or calls something that can fail.
    Fallible,
    Other,
}

fn role(g: &Grammar, n: &SyntaxNode) -> Role {
    let Some(def) = g.def(n.kind()) else {
        return Role::Other;
    };
    def.tags
        .iter()
        .find_map(|t| {
            Some(match &**t {
                "block" => Role::Block,
                "conditional" => Role::Conditional,
                "loop" => Role::Loop,
                "switch" => Role::Switch,
                "try" => Role::Try,
                "return" => Role::Return,
                "throw" => Role::Throw,
                "break" => Role::Break,
                "continue" => Role::Continue,
                "labeled" => Role::Labeled,
                "function" => Role::Function,
                "call" | "member-access" | "index" | "reference" => Role::Fallible,
                _ => return None,
            })
        })
        .unwrap_or(Role::Other)
}

/// Where a jump goes: a frame (by depth), out of the function, or to the nearest handler.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Break(usize),
    Continue(usize),
    Return,
    Throw,
}

enum Frame<'t> {
    Loop {
        brk: usize,
        cont: usize,
    },
    Switch {
        brk: usize,
    },
    Label {
        name: &'t str,
        brk: usize,
    },
    Catch(Vec<usize>),
    /// Jumps that cross a `finally`, with the block they leave from.
    Finally(Vec<(Target, usize)>),
}

struct Builder<'a> {
    g: &'a Grammar,
    text: &'a str,
    blocks: Vec<Block>,
    cur: usize,
    frames: Vec<Frame<'a>>,
    /// `Catch` and `Finally` frames open: only then can a statement lead to a handler.
    guards: usize,
    case_ends: Vec<(u32, usize)>,
}

fn constant_true(text: &str) -> bool {
    let t = text.trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace());
    matches!(t, "true" | "True" | "1")
}

impl<'a> Builder<'a> {
    fn new_block(&mut self) -> usize {
        self.blocks.push(Block::default());
        self.blocks.len() - 1
    }

    fn edge(&mut self, from: usize, to: usize, kind: u8) {
        self.blocks[from].succ.push(Edge {
            to: to as u32,
            kind,
        });
    }

    fn always(&mut self, from: usize, to: usize) {
        self.edge(from, to, 0);
    }

    /// The block after paths that end in `froms`.
    fn join(&mut self, froms: &[usize]) -> usize {
        if let [one] = froms {
            return *one;
        }
        let to = self.new_block();
        for &f in froms {
            self.always(f, to);
        }
        to
    }

    fn node(&self, n: &SyntaxNode, label: &str) -> Option<SyntaxNode> {
        self.g.slot(n, label)?.into_node()
    }

    fn nodes(&self, n: &SyntaxNode, label: &str) -> Vec<SyntaxNode> {
        self.g
            .slots(n, label)
            .filter_map(SyntaxElement::into_node)
            .collect()
    }

    fn text_of(&self, n: &SyntaxNode) -> &'a str {
        code_span(n, self.g).map_or("", |r| &self.text[r])
    }

    /// One operation for the statement `n`.
    fn emit(&mut self, n: &SyntaxNode) {
        let Some(r) = code_span(n, self.g) else {
            return;
        };
        let (start, len) = (u32::from(r.start()), u32::from(r.len()));
        self.blocks[self.cur].ops.push(Op {
            kind: 5, // OpKind::Other
            local: -1,
            start,
            len,
            stmt_start: start,
            stmt_len: len,
            ..Op::default()
        });
    }

    /// Can `n` fail (outside nested functions)?
    fn fallible(&self, n: &SyntaxNode) -> bool {
        let mut walk = n.preorder();
        while let Some(event) = walk.next() {
            if let WalkEvent::Enter(c) = event {
                match role(self.g, &c) {
                    Role::Function => walk.skip_subtree(),
                    Role::Fallible | Role::Throw => return true,
                    _ => {}
                }
            }
        }
        false
    }

    /// The expressions of a statement: when one can fail, a handler can take over from here.
    fn parts(&mut self, parts: &[Option<SyntaxNode>]) {
        if self.guards > 0 && parts.iter().flatten().any(|p| self.fallible(p)) {
            self.go(self.cur, Target::Throw);
        }
    }

    /// Routes a jump from `from` to its target through the `finally` blocks it crosses.
    fn go(&mut self, from: usize, t: Target) {
        let floor = match t {
            Target::Break(d) | Target::Continue(d) => d,
            _ => 0,
        };
        for i in (floor..self.frames.len()).rev() {
            let handlers = match &mut self.frames[i] {
                Frame::Finally(pending) => {
                    pending.push((t, from));
                    return;
                }
                Frame::Catch(entries) if t == Target::Throw => entries.clone(),
                _ => continue,
            };
            for e in handlers {
                self.edge(from, e, 3);
            }
            return;
        }
        let to = match (t, self.frames.get(floor)) {
            (
                Target::Break(_),
                Some(Frame::Loop { brk, .. } | Frame::Switch { brk } | Frame::Label { brk, .. }),
            ) => *brk,
            (Target::Continue(_), Some(Frame::Loop { cont, .. })) => *cont,
            _ => return,
        };
        self.always(from, to);
    }

    /// A jump: nothing runs after it in this block.
    fn jump(&mut self, t: Option<Target>) {
        if let Some(t) = t {
            self.go(self.cur, t);
        }
        self.cur = self.new_block();
    }

    /// The frame a `break` or `continue` (with `label`) addresses.
    fn find(&self, label: Option<&str>, is_break: bool) -> Option<Target> {
        let at = |d: usize| {
            if is_break {
                Target::Break(d)
            } else {
                Target::Continue(d)
            }
        };
        self.frames
            .iter()
            .enumerate()
            .rev()
            .find_map(|(d, f)| match (f, label) {
                (Frame::Loop { .. }, None) => Some(at(d)),
                (Frame::Switch { .. }, None) if is_break => Some(at(d)),
                (Frame::Label { name, .. }, Some(l)) if *name == l => {
                    Some(at(if is_break { d } else { d + 1 }))
                }
                _ => None,
            })
    }

    fn stmt(&mut self, n: &SyntaxNode) {
        self.emit(n);
        self.walk(n);
    }

    fn walk(&mut self, n: &SyntaxNode) {
        match role(self.g, n) {
            Role::Block => {
                for s in self.nodes(n, "items") {
                    self.stmt(&s);
                }
            }
            Role::Conditional => self.conditional(n),
            Role::Loop => self.looping(n),
            Role::Switch => self.switch(n),
            Role::Try => self.try_stmt(n),
            Role::Return => {
                let v = self.node(n, "value");
                self.parts(&[v]);
                self.jump(Some(Target::Return));
            }
            Role::Throw => {
                if self.guards > 0 {
                    self.go(self.cur, Target::Throw);
                }
                self.jump(None);
            }
            r @ (Role::Break | Role::Continue) => {
                let label = self.node(n, "label").map(|l| self.text_of(&l));
                let t = self.find(label, r == Role::Break);
                self.jump(t);
            }
            Role::Labeled => {
                let name = self.node(n, "label").map_or("", |l| self.text_of(&l));
                let after = self.new_block();
                self.frames.push(Frame::Label { name, brk: after });
                if let Some(body) = self.node(n, "body") {
                    self.stmt(&body);
                }
                self.frames.pop();
                self.always(self.cur, after);
                self.cur = after;
            }
            Role::Function | Role::Fallible | Role::Other => self.parts(&[Some(n.clone())]),
        }
    }

    fn conditional(&mut self, n: &SyntaxNode) {
        let then = self.node(n, "then");
        // an `else` clause has no condition: its body runs
        let Some(cond) = self.node(n, "condition") else {
            if let Some(t) = then {
                self.stmt(&t);
            }
            return;
        };
        self.parts(&[Some(cond)]);
        let (head, then_b, else_b) = (self.cur, self.new_block(), self.new_block());
        self.edge(head, then_b, 1);
        self.edge(head, else_b, 2);
        self.cur = then_b;
        if let Some(t) = then {
            self.stmt(&t);
        }
        let then_end = self.cur;
        self.cur = else_b;
        if let Some(e) = self.node(n, "else") {
            self.stmt(&e);
        }
        self.cur = self.join(&[then_end, self.cur]);
    }

    fn looping(&mut self, n: &SyntaxNode) {
        let (cond, iter) = (self.node(n, "condition"), self.node(n, "iterable"));
        let (init, update) = (self.node(n, "init"), self.node(n, "update"));
        let Some(body) = self.node(n, "body") else {
            return;
        };
        let test_last = cond
            .as_ref()
            .is_some_and(|c| c.text_range().start() >= body.text_range().end());
        let endless = match &cond {
            Some(c) => constant_true(self.text_of(c)),
            None => iter.is_none(),
        };
        let after = self.new_block();
        self.parts(&[init]);
        let top = self.new_block();
        self.always(self.cur, top);
        self.cur = top;
        if test_last {
            // `do ... while`: the body first, the test after it
            let test = self.new_block();
            self.frames.push(Frame::Loop {
                brk: after,
                cont: test,
            });
            self.stmt(&body);
            self.frames.pop();
            self.always(self.cur, test);
            self.cur = test;
            self.parts(&[cond]);
            self.edge(test, top, 1);
            self.edge(test, after, 2);
            self.blocks[test].branch_const = endless.then_some(true);
        } else {
            self.parts(&[cond, iter]);
            let body_b = self.new_block();
            self.edge(top, body_b, 1);
            self.edge(top, after, 2);
            self.blocks[top].branch_const = endless.then_some(true);
            let step = update.is_some().then(|| self.new_block());
            self.cur = body_b;
            self.frames.push(Frame::Loop {
                brk: after,
                cont: step.unwrap_or(top),
            });
            self.stmt(&body);
            self.frames.pop();
            if let Some(step) = step {
                self.always(self.cur, step);
                self.cur = step;
                self.parts(&[update]);
            }
            self.always(self.cur, top);
        }
        self.cur = after;
    }

    fn switch(&mut self, n: &SyntaxNode) {
        let subject = self.node(n, "subject");
        self.parts(&[subject]);
        let (dispatch, after) = (self.cur, self.new_block());
        self.frames.push(Frame::Switch { brk: after });
        let (mut default, mut prev) = (false, None);
        for case in self.nodes(n, "cases") {
            let entry = self.new_block();
            self.always(dispatch, entry);
            if let Some(p) = prev {
                self.always(p, entry);
            }
            self.cur = entry;
            default |= self.g.slot(&case, "test").is_none();
            for s in self.nodes(&case, "body") {
                self.stmt(&s);
            }
            if let Some(r) = code_span(&case, self.g) {
                self.case_ends.push((u32::from(r.start()), self.cur));
            }
            prev = Some(self.cur);
        }
        self.frames.pop();
        if !default {
            self.always(dispatch, after);
        }
        if let Some(p) = prev {
            self.always(p, after);
        }
        self.cur = after;
    }

    fn try_stmt(&mut self, n: &SyntaxNode) {
        let catches = self.nodes(n, "catches");
        let finalizer = self.node(n, "finally");
        let entries: Vec<usize> = catches.iter().map(|_| self.new_block()).collect();
        let (has_finally, has_catch) = (finalizer.is_some(), !entries.is_empty());
        if has_finally {
            self.frames.push(Frame::Finally(Vec::new()));
        }
        if has_catch {
            self.frames.push(Frame::Catch(entries.clone()));
        }
        self.guards += usize::from(has_finally) + usize::from(has_catch);
        let start = self.cur;
        if let Some(body) = self.node(n, "body") {
            self.stmt(&body);
        }
        if has_catch {
            self.frames.pop();
            self.guards -= 1;
            // a body that runs to its end may have failed on the way
            for &e in &entries {
                self.edge(self.cur, e, 3);
            }
        }
        let mut normal = vec![self.cur];
        for (c, entry) in catches.iter().zip(entries) {
            // the handler's own statements run whether or not a throw reaches it (what follows the
            // `try` stays reachable); only its block, built in the entry, can be unreachable
            self.cur = entry;
            let body = self.node(c, "body");
            if let Some(body) = &body {
                self.emit(body);
            }
            self.cur = self.new_block();
            self.always(start, self.cur);
            if let Some(body) = &body {
                self.walk(body);
            }
            normal.push(self.cur);
        }
        let Some(finalizer) = finalizer else {
            self.cur = self.join(&normal);
            return;
        };
        self.guards -= 1;
        let Some(Frame::Finally(pending)) = self.frames.pop() else {
            return;
        };
        // one copy of the `finally` block for the way in from the end of the `try` and its
        // handlers, and one for each target a jump out of them goes to
        let mut ways: Vec<(Option<Target>, Vec<usize>)> = vec![(None, normal)];
        for (t, from) in pending {
            match ways.iter_mut().find(|w| w.0 == Some(t)) {
                Some(w) => w.1.push(from),
                None => ways.push((Some(t), vec![from])),
            }
        }
        let mut after = 0;
        for (target, froms) in ways {
            self.cur = self.new_block();
            for from in froms {
                self.always(from, self.cur);
            }
            self.stmt(&finalizer);
            match target {
                Some(t) => self.go(self.cur, t),
                None => after = self.cur,
            }
        }
        self.cur = after;
    }
}
