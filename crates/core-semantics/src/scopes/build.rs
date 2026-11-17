//! The binder: one pre-order walk that opens scopes, declares names and records references as the
//! language's [`Table`] says, then resolves every reference by name up the scope chain.

use super::table::{Declares, Flow, Place, Redirect, Row, Sel, Table};
use super::{
    Access, Decl, Flags, Model, RefId, Reference, Scope, ScopeId, Symbol, SymbolId, TextRef,
};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange, TextSize};
use rowan::{GreenNodeData, GreenTokenData, NodeOrToken, WalkEvent};
use rustc_hash::FxHashSet as HashSet;
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
        arena: Vec::new(),
        names: HashSet::default(),
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
            let hidden = row.is_some_and(|r| r.hidden);
            self.model
                .scopes
                .push(Scope::new(outer, node.clone(), var, hidden));
            self.scopes.push(id);
            frame.scope = true;
        }
        let Some(row) = row else { return frame };
        if let Some(d) = &row.declares {
            self.declares(node, d, frame.scope, outer);
            frame.decl = true;
        }
        if let Some(how) = row.redirect {
            self.redirect(node, how);
        }
        if row.binding {
            self.binding(node);
        }
        if row.reference {
            self.reference(node, state, row);
        }
        if row.text && state.contains(Access::TYPE) {
            self.model.texts.push(TextRef {
                node_range: node.text_range(),
                node_kind: kind,
                scope: self.current(),
            });
        }
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
        let name = match self.names.get(name) {
            Some(n) => n.clone(),
            None => {
                let n: Rc<str> = name.into();
                self.names.insert(n.clone());
                n
            }
        };
        Some((name, range))
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
        let scope = self.place_scope(place, own_scope, outer);
        self.ctxs.push(Ctx {
            flags,
            scope,
            node: node.clone(),
        });
        for sel in d.names {
            let child = match sel {
                Sel::Field(f) => self.grammar.slot(node, f).and_then(|e| e.into_node()),
                Sel::Kind(k) => node.children().find(|c| self.grammar.name(c.kind()) == *k),
                _ => None,
            };
            if let Some(child) = child
                && let Some((name, range)) = self.name_token(&child, false)
            {
                self.declare(name, range, &child);
                break;
            }
        }
        if d.name_child
            && let Some(child) = node.children().next()
            && let Some((name, range)) = self.name_token(&child, true)
        {
            self.declare(name, range, &child);
        }
    }

    fn place_scope(&self, place: Place, own_scope: bool, outer: Option<ScopeId>) -> ScopeId {
        let current = self.current();
        match place {
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
        }
    }

    /// `global x`, `nonlocal x`: the names of the child nodes are declared in another scope, and
    /// this scope sends its declarations and references of them there.
    fn redirect(&mut self, node: &SyntaxNode, how: Redirect) {
        let current = self.current();
        for child in node.children() {
            let Some((name, range)) = self.name_token(&child, false) else {
                continue;
            };
            let target = match how {
                Redirect::Top => ScopeId(0),
                Redirect::Outer => {
                    let vars: Vec<ScopeId> = self
                        .model
                        .ancestors(current)
                        .skip(1)
                        .filter(|s| self.model.scope(*s).var)
                        .collect();
                    vars.iter()
                        .copied()
                        .find(|s| self.model.own(*s, &name).is_some())
                        .or(vars.first().copied())
                        .unwrap_or(ScopeId(0))
                }
            };
            self.declare_in(
                name.clone(),
                range,
                &child,
                (node.text_range(), node.kind()),
                Flags::VALUE | Flags::REDIRECT,
                target,
            );
            self.model.scopes[current.0 as usize]
                .redirects_mut()
                .insert(name, target);
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
        let node = (ctx.node.text_range(), ctx.node.kind());
        let (scope, flags) = (ctx.scope, ctx.flags);
        self.declare_in(name, range, binding, node, flags, scope);
    }

    fn declare_in(
        &mut self,
        name: Rc<str>,
        range: TextRange,
        binding: &SyntaxNode,
        node: (TextRange, SyntaxKind),
        flags: Flags,
        mut scope: ScopeId,
    ) {
        if let Some(target) = self.model.redirect(scope, &name) {
            scope = target;
        }
        let decl = Decl {
            name: range,
            node_range: node.0,
            node_kind: node.1,
            binding_range: binding.text_range(),
            binding_kind: binding.kind(),
            flags,
            from: self.current(),
        };
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

    fn reference(&mut self, node: &SyntaxNode, state: State, row: &Row) {
        let Some((name, range)) = self.name_token(node, false) else {
            return;
        };
        let access = if state == Access::NONE {
            Access::READ
        } else {
            state
        };
        if access.contains(Access::WRITE)
            && let Some((flags, place)) = row.implicit
        {
            let place = if access.contains(Access::HOIST) {
                Place::Var
            } else {
                place
            };
            let scope = self.place_scope(place, false, None);
            let at = (node.text_range(), node.kind());
            let flags = if access.contains(Access::BARE) {
                flags | Flags::BARE
            } else {
                flags
            };
            self.declare_in(name.clone(), range, node, at, flags, scope);
        }
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

    /// The state of each child node of `node`, pushed on the arena. Read off the green tree, so
    /// the children and tokens of the node are not made.
    fn flows(&mut self, node: &SyntaxNode, row: &Row, state: State) {
        // Children that only pass on a state of nothing get what a child gets by default.
        if state == Access::NONE
            && row
                .children
                .iter()
                .all(|(_, f)| matches!(f, Flow::Through | Flow::ThroughType))
        {
            return;
        }
        let mut conds: Vec<(usize, bool)> = Vec::new();
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
        let node_count = if row
            .children
            .iter()
            .any(|(s, _)| matches!(s, Sel::ExceptLast))
        {
            own_parts(&green, base)
                .filter(|(c, _)| matches!(c, NodeOrToken::Node(_)))
                .count()
        } else {
            0
        };
        let own_tokens = || {
            own_parts(&green, base).filter_map(|(c, r)| match c {
                NodeOrToken::Token(t) => Some((t, r)),
                NodeOrToken::Node(_) => None,
            })
        };
        let op = own_tokens().find(|(t, _)| self.table.is_operator(SyntaxKind(t.kind().0)));
        let op_start = op.map(|(_, r)| r.start());
        let op_text = op.map(|(t, _)| t.text());
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
                    Sel::ExceptLast => index + 1 < node_count,
                    Sel::Kind(k) => match &child {
                        NodeOrToken::Node(n) => self.grammar.name(SyntaxKind(n.kind().0)) == *k,
                        NodeOrToken::Token(_) => false,
                    },
                    Sel::Field(_) => fields[*i] == Some(range),
                    Sel::BeforeOp => op_start.is_none_or(|o| start < o),
                    Sel::WithToken(text) => own_tokens().any(|(t, _)| t.text() == *text),
                    Sel::AfterText(text) => own_tokens()
                        .find(|(t, _)| t.text() == *text)
                        .is_some_and(|(_, r)| r.end() <= start),
                });
            index += 1;
            self.arena
                .push(hit.map(|(_, (_, flow))| access(flow, node, state, op_text, &mut conds)));
        }
    }

    fn resolve(&mut self) {
        // Names are interned, so the same name read in the same scope finds the same symbol
        // (unless it is read in a parameter list).
        let mut seen: rustc_hash::FxHashMap<(u32, usize, bool), Option<SymbolId>> =
            Default::default();
        let found: Vec<Option<SymbolId>> = (self.model.refs.iter())
            .map(|r| {
                if !r.in_params.is_empty() {
                    return self.model.find(r);
                }
                let key = (
                    r.scope.0,
                    Rc::as_ptr(&r.name) as *const u8 as usize,
                    !(r.is_read() || r.is_write()),
                );
                *seen.entry(key).or_insert_with(|| self.model.find(r))
            })
            .collect();
        for (i, symbol) in found.into_iter().enumerate() {
            self.model.refs[i].symbol = symbol;
            if let Some(s) = symbol {
                self.model.symbols[s.0 as usize].refs.push(RefId(i as u32));
            }
        }
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

/// The state a [`Flow`] gives the child, and whether the child is outside the node's scope.
fn access(
    flow: &Flow,
    node: &SyntaxNode,
    state: State,
    op_text: Option<&str>,
    conds: &mut Vec<(usize, bool)>,
) -> (State, bool) {
    let rw = Access::READ.union(Access::WRITE);
    let query = Access::READ.union(Access::TYPE);
    match flow {
        Flow::Write => (Access::WRITE, false),
        Flow::ReadWrite => (rw, false),
        Flow::Assign(plain) if op_text == Some(*plain) => (Access::WRITE, false),
        Flow::Assign(_) => (rw, false),
        Flow::Type => (Access::TYPE, false),
        Flow::Query => (query, false),
        Flow::Through => (state, false),
        Flow::ThroughType if state.contains(Access::TYPE) => (state, false),
        Flow::ThroughType => (Access::NONE, false),
        Flow::Outside => (Access::NONE, true),
        Flow::WriteBare => (Access::WRITE.union(Access::BARE), false),
        Flow::Delete => (Access::READ.union(Access::DELETE), false),
        Flow::WriteVar => (Access::WRITE.union(Access::HOIST), false),
        Flow::OutsideQuery => (query, true),
        // Each test runs once for the node, whatever the number of its children.
        Flow::Cond(test, yes, no) => {
            let key = *test as usize;
            let held = conds.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
            let hit = held.unwrap_or_else(|| {
                let v = test(node);
                conds.push((key, v));
                v
            });
            access(if hit { yes } else { no }, node, state, op_text, conds)
        }
    }
}
