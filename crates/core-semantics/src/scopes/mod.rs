//! The scope model of one file: scopes, the symbols declared in them and the references that
//! resolve to those symbols. Language neutral: [`bind`] walks a tree and a language's [`Table`]
//! (which node kinds open scopes, declare names, refer to names, assign) decides what each node
//! means; resolution is by name up the scope chain, over the namespaces (value, type) a symbol's
//! flags name. What a language adds to the rules that read the model is data of its table or a
//! hook of its own.

mod build;
mod table;

pub use build::bind;
pub use table::{Declares, Flow, Place, Redirect, Row, Sel, Table};

use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use rustc_hash::FxHashMap as HashMap;
use std::rc::Rc;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeId(pub u32);
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub u32);
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RefId(pub u32);

/// What a symbol or one of its declarations is: the namespaces it lives in and its kind.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flags(pub u32);

impl Flags {
    /// In the value namespace: a reference that is not a type finds it.
    pub const VALUE: Flags = Flags(1);
    /// In the type namespace: a type reference finds it.
    pub const TYPE: Flags = Flags(1 << 1);
    /// A variable that can be assigned only once (`const`).
    pub const CONST: Flags = Flags(1 << 2);
    /// A variable of the nearest function scope (`var`).
    pub const VAR: Flags = Flags(1 << 3);
    pub const LET: Flags = Flags(1 << 4);
    pub const FUNCTION: Flags = Flags(1 << 5);
    pub const CLASS: Flags = Flags(1 << 6);
    pub const PARAMETER: Flags = Flags(1 << 7);
    pub const IMPORT: Flags = Flags(1 << 8);
    pub const CATCH: Flags = Flags(1 << 9);
    pub const ENUM: Flags = Flags(1 << 10);
    pub const ENUM_MEMBER: Flags = Flags(1 << 11);
    pub const NAMESPACE: Flags = Flags(1 << 12);
    pub const INTERFACE: Flags = Flags(1 << 13);
    pub const TYPE_ALIAS: Flags = Flags(1 << 14);
    pub const TYPE_PARAMETER: Flags = Flags(1 << 15);
    /// The name of a function or class expression, visible inside it only.
    pub const EXPRESSION_NAME: Flags = Flags(1 << 16);
    /// Declared by a statement that sends a name to another scope (`global`, `nonlocal`).
    pub const REDIRECT: Flags = Flags(1 << 17);
    /// Declared with no value (`x: int`): no read finds a value.
    pub const BARE: Flags = Flags(1 << 18);

    pub const fn union(self, o: Flags) -> Flags {
        Flags(self.0 | o.0)
    }

    pub const fn contains(self, o: Flags) -> bool {
        self.0 & o.0 == o.0
    }

    pub const fn intersects(self, o: Flags) -> bool {
        self.0 & o.0 != 0
    }

    /// The flags that are not in `o`.
    pub const fn without(self, o: Flags) -> Flags {
        Flags(self.0 & !o.0)
    }
}

impl std::ops::BitOr for Flags {
    type Output = Flags;
    fn bitor(self, o: Flags) -> Flags {
        self.union(o)
    }
}

/// How a reference uses its name.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Access(u8);

impl Access {
    pub const NONE: Access = Access(0);
    pub const READ: Access = Access(1);
    pub const WRITE: Access = Access(2);
    /// Written in a type: names a type (`x: T`), or with READ a value (`typeof x`).
    pub const TYPE: Access = Access(4);
    /// Assigned in the nearest scope that holds `var` declarations.
    pub const HOIST: Access = Access(8);
    /// Unbound (`del x`); with READ.
    pub const DELETE: Access = Access(16);
    /// Declared with no value.
    pub const BARE: Access = Access(32);

    pub const fn union(self, o: Access) -> Access {
        Access(self.0 | o.0)
    }

    pub const fn contains(self, o: Access) -> bool {
        self.0 & o.0 == o.0
    }
}

#[derive(Debug)]
pub struct Scope {
    pub parent: Option<ScopeId>,
    pub node: SyntaxNode,
    /// `var` declarations of the scopes inside it stop here.
    pub var: bool,
    pub symbols: Vec<SymbolId>,
    /// Its names are not visible to the scopes nested in it.
    pub hidden: bool,
    names: HashMap<Rc<str>, SymbolId>,
    /// Names whose declarations and references go to another scope (`global`, `nonlocal`).
    redirects: Option<HashMap<Rc<str>, ScopeId>>,
}

/// One place a name is declared.
#[derive(Debug, Clone)]
pub struct Decl {
    /// The name as written.
    pub name: TextRange,
    /// Where the declaring node (a declarator, a parameter, a function, an import specifier) and
    /// the binding node the name was read from are, and their kinds; the model holds no node,
    /// which would keep the tree's nodes alive.
    pub node_range: TextRange,
    pub node_kind: SyntaxKind,
    pub binding_range: TextRange,
    pub binding_kind: SyntaxKind,
    pub flags: Flags,
    /// The scope the declaration is written in (not the symbol's, when `global` sent it elsewhere).
    pub from: ScopeId,
}

impl Decl {
    /// The declaring node, found in the tree `root` the model was built from.
    pub fn node(&self, root: &SyntaxNode) -> SyntaxNode {
        node_at(root, self.node_range, self.node_kind)
    }

    /// The binding node, found in the tree `root` the model was built from.
    pub fn binding(&self, root: &SyntaxNode) -> SyntaxNode {
        node_at(root, self.binding_range, self.binding_kind)
    }
}

/// The node of `kind` that fills `range`.
fn node_at(root: &SyntaxNode, range: TextRange, kind: SyntaxKind) -> SyntaxNode {
    let found = match root.covering_element(range) {
        core_syntax::SyntaxElement::Node(n) => Some(n),
        core_syntax::SyntaxElement::Token(t) => t.parent(),
    };
    found
        .into_iter()
        .flat_map(|n| n.ancestors())
        .find(|n| n.kind() == kind)
        .expect("a node of the model is in its tree")
}

#[derive(Debug)]
pub struct Symbol {
    pub name: Rc<str>,
    pub scope: ScopeId,
    /// The union of the flags of its declarations.
    pub flags: Flags,
    /// In source order.
    pub decls: Vec<Decl>,
    pub refs: Vec<RefId>,
}

#[derive(Debug)]
pub struct Reference {
    pub name: Rc<str>,
    /// Where the reference node is and what kind it is; [`Reference::node`] finds it. The model
    /// holds no node of a reference, which would keep the tree's nodes alive.
    pub node_range: TextRange,
    pub node_kind: SyntaxKind,
    /// The name as written.
    pub range: TextRange,
    pub access: Access,
    pub scope: ScopeId,
    /// The scopes whose parameter lists the reference is in: it sees only their parameters.
    pub in_params: Vec<ScopeId>,
    pub symbol: Option<SymbolId>,
}

impl Reference {
    /// The reference node, found in the tree `root` the model was built from.
    pub fn node(&self, root: &SyntaxNode) -> SyntaxNode {
        node_at(root, self.node_range, self.node_kind)
    }

    pub fn is_delete(&self) -> bool {
        self.access.contains(Access::DELETE)
    }

    pub fn is_write(&self) -> bool {
        self.access.contains(Access::WRITE)
    }

    pub fn is_type(&self) -> bool {
        self.access.contains(Access::TYPE)
    }

    /// A read of the value (a type reference is none).
    pub fn is_read(&self) -> bool {
        self.access.contains(Access::READ)
    }
}

/// A leaf whose text names things (a string in an annotation), found in a type position.
#[derive(Debug)]
pub struct TextRef {
    pub node_range: TextRange,
    pub node_kind: SyntaxKind,
    pub scope: ScopeId,
}

impl TextRef {
    /// The leaf, found in the tree `root` the model was built from.
    pub fn node(&self, root: &SyntaxNode) -> SyntaxNode {
        node_at(root, self.node_range, self.node_kind)
    }
}

#[derive(Debug, Default)]
pub struct Model {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
    pub refs: Vec<Reference>,
    /// The leaves of rows with `text` that stand in a type position, in source order.
    pub texts: Vec<TextRef>,
}

impl Model {
    pub fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0 as usize]
    }

    pub fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    pub fn reference(&self, id: RefId) -> &Reference {
        &self.refs[id.0 as usize]
    }

    pub fn symbol_ids(&self) -> impl Iterator<Item = SymbolId> {
        (0..self.symbols.len() as u32).map(SymbolId)
    }

    /// The scope and its ancestors, innermost first.
    pub fn ancestors(&self, from: ScopeId) -> impl Iterator<Item = ScopeId> + '_ {
        std::iter::successors(Some(from), |s| self.scope(*s).parent)
    }

    /// The nearest scope (or the top one) that holds `var` declarations.
    pub fn var_scope(&self, from: ScopeId) -> ScopeId {
        self.ancestors(from)
            .find(|s| self.scope(*s).var)
            .unwrap_or(ScopeId(0))
    }

    /// The symbol `name` declares in exactly this scope.
    pub fn own(&self, scope: ScopeId, name: &str) -> Option<SymbolId> {
        self.scope(scope).names.get(name).copied()
    }

    /// The symbol a reference to `name` made in `from` finds, in the value or the type namespace.
    pub fn lookup(&self, from: ScopeId, name: &str, ty: bool) -> Option<SymbolId> {
        let want = if ty { Flags::TYPE } else { Flags::VALUE };
        self.ancestors(from)
            .filter(|s| *s == from || !self.scope(*s).hidden)
            .filter_map(|s| self.own(self.redirect(s, name).unwrap_or(s), name))
            .find(|s| self.symbol(*s).flags.intersects(want))
    }

    /// Where the declarations and references of `name` in `scope` go, when a statement sent them elsewhere.
    pub fn redirect(&self, scope: ScopeId, name: &str) -> Option<ScopeId> {
        self.scope(scope).redirects.as_ref()?.get(name).copied()
    }

    /// The symbol the reference finds: as [`Model::lookup`], except that a reference in a
    /// parameter list does not see what the body of that function declares.
    pub fn find(&self, r: &Reference) -> Option<SymbolId> {
        let ty = !(r.is_read() || r.is_write());
        let want = if ty { Flags::TYPE } else { Flags::VALUE };
        let params = Flags::PARAMETER | Flags::TYPE_PARAMETER | Flags::EXPRESSION_NAME;
        self.ancestors(r.scope).find_map(|s| {
            if s != r.scope && self.scope(s).hidden {
                return None;
            }
            let id = self.own(self.redirect(s, &r.name).unwrap_or(s), &r.name)?;
            let f = self.symbol(id).flags;
            (f.intersects(want) && (!r.in_params.contains(&s) || f.intersects(params)))
                .then_some(id)
        })
    }

    /// A reference the binder did not meet (a name inside a string), resolved like the others.
    pub fn add_reference(
        &mut self,
        name: &str,
        range: TextRange,
        node: (TextRange, SyntaxKind),
        access: Access,
        scope: ScopeId,
    ) -> RefId {
        let mut r = Reference {
            name: name.into(),
            node_range: node.0,
            node_kind: node.1,
            range,
            access,
            scope,
            in_params: Vec::new(),
            symbol: None,
        };
        r.symbol = self.find(&r);
        let id = RefId(self.refs.len() as u32);
        if let Some(s) = r.symbol {
            self.symbols[s.0 as usize].refs.push(id);
        }
        self.refs.push(r);
        id
    }

    /// The references no scope resolves.
    pub fn unresolved(&self) -> impl Iterator<Item = &Reference> {
        self.refs.iter().filter(|r| r.symbol.is_none())
    }

    /// The resolved references of a symbol.
    pub fn refs_of(&self, id: SymbolId) -> impl Iterator<Item = &Reference> {
        self.symbol(id).refs.iter().map(|r| self.reference(*r))
    }
}

impl Scope {
    pub(crate) fn new(parent: Option<ScopeId>, node: SyntaxNode, var: bool, hidden: bool) -> Scope {
        Scope {
            parent,
            node,
            var,
            symbols: Vec::new(),
            hidden,
            names: HashMap::default(),
            redirects: None,
        }
    }

    pub(crate) fn redirects_mut(&mut self) -> &mut HashMap<Rc<str>, ScopeId> {
        self.redirects.get_or_insert_with(HashMap::default)
    }

    pub(crate) fn names_mut(&mut self) -> &mut HashMap<Rc<str>, SymbolId> {
        &mut self.names
    }
}
