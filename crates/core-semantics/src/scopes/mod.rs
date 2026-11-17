//! The scope model of one file: scopes, the symbols declared in them and the references that
//! resolve to those symbols. Language neutral: [`bind`] walks a tree and a language's [`Table`]
//! (which node kinds open scopes, declare names, refer to names, assign) decides what each node
//! means; resolution is by name up the scope chain, over the namespaces (value, type) a symbol's
//! flags name. What a language adds to the rules that read the model is data of its table or a
//! hook of its own.

mod build;
mod table;

pub use build::bind;
pub use table::{Declares, Flow, Place, Row, Sel, Table};

use core_syntax::{SyntaxNode, TextRange};
use std::collections::HashMap;

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
    names: HashMap<Box<str>, SymbolId>,
}

/// One place a name is declared.
#[derive(Debug, Clone)]
pub struct Decl {
    /// The name as written.
    pub name: TextRange,
    /// The declaring node (a declarator, a parameter, a function, an import specifier).
    pub node: SyntaxNode,
    /// The binding node the name was read from.
    pub binding: SyntaxNode,
    pub flags: Flags,
}

#[derive(Debug)]
pub struct Symbol {
    pub name: Box<str>,
    pub scope: ScopeId,
    /// The union of the flags of its declarations.
    pub flags: Flags,
    /// In source order.
    pub decls: Vec<Decl>,
    pub refs: Vec<RefId>,
}

#[derive(Debug)]
pub struct Reference {
    pub name: Box<str>,
    /// The reference node.
    pub node: SyntaxNode,
    /// The name as written.
    pub range: TextRange,
    pub access: Access,
    pub scope: ScopeId,
    /// The scopes whose parameter lists the reference is in: it sees only their parameters.
    pub in_params: Vec<ScopeId>,
    pub symbol: Option<SymbolId>,
}

impl Reference {
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

#[derive(Debug, Default)]
pub struct Model {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
    pub refs: Vec<Reference>,
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
            .filter_map(|s| self.own(s, name))
            .find(|s| self.symbol(*s).flags.intersects(want))
    }

    /// The symbol the reference finds: as [`Model::lookup`], except that a reference in a
    /// parameter list does not see what the body of that function declares.
    pub fn find(&self, r: &Reference) -> Option<SymbolId> {
        let ty = !(r.is_read() || r.is_write());
        let want = if ty { Flags::TYPE } else { Flags::VALUE };
        let params = Flags::PARAMETER | Flags::TYPE_PARAMETER | Flags::EXPRESSION_NAME;
        self.ancestors(r.scope).find_map(|s| {
            let id = self.own(s, &r.name)?;
            let f = self.symbol(id).flags;
            (f.intersects(want) && (!r.in_params.contains(&s) || f.intersects(params)))
                .then_some(id)
        })
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
    pub(crate) fn new(parent: Option<ScopeId>, node: SyntaxNode, var: bool) -> Scope {
        Scope {
            parent,
            node,
            var,
            symbols: Vec::new(),
            names: HashMap::new(),
        }
    }

    pub(crate) fn names_mut(&mut self) -> &mut HashMap<Box<str>, SymbolId> {
        &mut self.names
    }
}
