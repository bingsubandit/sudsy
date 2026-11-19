//! What a language says about its node kinds to the binder: which kinds hold names, refer to
//! names or assign. Rows are data, written with the grammar's kind names; [`Table::new`] resolves
//! them against the grammar once. Which kinds open scopes is the grammar's `#scope` role; the
//! table only says which of those scopes hold `var` declarations.
//!
//! concept: scope-tables
//! what: Which node kinds open scopes, declare, refer to or assign names, as rows resolved against the grammar.
//! tools: none (language facts as rows)
//! reads: family:scope, file:scope.rows

use super::Flags;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};

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
    /// The innermost scope, unless the name is already declared in it or in a scope it sees: a
    /// write then assigns that variable (Ruby: a block assigns a variable of its method).
    Unseen,
}

/// The child nodes of a node a [`Flow`] applies to.
#[derive(Copy, Clone, Debug)]
pub enum Sel {
    All,
    First,
    /// The child filling a role field or labelled item of the grammar's body.
    Field(&'static str),
    /// The children of this node kind (`Decorator`).
    Kind(&'static str),
    /// The last child (the last own name token of a node's tokens).
    Last,
    /// The last own name token before the first own token with this text (`for (T t : xs)`).
    LastBefore(&'static str),
    /// The children before the first own operator token.
    BeforeOp,
    /// The children after the first own token with this text.
    AfterText(&'static str),
    /// All the children, when the node has an own token with this text (`export type { A }`).
    WithToken(&'static str),
    /// All the child nodes but the last (the targets of `a = b = c`).
    ExceptLast,
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
    /// They are read and unbound (`del x`).
    Delete,
    /// They are assigned and declared with no value (`x: int`).
    WriteBare,
    /// The state of the node itself (assigned, a type) goes on to them: an array pattern.
    Through,
    /// Only the type state goes on: the left of a qualified name `N.T`, a member `a.b` in a type.
    ThroughType,
    /// They are assigned and declared in the nearest scope that holds `var` declarations (a walrus
    /// in a comprehension).
    WriteVar,
    /// As [`Flow::Outside`], and they name values in a type (an annotation, evaluated outside).
    OutsideQuery,
    /// The first flow when the predicate holds for the row's node, else the second.
    Cond(&'static Pred, &'static Flow, &'static Flow),
    /// They are assigned when the node's operator is an assignment (`=`), read and assigned when
    /// it is a compound one (`+=`) or `++`/`--`, and as the node itself otherwise (`&$x`).
    Assigns,
    /// They declare their names in the nearest node that `declares` (a parameter, a variable).
    Declare,
    /// They are not references (a name in a position no scope looks up).
    Skip,
    /// They are read to test that the name exists (`typeof x`): never undefined.
    Probe,
    /// They name a markup element (`<Tag />`).
    Tag,
    /// They declare their name in the scope of the node and assign the one of the scope around it
    /// (`use (&$x)` of a closure).
    Capture,
}

/// A test of the row's node that picks between two flows.
#[derive(Copy, Clone, Debug)]
pub enum Pred {
    /// The node has an own token with this text (`x: int = 1`: `=`).
    HasToken(&'static str),
    /// No sibling before the node has its kind (the first `for` of a comprehension).
    FirstOfKind,
    /// The node's head (its first own name token, else the name its first child ends with) is in the
    /// set (`typing.Optional`, a command word).
    HeadIn(&'static [&'static str]),
    /// The head of the node's parent is in the set (`cast(...)`: the arguments' parent).
    CalleeIs(&'static [&'static str]),
}

impl Pred {
    pub(crate) fn test(&self, node: &SyntaxNode, table: &Table) -> bool {
        match self {
            Pred::HasToken(text) => node
                .children_with_tokens()
                .any(|e| e.as_token().is_some_and(|t| t.text() == *text)),
            Pred::FirstOfKind => node.prev_sibling().is_none_or(|p| p.kind() != node.kind()),
            Pred::HeadIn(set) => table
                .head(node)
                .is_some_and(|name| set.contains(&name.text())),
            Pred::CalleeIs(set) => node
                .parent()
                .and_then(|call| table.head(&call))
                .is_some_and(|name| set.contains(&name.text())),
        }
    }
}

/// Where the names of a statement that sends declarations elsewhere (`global`, `nonlocal`) go.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Redirect {
    /// The top scope.
    Top,
    /// The nearest scope above the current one that holds `var` declarations (and the name, if one does).
    Outer,
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
    /// The name is in the first of these children that the node has (a function's `name`, an
    /// import's `asname` before its `name`), not in a binding node.
    pub names: &'static [Sel],
    /// The name is the last name token under the last child node (the end of an import path or
    /// its alias); none when the node has an own token with `skip_token`.
    pub name_last: bool,
    pub skip_token: &'static str,
    /// The flags when the parent node has one of these kinds (a property in a class is a member).
    pub parent_flags: &'static [(&'static str, Flags)],
    /// The declaration is unchecked (`Flags::UNCHECKED`) when its parent or one of its children
    /// has one of these kinds (`namespace A.B {}` declares `A` for `B`'s sake).
    pub unchecked: &'static [&'static str],
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
        names: &[],
        name_last: false,
        skip_token: "",
        parent_flags: &[],
        unchecked: &[],
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
    /// The names of the node's child nodes are declared in another scope for the rest of this one.
    pub redirect: Option<Redirect>,
    /// The names declared here are not visible to the scopes nested in this one (class bodies).
    pub hidden: bool,
    /// A reference that is assigned declares its name, with these flags, in this place (assigning
    /// declares: Python).
    pub implicit: Option<(Flags, Place)>,
    /// A leaf whose text names things when it stands in a type position: the model lists it.
    pub text: bool,
    /// What the node's own name tokens do, for a grammar whose names are bare tokens, not nodes
    /// (the first row that selects a token decides).
    pub tokens: &'static [(Sel, Flow)],
    /// The scope the node opens does not see the names of the scopes around it (a PHP function).
    pub closed: bool,
    /// Names under the node are not references (an import path); what the node declares still is.
    pub inert: bool,
    /// The node's operator is a child node (`operator:` slot), not a token.
    pub op_node: bool,
    /// The node makes the names of its scope unknowable (`$$x`, `include`): rules that need
    /// every name of the scope leave it alone.
    pub dynamic: bool,
}

impl Row {
    pub const NONE: Row = Row {
        kind: "",
        var_scope: false,
        declares: None,
        binding: false,
        reference: false,
        children: &[],
        redirect: None,
        hidden: false,
        implicit: None,
        text: false,
        tokens: &[],
        closed: false,
        inert: false,
        dynamic: false,
        op_node: false,
    };
}

/// A language's rows, resolved against its grammar.
pub struct Table {
    rows: Vec<Option<&'static Row>>,
    scopes: Vec<bool>,
    hosts: Vec<bool>,
    name_tokens: Vec<SyntaxKind>,
    labels: Vec<SyntaxKind>,
    operators: Vec<bool>,
    params: Vec<bool>,
    bare: bool,
    identifiers: bool,
    dynamic: &'static [&'static str],
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
            labels: Vec::new(),
            operators: vec![false; size(true)],
            params: vec![false; size(false)],
            bare: false,
            identifiers: false,
            dynamic: &[],
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

    /// Every own name token of a node is a reference unless a row says otherwise (Java: names
    /// are bare tokens, so a use is any name that is not a declaration).
    pub fn bare_names(mut self, bare: bool) -> Table {
        self.bare = bare;
        self
    }

    /// Only name tokens written as an identifier (letters, digits, `_`), optionally followed by a
    /// subscript `[...]` that is no part of the name, are names (shell words: `-r`, `x[0]`).
    pub fn identifier_names(mut self, on: bool) -> Table {
        self.identifiers = on;
        self
    }

    /// Referring to one of these names makes the names of the scope unknowable (`extract`).
    pub fn dynamic_names(mut self, names: &'static [&'static str]) -> Table {
        self.dynamic = names;
        self
    }

    /// Name tokens of these kinds are written with a trailing `:` that is no part of the name
    /// (Ruby `key:`).
    pub fn label_names(mut self, kinds: &[&str], grammar: &Grammar) -> Table {
        self.labels = kinds
            .iter()
            .map(|k| grammar.kind_by_name(k).expect("scope table: label kind"))
            .collect();
        self
    }

    pub(crate) fn has_labels(&self) -> bool {
        !self.labels.is_empty()
    }

    pub(crate) fn is_label(&self, kind: SyntaxKind) -> bool {
        self.labels.contains(&kind)
    }

    /// The name a name token's text stands for (`None`: it is no name).
    pub(crate) fn name_of<'a>(&self, text: &'a str) -> Option<&'a str> {
        if !self.identifiers {
            return Some(text);
        }
        let name = text.split_once('[').map_or(text, |(n, _)| n);
        let mut chars = name.chars();
        (chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
            && chars.all(|c| c.is_alphanumeric() || c == '_'))
        .then_some(name)
    }

    /// The first own name token of the node, else the name its first child node ends with.
    pub(crate) fn head(&self, n: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
        let own = n
            .children_with_tokens()
            .find_map(|e| e.into_token().filter(|t| self.is_name_token(t.kind())));
        own.or_else(|| self.last_name(&n.first_child()?))
    }

    pub(crate) fn is_bare(&self) -> bool {
        self.bare
    }

    pub(crate) fn is_dynamic_name(&self, name: &str) -> bool {
        self.dynamic.contains(&name)
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

    /// The last name token of a dotted expression: `Optional` for `typing.Optional`.
    pub(crate) fn last_name(&self, n: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
        let start = n.text_range().start();
        let mut t = n.last_token();
        while let Some(tok) = t {
            if self.is_name_token(tok.kind()) {
                return Some(tok);
            }
            if tok.text_range().start() <= start {
                return None;
            }
            t = tok.prev_token();
        }
        None
    }

    pub(crate) fn is_operator(&self, kind: SyntaxKind) -> bool {
        self.operators
            .get(kind.index() as usize)
            .copied()
            .unwrap_or(false)
    }
}
