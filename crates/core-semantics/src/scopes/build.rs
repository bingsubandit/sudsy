//! The binder: one pre-order walk that opens scopes, declares names and records references as the
//! language's [`Table`] says, then resolves every reference by name up the scope chain.

use super::table::{Declares, Flow, Place, Row, Sel, Table};
use super::{Access, Decl, Flags, Model, RefId, Reference, Scope, ScopeId, Symbol, SymbolId};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange, TextSize};
use rowan::{GreenNodeData, GreenTokenData, NodeOrToken, WalkEvent};
use std::collections::HashSet;
use std::rc::Rc;

/// What a node inherits from its parent about the references it holds: the access they have,
/// none for a plain read.
type State = Access;

struct Frame {
    /// The state of each child node, when the row says anything about them: `len` entries of
    /// the binder's arena from `start`.
    start: usize,
    len: usize,
    next: usize,
    scope: bool,
    decl: bool,
    params: bool,
    inert: bool,
    /// The scope this node is outside of, put back when it is left.
    hidden: Option<ScopeId>,
}

struct Ctx {
    flags: Flags,
    scope: ScopeId,
    node: SyntaxNode,
}

struct Binder<'a> {
    grammar: &'a Grammar,
    table: &'a Table,
    model: Model,
    scopes: Vec<ScopeId>,
    ctxs: Vec<Ctx>,
    params: Vec<ScopeId>,
    /// How many enclosing nodes are inert: names under them are no references.
    inert: u32,
    arena: Vec<Option<(State, bool)>>,
    /// Every name once: symbols, scopes and references share it.
    names: HashSet<Rc<str>>,
}

/// The model of the tree under `root`.
pub fn bind(root: &SyntaxNode, grammar: &Grammar, table: &Table) -> Model {
    let mut b = Binder {
        grammar,
        table,
        model: Model::default(),
        scopes: Vec::new(),
        ctxs: Vec::new(),
        params: Vec::new(),
        inert: 0,
        arena: Vec::new(),
        names: HashSet::new(),
    };
    let mut frames: Vec<Frame> = Vec::new();
    for event in root.preorder() {
        match event {
            WalkEvent::Enter(node) => {
                let (state, outside) = match frames.last_mut() {
                    Some(parent) => {
                        let i = parent.next;
                        parent.next += 1;
                        let hit = (i < parent.len).then(|| b.arena[parent.start + i]);
                        hit.flatten().unwrap_or_default()
                    }
                    None => Default::default(),
                };
                let frame = b.enter(&node, state, outside);
                frames.push(frame);
            }
            WalkEvent::Leave(_) => {
                if let Some(frame) = frames.pop() {
                    b.arena.truncate(frame.start);
                    if frame.decl {
                        b.ctxs.pop();
                    }
                    if frame.scope {
                        b.scopes.pop();
                    }
                    if frame.params {
                        b.params.pop();
                    }
                    if frame.inert {
                        b.inert -= 1;
                    }
                    b.scopes.extend(frame.hidden);
                }
            }
        }
    }
    b.resolve();
    b.model
}

impl Binder<'_> {
    fn enter(&mut self, node: &SyntaxNode, state: State, outside: bool) -> Frame {
        let kind = node.kind();
        let row = self.table.row(kind);
        let mut frame = Frame {
            start: self.arena.len(),
            len: 0,
            next: 0,
            scope: false,
            decl: false,
            params: false,
            inert: false,
            hidden: if outside { self.scopes.pop() } else { None },
        };
        let outer = self.scopes.last().copied();
        if self.table.is_params(kind) {
            self.params.push(outer.unwrap_or(ScopeId(0)));
            frame.params = true;
        }
        if self.table.is_scope(kind) {
            let id = ScopeId(self.model.scopes.len() as u32);
            let var = row.is_some_and(|r| r.var_scope);
            let closed = row.is_some_and(|r| r.closed);
            self.model
                .scopes
                .push(Scope::new(outer, node.clone(), var, closed));
            self.scopes.push(id);
            frame.scope = true;
        }
        let Some(row) = row else {
            self.tokens(node, None, state);
            return frame;
        };
        if row.inert {
            self.inert += 1;
            frame.inert = true;
        }
        if row.dynamic {
            self.mark_dynamic();
        }
        if let Some(d) = &row.declares {
            self.declares(node, d, frame.scope, outer);
            frame.decl = true;
        }
        if row.binding {
            self.binding(node);
        }
        if row.reference {
            self.reference(node, state);
        }
        self.tokens(node, Some(row), state);
        if !row.children.is_empty() {
            self.flows(node, row, state);
            frame.len = self.arena.len() - frame.start;
        }
        frame
    }

    fn current(&self) -> ScopeId {
        self.scopes.last().copied().unwrap_or(ScopeId(0))
    }

    /// The first token under `node` that is code, when its kind is one of the table's names: the
    /// name (interned, quotes trimmed when `trim`) and where it was written. Read off the green
    /// tree, which allocates nothing.
    fn name_token(&mut self, node: &SyntaxNode, trim: bool) -> Option<(Rc<str>, TextRange)> {
        let green = node.green();
        let (kind, text, at) = self.first_code(&green, node.text_range().start())?;
        if !self.table.is_name_token(kind) {
            return None;
        }
        let name = if trim {
            text.trim_matches(['"', '\''])
        } else {
            text
        };
        let range = TextRange::at(at, TextSize::of(text));
        Some((self.intern(name), range))
    }

    fn intern(&mut self, name: &str) -> Rc<str> {
        match self.names.get(name) {
            Some(n) => n.clone(),
            None => {
                let n: Rc<str> = name.into();
                self.names.insert(n.clone());
                n
            }
        }
    }

    fn mark_dynamic(&mut self) {
        let scope = self.model.var_scope(self.current());
        self.model.scopes[scope.0 as usize].dynamic = true;
    }

    /// The last token under `node` that is code, when its kind is one of the table's names.
    fn last_name(&mut self, node: &SyntaxNode) -> Option<(Rc<str>, TextRange)> {
        let green = node.green();
        let (kind, text, at) = self.last_code(&green, node.text_range().end())?;
        if !self.table.is_name_token(kind) {
            return None;
        }
        Some((self.intern(text), TextRange::at(at, TextSize::of(text))))
    }

    fn last_code<'g>(
        &self,
        node: &'g GreenNodeData,
        end: TextSize,
    ) -> Option<(SyntaxKind, &'g str, TextSize)> {
        let mut at = end;
        for child in node.children().rev() {
            let len = child.text_len();
            at -= len;
            match child {
                NodeOrToken::Token(t) => {
                    let kind = SyntaxKind(t.kind().0);
                    if len > TextSize::new(0) && !self.grammar.is_trivia(kind) {
                        return Some((kind, t.text(), at));
                    }
                }
                NodeOrToken::Node(n) => {
                    if let Some(found) = self.last_code(n, at + len) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    fn first_code<'g>(
        &self,
        node: &'g GreenNodeData,
        start: TextSize,
    ) -> Option<(SyntaxKind, &'g str, TextSize)> {
        let mut at = start;
        for child in node.children() {
            let len = child.text_len();
            match child {
                NodeOrToken::Token(t) => {
                    let kind = SyntaxKind(t.kind().0);
                    if len > TextSize::new(0) && !self.grammar.is_trivia(kind) {
                        return Some((kind, t.text(), at));
                    }
                }
                NodeOrToken::Node(n) => {
                    if let Some(found) = self.first_code(n, at) {
                        return Some(found);
                    }
                }
            }
            at += len;
        }
        None
    }

    fn declares(
        &mut self,
        node: &SyntaxNode,
        d: &Declares,
        own_scope: bool,
        outer: Option<ScopeId>,
    ) {
        let (mut flags, mut place) = (d.flags, d.place);
        if let Some(parent) = node.parent()
            && let Some((_, p)) = d
                .parent_places
                .iter()
                .find(|(k, _)| *k == self.grammar.name(parent.kind()))
        {
            place = *p;
        }
        if let Some(parent) = node.parent()
            && let Some((_, f)) = d
                .parent_flags
                .iter()
                .find(|(k, _)| *k == self.grammar.name(parent.kind()))
        {
            flags = *f;
        }
        if let Some((extra, p)) = d.expression
            && !node.parent().is_some_and(|p| self.table.is_host(p.kind()))
        {
            flags = flags | extra;
            place = p;
        }
        if !d.keywords.is_empty() {
            let from = if d.keyword_on_parent {
                node.parent()
            } else {
                Some(node.clone())
            };
            let hit = from.and_then(|n| {
                own_parts(&n.green(), n.text_range().start())
                    .find_map(|(c, _)| match c {
                        NodeOrToken::Token(t) => d.keywords.iter().find(|(k, ..)| *k == t.text()),
                        NodeOrToken::Node(_) => None,
                    })
                    .copied()
            });
            if let Some((_, f, p)) = hit {
                flags = f;
                place = p;
            }
        }
        let current = self.current();
        let scope = match place {
            Place::Scope => current,
            Place::Parent if own_scope => outer.unwrap_or(ScopeId(0)),
            Place::Parent => current,
            Place::Var => self.model.var_scope(current),
            Place::Enclosing(kind) => self
                .scopes
                .iter()
                .rev()
                .copied()
                .find(|s| self.grammar.name(self.model.scope(*s).node.kind()) == kind)
                .unwrap_or(current),
        };
        self.ctxs.push(Ctx {
            flags,
            scope,
            node: node.clone(),
        });
        if d.name_child
            && let Some(child) = node.children().next()
            && let Some((name, range)) = self.name_token(&child, true)
        {
            self.declare(name, range, &child);
        }
        if d.name_last
            && let Some(child) = node.children().last()
            && (d.skip_token.is_empty()
                || !own_parts(&node.green(), node.text_range().start()).any(|(c, _)| {
                    matches!(c, NodeOrToken::Token(t) if t.text() == d.skip_token)
                }))
            && let Some((name, range)) = self.last_name(&child)
        {
            self.declare(name, range, &child);
        }
    }

    fn binding(&mut self, node: &SyntaxNode) {
        if self.ctxs.is_empty() {
            return;
        }
        if let Some((name, range)) = self.name_token(node, false) {
            self.declare(name, range, node);
        }
    }

    fn declare(&mut self, name: Rc<str>, range: TextRange, binding: &SyntaxNode) {
        let Some(ctx) = self.ctxs.last() else { return };
        let decl = Decl {
            name: range,
            node_range: ctx.node.text_range(),
            node_kind: ctx.node.kind(),
            binding_range: binding.text_range(),
            binding_kind: binding.kind(),
            flags: ctx.flags,
        };
        let scope = ctx.scope;
        self.add_decl(scope, name, decl);
    }

    fn add_decl(&mut self, scope: ScopeId, name: Rc<str>, decl: Decl) {
        let flags = decl.flags;
        let scope_ref = &mut self.model.scopes[scope.0 as usize];
        // The name of a function expression is a variable of its own, between the function's scope
        // and the one around it: a declaration of the same name in the function is another one.
        if let Some(&id) = scope_ref.names_mut().get(&*name)
            && (flags.contains(Flags::EXPRESSION_NAME)
                || !self.model.symbols[id.0 as usize]
                    .flags
                    .contains(Flags::EXPRESSION_NAME))
        {
            let s = &mut self.model.symbols[id.0 as usize];
            s.flags = s.flags | flags;
            s.decls.push(decl);
            return;
        }
        let scope_ref = &mut self.model.scopes[scope.0 as usize];
        let id = SymbolId(self.model.symbols.len() as u32);
        scope_ref.names_mut().insert(name.clone(), id);
        scope_ref.symbols.push(id);
        self.model.symbols.push(Symbol {
            name,
            scope,
            flags,
            decls: vec![decl],
            refs: Vec::new(),
        });
    }

    fn reference(&mut self, node: &SyntaxNode, state: State) {
        if state.contains(Access::DECLARE) {
            return self.binding(node);
        }
        if self.inert > 0 {
            return;
        }
        let Some((name, range)) = self.name_token(node, false) else {
            return;
        };
        self.push_ref(node, name, range, state);
    }

    fn push_ref(&mut self, node: &SyntaxNode, name: Rc<str>, range: TextRange, state: State) {
        if self.table.is_dynamic_name(&name) {
            self.mark_dynamic();
        }
        let access = if state == Access::NONE {
            Access::READ
        } else {
            state
        };
        self.model.refs.push(Reference {
            name,
            node_range: node.text_range(),
            node_kind: node.kind(),
            range,
            access,
            scope: self.current(),
            in_params: self.params.clone(),
            symbol: None,
        });
    }

    /// What the own name tokens of `node` do: a row's `tokens` say, else (bare names) they are
    /// read, or take the state of the parent.
    fn tokens(&mut self, node: &SyntaxNode, row: Option<&Row>, state: State) {
        let uses = row.map_or(&[][..], |r| r.tokens);
        if uses.is_empty() && !self.table.is_bare() {
            return;
        }
        let table = self.table;
        let green = node.green();
        let base = node.text_range().start();
        let is_name = |c: &NodeOrToken<&GreenNodeData, &GreenTokenData>| match c {
            NodeOrToken::Token(t) => table.is_name_token(SyntaxKind(t.kind().0)),
            NodeOrToken::Node(_) => false,
        };
        let count = own_parts(&green, base).filter(|(c, _)| is_name(c)).count();
        if count == 0 {
            return;
        }
        let tokens = || {
            own_parts(&green, base).filter_map(|(c, r)| match c {
                NodeOrToken::Token(t) => Some((t, r)),
                NodeOrToken::Node(_) => None,
            })
        };
        let op = tokens().find(|(t, _)| table.is_operator(SyntaxKind(t.kind().0)));
        let op_start = op.map(|(_, r)| r.start());
        let op_text = op.map(|(t, _)| t.text());
        let text_at = |text: &str| tokens().find(|(t, _)| t.text() == text).map(|(_, r)| r);
        let mut index = 0;
        for (child, range) in own_parts(&green, base) {
            if !is_name(&child) {
                continue;
            }
            let NodeOrToken::Token(t) = child else { continue };
            let i = index;
            index += 1;
            let hit = uses.iter().find(|(sel, _)| match sel {
                Sel::All => true,
                Sel::First => i == 0,
                Sel::Last => i + 1 == count,
                Sel::LastBefore(text) => text_at(text).is_some_and(|stop| {
                    range.end() <= stop.start()
                        && !own_parts(&green, base).any(|(c, r)| {
                            is_name(&c) && r.start() > range.start() && r.end() <= stop.start()
                        })
                }),
                Sel::BeforeOp => op_start.is_none_or(|o| range.start() < o),
                Sel::AfterText(text) => {
                    text_at(text).is_some_and(|stop| stop.end() <= range.start())
                }
                Sel::WithToken(text) => text_at(text).is_some(),
                Sel::Field(_) | Sel::Kind(_) => false,
            });
            let access = match hit.map(|(_, f)| *f) {
                Some(Flow::Skip) => continue,
                Some(Flow::Declare) => {
                    let name = self.intern(t.text());
                    self.declare(name, range, node);
                    continue;
                }
                Some(flow) => access_of(self.grammar, flow, state, op_text),
                None => state,
            };
            if self.inert == 0 {
                let name = self.intern(t.text());
                self.push_ref(node, name, range, access);
            }
        }
    }

    /// The state of each child node of `node`, pushed on the arena. Read off the green tree, so
    /// the children and tokens of the node are not made.
    fn flows(&mut self, node: &SyntaxNode, row: &Row, state: State) {
        let fields: Vec<Option<TextRange>> = if row
            .children
            .iter()
            .any(|(sel, _)| matches!(sel, Sel::Field(_)))
        {
            row.children
                .iter()
                .map(|(sel, _)| match sel {
                    Sel::Field(f) => self.grammar.slot(node, f).map(|e| e.text_range()),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };
        let green = node.green();
        let base = node.text_range().start();
        let own_tokens = || {
            own_parts(&green, base).filter_map(|(c, r)| match c {
                NodeOrToken::Token(t) => Some((t, r)),
                NodeOrToken::Node(_) => None,
            })
        };
        let op = own_tokens().find(|(t, _)| self.table.is_operator(SyntaxKind(t.kind().0)));
        // An operator that is a node of its own (`operator:OperationReference`).
        let op_node = op.is_none().then(|| {
            let wanted = row.children.iter().any(|(sel, f)| {
                matches!(sel, Sel::BeforeOp) || matches!(f, Flow::Assigns | Flow::Assign(_))
            });
            wanted.then(|| self.grammar.slot(node, "operator")?.into_node()).flatten()
        });
        let op_green = op_node.as_ref().and_then(|n| n.as_ref()).map(|n| (n.green(), n.text_range()));
        let op_alt = op_green
            .as_ref()
            .and_then(|(g, r)| self.first_code(g, r.start()));
        let op_start = op.map(|(_, r)| r.start()).or(op_alt.map(|(_, _, at)| at));
        let op_text = op.map(|(t, _)| t.text()).or(op_alt.map(|(_, text, _)| text));
        let last = if row.children.iter().any(|(sel, _)| matches!(sel, Sel::Last)) {
            own_parts(&green, base)
                .filter(|(c, _)| matches!(c, NodeOrToken::Node(_)))
                .count()
        } else {
            0
        };
        let mut index = 0;
        for (child, range) in own_parts(&green, base) {
            if matches!(child, NodeOrToken::Token(_)) {
                continue;
            }
            let start = range.start();
            let hit = row
                .children
                .iter()
                .enumerate()
                .find(|(i, (sel, _))| match sel {
                    Sel::All => true,
                    Sel::First => index == 0,
                    Sel::Last => index + 1 == last,
                    Sel::LastBefore(_) => false,
                    Sel::Kind(name) => matches!(&child, NodeOrToken::Node(n)
                        if self.grammar.name(SyntaxKind(n.kind().0)) == *name),
                    Sel::Field(_) => fields[*i] == Some(range),
                    Sel::BeforeOp => op_start.is_none_or(|o| start < o),
                    Sel::WithToken(text) => own_tokens().any(|(t, _)| t.text() == *text),
                    Sel::AfterText(text) => own_tokens()
                        .find(|(t, _)| t.text() == *text)
                        .is_some_and(|(_, r)| r.end() <= start),
                });
            index += 1;
            self.arena.push(hit.map(|(_, (_, flow))| {
                (
                    access_of(self.grammar, *flow, state, op_text),
                    matches!(flow, Flow::Outside),
                )
            }));
        }
    }

    /// A write to a name no scope has declares it, in the nearest scope that holds variables.
    fn declare_implicit(&mut self, flags: Flags) {
        for i in 0..self.model.refs.len() {
            let r = &self.model.refs[i];
            if !r.is_write() || self.model.find(r).is_some() {
                continue;
            }
            let (name, scope) = (r.name.clone(), self.model.var_scope(r.scope));
            let decl = Decl {
                name: r.range,
                node_range: r.node_range,
                node_kind: r.node_kind,
                binding_range: r.node_range,
                binding_kind: r.node_kind,
                flags,
            };
            self.add_decl(scope, name, decl);
        }
    }

    fn resolve(&mut self) {
        if let Some(flags) = self.table.implicit_flags() {
            self.declare_implicit(flags);
        }
        let found: Vec<Option<SymbolId>> =
            self.model.refs.iter().map(|r| self.model.find(r)).collect();
        for (i, symbol) in found.into_iter().enumerate() {
            self.model.refs[i].symbol = symbol;
            if let Some(s) = symbol {
                self.model.symbols[s.0 as usize].refs.push(RefId(i as u32));
            }
        }
    }
}

/// What a [`Flow`] does to the access of the names it reaches: `state` is what the node itself
/// has, `op` the text of its operator.
fn access_of(grammar: &Grammar, flow: Flow, state: State, op: Option<&str>) -> Access {
    match flow {
        Flow::Write => Access::WRITE,
        Flow::ReadWrite => Access::READ.union(Access::WRITE),
        Flow::Assign(plain) if op == Some(plain) => Access::WRITE,
        Flow::Assign(_) => Access::READ.union(Access::WRITE),
        Flow::Assigns => match op {
            Some("++" | "--") => Access::READ.union(Access::WRITE),
            Some(t) => match grammar.operator_kind(t) {
                Some("assign") => Access::WRITE,
                Some("compound") => Access::READ.union(Access::WRITE),
                _ => Access::NONE,
            },
            None => Access::NONE,
        },
        Flow::Type => Access::TYPE,
        Flow::Query => Access::READ.union(Access::TYPE),
        Flow::Through => state,
        Flow::ThroughType if state.contains(Access::TYPE) => state,
        Flow::ThroughType | Flow::Outside | Flow::Skip => Access::NONE,
        Flow::Declare => Access::DECLARE,
    }
}

/// The children of a green node with the range each one fills, the node starting at `start`.
fn own_parts(
    green: &GreenNodeData,
    start: TextSize,
) -> impl Iterator<Item = (NodeOrToken<&GreenNodeData, &GreenTokenData>, TextRange)> {
    let mut at = start;
    green.children().map(move |c| {
        let range = TextRange::at(at, c.text_len());
        at += c.text_len();
        (c, range)
    })
}
