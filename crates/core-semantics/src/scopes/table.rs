//! What a language says about its node kinds to the binder: which kinds hold names, refer to
//! names or assign. Rows are data, written with the grammar's kind names; [`Table::new`] resolves
//! them against the grammar once. Which kinds open scopes is the grammar's `#scope` role; the
//! table only says which of those scopes hold `var` declarations.

use super::Flags;
use core_syntax::{Grammar, SyntaxKind};

/// The scope a declaration goes to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// The innermost scope when the node is entered (its own scope when it opens one).
    Scope,
    /// The scope around the node's own scope (the same as `Scope` for a node with none).
    Parent,
    /// The nearest scope that holds `var` declarations.
    Var,
    /// The nearest enclosing scope whose node has this kind (`infer U` goes to its conditional type).
    Enclosing(&'static str),
}

/// The child nodes of a node a [`Flow`] applies to.
#[derive(Copy, Clone, Debug)]
pub enum Sel {
    All,
    First,
    /// The child filling a role field or labelled item of the grammar's body.
    Field(&'static str),
    /// The children before the first own operator token.
    BeforeOp,
    /// The children after the first own token with this text.
    AfterText(&'static str),
    /// All the children, when the node has an own token with this text (`export type { A }`).
    WithToken(&'static str),
}

/// What the references among the selected children do with their names.
#[derive(Copy, Clone, Debug)]
pub enum Flow {
    /// They are assigned.
    Write,
    /// They are read and assigned (`a++`).
    ReadWrite,
    /// They are assigned; read too unless the node's operator token has this text (`+=` reads, `=` does not).
    Assign(&'static str),
    /// They name types.
    Type,
    /// They name values in a type (`typeof x`): read, and in a type.
    Query,
    /// They are outside the scope of the node (the false branch of a conditional type).
    Outside,
    /// The state of the node itself (assigned, a type) goes on to them: an array pattern.
    Through,
    /// Only the type state goes on: the left of a qualified name `N.T`, a member `a.b` in a type.
    ThroughType,
}

/// A node whose binding nodes declare names.
#[derive(Copy, Clone, Debug)]
pub struct Declares {
    pub flags: Flags,
    pub place: Place,
    /// When the node is not in a statement position (a function or class expression): the flags
    /// and the place of its name instead.
    pub expression: Option<(Flags, Place)>,
    /// The first own token with one of these texts (`var`, `let`, `const`) overrides flags and place.
    pub keywords: &'static [(&'static str, Flags, Place)],
    /// Read the keyword from the parent node (a declarator takes it from its declaration).
    pub keyword_on_parent: bool,
    /// The place when the parent node has one of these kinds (`infer` type parameters).
    pub parent_places: &'static [(&'static str, Place)],
    /// The name is the first child node's identifier or string (an enum member), not a binding node.
    pub name_child: bool,
}

impl Declares {
    pub const NONE: Declares = Declares {
        flags: Flags(0),
        place: Place::Scope,
        expression: None,
        keywords: &[],
        keyword_on_parent: false,
        parent_places: &[],
        name_child: false,
    };
}

#[derive(Copy, Clone, Debug)]
pub struct Row {
    pub kind: &'static str,
    /// The node opens a scope (the grammar's `#scope`) that holds `var` declarations.
    pub var_scope: bool,
    pub declares: Option<Declares>,
    /// A leaf that declares its name in the nearest node that `declares`.
    pub binding: bool,
    /// A leaf that refers to a name.
    pub reference: bool,
    pub children: &'static [(Sel, Flow)],
}

impl Row {
    pub const NONE: Row = Row {
        kind: "",
        var_scope: false,
        declares: None,
        binding: false,
        reference: false,
        children: &[],
    };
}

/// A language's rows, resolved against its grammar.
pub struct Table {
    rows: Vec<Option<&'static Row>>,
    scopes: Vec<bool>,
    hosts: Vec<bool>,
    name_tokens: Vec<SyntaxKind>,
    operators: Vec<bool>,
    params: Vec<bool>,
}

impl Table {
    /// `hosts` are the node kinds besides blocks (`#block`) whose child functions and classes are
    /// declarations; `name_tokens` the token kinds a name is written with (others are no name).
    pub fn new(
        grammar: &Grammar,
        rows: &'static [Row],
        hosts: &[&str],
        name_tokens: &[&str],
    ) -> Table {
        let kind = |name: &str| {
            grammar
                .kind_by_name(name)
                .unwrap_or_else(|| panic!("scope table: no kind `{name}` in the grammar"))
        };
        let size = |tokens: bool| {
            let list = if tokens {
                grammar.tokens()
            } else {
                grammar.nodes()
            };
            list.len()
        };
        let mut table = Table {
            rows: vec![None; size(false)],
            scopes: vec![false; size(false)],
            hosts: vec![false; size(false)],
            name_tokens: name_tokens.iter().map(|n| kind(n)).collect(),
            operators: vec![false; size(true)],
            params: vec![false; size(false)],
        };
        for row in rows {
            table.rows[kind(row.kind).index() as usize] = Some(row);
        }
        for def in grammar.nodes() {
            let i = def.kind.index() as usize;
            table.scopes[i] = def.has_tag("scope");
            table.hosts[i] = def.has_tag("block");
            table.params[i] = def.has_tag("parameter-list");
        }
        for name in hosts {
            table.hosts[kind(name).index() as usize] = true;
        }
        for def in grammar.tokens() {
            table.operators[def.kind.index() as usize] = def.has_tag("operator");
        }
        table
    }

    pub(crate) fn row(&self, kind: SyntaxKind) -> Option<&'static Row> {
        self.rows.get(kind.index() as usize).copied().flatten()
    }

    pub(crate) fn is_scope(&self, kind: SyntaxKind) -> bool {
        self.scopes
            .get(kind.index() as usize)
            .copied()
            .unwrap_or(false)
    }

    /// A node whose references see only the parameters of the scope around it: the `parameter-list` role.
    pub(crate) fn is_params(&self, kind: SyntaxKind) -> bool {
        self.params
            .get(kind.index() as usize)
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn is_host(&self, kind: SyntaxKind) -> bool {
        self.hosts
            .get(kind.index() as usize)
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn is_name_token(&self, kind: SyntaxKind) -> bool {
        self.name_tokens.contains(&kind)
    }

    pub(crate) fn is_operator(&self, kind: SyntaxKind) -> bool {
        self.operators
            .get(kind.index() as usize)
            .copied()
            .unwrap_or(false)
    }
}
