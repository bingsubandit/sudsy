//! The binder: one pre-order walk that opens scopes, declares names and records references as the
//! language's [`Table`] says, then resolves every reference by name up the scope chain.

use super::table::{Declares, Flow, Place, Row, Sel, Table};
use super::{Access, Decl, Flags, Model, RefId, Reference, Scope, ScopeId, Symbol, SymbolId};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken, TextRange};
use rowan::WalkEvent;

/// What a node inherits from its parent about the references it holds: the access they have,
/// none for a plain read.
type State = Access;

struct Frame {
    /// The state of each child node, when the row says anything about them.
    flows: Vec<Option<(State, bool)>>,
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
    };
    let mut frames: Vec<Frame> = Vec::new();
    for event in root.preorder() {
        match event {
            WalkEvent::Enter(node) => {
                let (state, outside) = match frames.last_mut() {
                    Some(parent) => {
                        let i = parent.next;
                        parent.next += 1;
                        parent.flows.get(i).copied().flatten().unwrap_or_default()
                    }
                    None => Default::default(),
                };
                let frame = b.enter(&node, state, outside);
                frames.push(frame);
            }
            WalkEvent::Leave(_) => {
                if let Some(frame) = frames.pop() {
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
            flows: Vec::new(),
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
            self.model.scopes.push(Scope::new(outer, node.clone(), var));
            self.scopes.push(id);
            frame.scope = true;
        }
        let Some(row) = row else { return frame };
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
        if !row.children.is_empty() {
            frame.flows = self.flows(node, row, state);
        }
        frame
    }

    fn current(&self) -> ScopeId {
        self.scopes.last().copied().unwrap_or(ScopeId(0))
    }

    /// The first token under `node` that is code: a name when its kind is one of the table's.
    fn name_token(&self, node: &SyntaxNode) -> Option<SyntaxToken> {
        let t = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| !self.grammar.is_trivia(t.kind()) && !t.text_range().is_empty())?;
        self.table.is_name_token(t.kind()).then_some(t)
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
                n.children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find_map(|t| d.keywords.iter().find(|(k, ..)| *k == t.text()))
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
            && let Some(t) = self.name_token(&child)
        {
            let text = t.text();
            let name = text.trim_matches(['"', '\'']);
            let range = t.text_range();
            self.declare(name, range, &child);
        }
    }

    fn binding(&mut self, node: &SyntaxNode) {
        if self.ctxs.is_empty() {
            return;
        }
        if let Some(t) = self.name_token(node) {
            self.declare(t.text(), t.text_range(), node);
        }
    }

    fn declare(&mut self, name: &str, range: TextRange, binding: &SyntaxNode) {
        let Some(ctx) = self.ctxs.last() else { return };
        let decl = Decl {
            name: range,
            node: ctx.node.clone(),
            binding: binding.clone(),
            flags: ctx.flags,
        };
        let (scope, flags) = (ctx.scope, ctx.flags);
        let scope_ref = &mut self.model.scopes[scope.0 as usize];
        // The name of a function expression is a variable of its own, between the function's scope
        // and the one around it: a declaration of the same name in the function is another one.
        if let Some(&id) = scope_ref.names_mut().get(name)
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
        scope_ref.names_mut().insert(name.into(), id);
        scope_ref.symbols.push(id);
        self.model.symbols.push(Symbol {
            name: name.into(),
            scope,
            flags,
            decls: vec![decl],
            refs: Vec::new(),
        });
    }

    fn reference(&mut self, node: &SyntaxNode, state: State) {
        let Some(t) = self.name_token(node) else {
            return;
        };
        let access = if state == Access::NONE {
            Access::READ
        } else {
            state
        };
        self.model.refs.push(Reference {
            name: t.text().into(),
            node: node.clone(),
            range: t.text_range(),
            access,
            scope: self.current(),
            in_params: self.params.clone(),
            symbol: None,
        });
    }

    /// The state of each child node of `node`.
    fn flows(&self, node: &SyntaxNode, row: &Row, state: State) -> Vec<Option<(State, bool)>> {
        let fields: Vec<Option<TextRange>> = row
            .children
            .iter()
            .map(|(sel, _)| match sel {
                Sel::Field(f) => self.grammar.slot(node, f).map(|e| e.text_range()),
                _ => None,
            })
            .collect();
        let tokens: Vec<SyntaxToken> = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .collect();
        let op = tokens.iter().find(|t| self.table.is_operator(t.kind()));
        let op_start = op.map(|t| t.text_range().start());
        let op_text = op.map(|t| t.text());
        let mut out = Vec::new();
        for (index, child) in node.children().enumerate() {
            let start = child.text_range().start();
            let hit = row
                .children
                .iter()
                .enumerate()
                .find(|(i, (sel, _))| match sel {
                    Sel::All => true,
                    Sel::First => index == 0,
                    Sel::Field(_) => fields[*i] == Some(child.text_range()),
                    Sel::BeforeOp => op_start.is_none_or(|o| start < o),
                    Sel::WithToken(text) => tokens.iter().any(|t| t.text() == *text),
                    Sel::AfterText(text) => tokens
                        .iter()
                        .find(|t| t.text() == *text)
                        .is_some_and(|t| t.text_range().end() <= start),
                });
            out.push(hit.map(|(_, (_, flow))| {
                (
                    match flow {
                        Flow::Write => Access::WRITE,
                        Flow::ReadWrite => Access::READ.union(Access::WRITE),
                        Flow::Assign(plain) if op_text == Some(*plain) => Access::WRITE,
                        Flow::Assign(_) => Access::READ.union(Access::WRITE),
                        Flow::Type => Access::TYPE,
                        Flow::Query => Access::READ.union(Access::TYPE),
                        Flow::Through => state,
                        Flow::ThroughType if state.contains(Access::TYPE) => state,
                        Flow::ThroughType | Flow::Outside => Access::NONE,
                    },
                    matches!(flow, Flow::Outside),
                )
            }));
        }
        out
    }

    fn resolve(&mut self) {
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
