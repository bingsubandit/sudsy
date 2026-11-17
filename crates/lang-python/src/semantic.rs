//! The scope model of a file, for the rules that need to know what a name binds to: the shared
//! binder of `core_semantics::scopes` run over the tree with Python's table (which kinds open
//! scopes, declare names, refer to names or assign). Built on the first scope rule that runs on a
//! file and shared with the others (docs/formatting/python.md#scope-layer).
//!
//! Resolution is by name up the scope chain, whatever the order of the code: `x` read at the top of
//! a module finds an `x` assigned below it. What depends on the order (a redefinition before a
//! read, a read before an assignment in a function) the rules read off the positions of the
//! declarations and references of a symbol.

use crate::generated::kind as k;
use core_rules::FileContext;
use core_rules::checks::FileCache;
use core_semantics::scopes::{
    Access, Declares, Flags, Flow, Model, Place, Redirect, Row, ScopeId, Sel, Table, bind,
};
use core_syntax::{Grammar, SyntaxNode, TextRange, TextSize, grammars};
use rustc_hash::FxHashMap as HashMap;
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;
use std::sync::OnceLock;

const V: Flags = Flags::VALUE;

const fn declares(flags: Flags, place: Place, names: &'static [Sel]) -> Declares {
    Declares {
        flags,
        place,
        names,
        ..Declares::NONE
    }
}

const fn names(kind: &'static str, flags: Flags, place: Place, names: &'static [Sel]) -> Row {
    Row {
        kind,
        declares: Some(declares(flags, place, names)),
        ..Row::NONE
    }
}

const fn flows(kind: &'static str, children: &'static [(Sel, Flow)]) -> Row {
    Row {
        kind,
        children,
        ..Row::NONE
    }
}

const NAME: &[Sel] = &[Sel::Field("name")];
const OUTSIDE: &[(Sel, Flow)] = &[
    (Sel::Kind("Decorator"), Flow::Outside),
    (Sel::Kind("TypeParams"), Flow::Outside),
    (Sel::Kind("Arguments"), Flow::Outside),
    (Sel::Field("returns"), Flow::OutsideQuery),
];
const PATTERN_NAME: &[Sel] = &[Sel::Kind("Identifier")];
const THROUGH: &[(Sel, Flow)] = &[(Sel::All, Flow::Through)];
const TYPE_PARAM: Flags = V.union(Flags::TYPE).union(Flags::TYPE_PARAMETER);

/// What Python says to the binder (docs/formatting/python.md#scope-layer).
const ROWS: &[Row] = &[
    Row {
        kind: "ModModule",
        var_scope: true,
        ..Row::NONE
    },
    Row {
        kind: "StmtFunctionDef",
        var_scope: true,
        declares: Some(declares(V.union(Flags::FUNCTION), Place::Parent, NAME)),
        children: OUTSIDE,
        ..Row::NONE
    },
    Row {
        kind: "StmtClassDef",
        hidden: true,
        declares: Some(declares(V.union(Flags::CLASS), Place::Parent, NAME)),
        children: OUTSIDE,
        ..Row::NONE
    },
    Row {
        kind: "ExprLambda",
        var_scope: true,
        ..Row::NONE
    },
    Row {
        kind: "Parameter",
        declares: Some(declares(V.union(Flags::PARAMETER), Place::Scope, NAME)),
        children: &[(Sel::Field("annotation"), Flow::OutsideQuery)],
        ..Row::NONE
    },
    flows(
        "ParameterWithDefault",
        &[(Sel::AfterText("="), Flow::Outside)],
    ),
    names("TypeParamTypeVar", TYPE_PARAM, Place::Scope, NAME),
    names("TypeParamTypeVarTuple", TYPE_PARAM, Place::Scope, NAME),
    names("TypeParamParamSpec", TYPE_PARAM, Place::Scope, NAME),
    names(
        "Alias",
        V.union(Flags::IMPORT),
        Place::Scope,
        &[Sel::Field("asname"), Sel::Field("name")],
    ),
    names(
        "ExceptHandlerExceptHandler",
        V.union(Flags::CATCH),
        Place::Scope,
        NAME,
    ),
    names("PatternMatchAs", V, Place::Scope, PATTERN_NAME),
    names("PatternMatchStar", V, Place::Scope, PATTERN_NAME),
    names("PatternMatchMapping", V, Place::Scope, PATTERN_NAME),
    Row {
        kind: "StmtGlobal",
        redirect: Some(Redirect::Top),
        ..Row::NONE
    },
    Row {
        kind: "StmtNonlocal",
        redirect: Some(Redirect::Outer),
        ..Row::NONE
    },
    Row {
        kind: "ExprName",
        reference: true,
        implicit: Some((V, Place::Scope)),
        ..Row::NONE
    },
    Row {
        kind: "ExprStringLiteral",
        text: true,
        ..Row::NONE
    },
    flows("StmtDelete", &[(Sel::All, Flow::Delete)]),
    flows("StmtAssign", &[(Sel::ExceptLast, Flow::Write)]),
    flows("StmtAugAssign", &[(Sel::Field("left"), Flow::ReadWrite)]),
    flows(
        "StmtAnnAssign",
        &[
            (
                Sel::Field("left"),
                Flow::Cond(has_value, &Flow::Write, &Flow::WriteBare),
            ),
            (Sel::Field("annotation"), Flow::Query),
        ],
    ),
    flows("StmtFor", &[(Sel::Field("target"), Flow::Write)]),
    flows("StmtTypeAlias", &[(Sel::Field("name"), Flow::Write)]),
    flows("WithItem", &[(Sel::AfterText("as"), Flow::Write)]),
    flows("ExprNamed", &[(Sel::Field("left"), Flow::WriteVar)]),
    flows(
        "Comprehension",
        &[
            (Sel::Field("target"), Flow::Write),
            (
                Sel::Field("iter"),
                Flow::Cond(first_generator, &Flow::Outside, &Flow::Through),
            ),
        ],
    ),
    flows("ExprTuple", THROUGH),
    flows("ExprList", THROUGH),
    flows("ExprStarred", THROUGH),
    flows("Parenthesized", THROUGH),
    flows("ExprBinOp", &[(Sel::All, Flow::ThroughType)]),
    flows(
        "ExprSubscript",
        &[(
            Sel::All,
            Flow::Cond(is_generic, &Flow::Query, &Flow::ThroughType),
        )],
    ),
    flows(
        "Arguments",
        &[(
            Sel::First,
            Flow::Cond(is_cast, &Flow::Query, &Flow::Through),
        )],
    ),
];

/// Does the annotated assignment have a value?
fn has_value(n: &SyntaxNode) -> bool {
    n.children_with_tokens().any(|e| e.kind() == k::EQUAL)
}

/// The first `for` of a comprehension: its iterable is read outside the comprehension's scope.
fn first_generator(n: &SyntaxNode) -> bool {
    n.prev_sibling()
        .is_none_or(|p| p.kind() != k::COMPREHENSION)
}

/// The last name of a dotted expression: `Optional` for `typing.Optional`.
pub fn last_name(n: &SyntaxNode) -> Option<String> {
    last_name_token(n).map(|t| t.text().to_owned())
}

fn last_name_token(n: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
    let start = n.text_range().start();
    let mut t = n.last_token();
    while let Some(tok) = t {
        if tok.kind() == k::NAME {
            return Some(tok);
        }
        if tok.text_range().start() <= start {
            return None;
        }
        t = tok.prev_token();
    }
    None
}

/// Typing generics and builtin collections: a string in their subscript is a type.
const GENERICS: &[&str] = &[
    "AbstractSet",
    "AsyncContextManager",
    "AsyncGenerator",
    "AsyncIterable",
    "AsyncIterator",
    "Awaitable",
    "Callable",
    "ChainMap",
    "ClassVar",
    "Collection",
    "Concatenate",
    "Container",
    "ContextManager",
    "Coroutine",
    "Counter",
    "DefaultDict",
    "Deque",
    "Dict",
    "Final",
    "FrozenSet",
    "Generator",
    "Generic",
    "ItemsView",
    "Iterable",
    "Iterator",
    "KeysView",
    "List",
    "Mapping",
    "MappingView",
    "Match",
    "MutableMapping",
    "MutableSequence",
    "MutableSet",
    "NotRequired",
    "Optional",
    "OrderedDict",
    "Pattern",
    "Protocol",
    "ReadOnly",
    "Required",
    "Reversible",
    "Sequence",
    "Set",
    "Tuple",
    "Type",
    "TypeGuard",
    "TypeIs",
    "Union",
    "Unpack",
    "ValuesView",
    "frozenset",
    "dict",
    "list",
    "set",
    "tuple",
    "type",
];

fn is_generic(n: &SyntaxNode) -> bool {
    n.first_child()
        .and_then(|o| last_name_token(&o))
        .is_some_and(|name| GENERICS.contains(&name.text()))
}

/// The arguments of `cast(...)`: the first one is a type.
fn is_cast(n: &SyntaxNode) -> bool {
    n.parent()
        .and_then(|call| call.first_child())
        .and_then(|callee| last_name_token(&callee))
        .is_some_and(|name| name.text() == "cast")
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| Table::new(grammars::PYTHON.get(), ROWS, &[], &["Name", "Star"]))
}

/// The start and kind of a node.
type NodeKey = (u32, u32);

/// What a scope rule reads: the model and the facts of the file.
pub struct Cx {
    pub s: Model,
    pub root: SyntaxNode,
    pub g: &'static Grammar,
    /// `.pyi`: annotations are never evaluated.
    /// `__init__.py`: its `__all__` may name submodules.
    pub init: bool,
    /// The minor version of the target (`3.14`: 14).
    pub minor: u32,
    /// Syntax errors of quoted annotations: where and why.
    pub forward_errors: Vec<(TextRange, String)>,
    by_ref: OnceCell<HashMap<u32, usize>>,
    /// What [`Cx::qualified`] found, by the start and kind of the node asked about.
    qualified: RefCell<HashMap<NodeKey, Option<Rc<str>>>>,
}

thread_local! {
    static CACHE: FileCache<Option<Rc<Cx>>> = const { FileCache::new() };
    /// The file parsed last on this thread had syntax errors: its text's length and hash, and
    /// the target version's minor number.
    static NOTE: RefCell<Option<Note>> = const { RefCell::new(None) };
}

struct Note {
    len: usize,
    hash: u64,
    errors: bool,
    minor: u32,
}

fn hash(text: &str) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    h.write(text.as_bytes());
    h.finish()
}

/// What the parse of `text` found, for the rules that follow on this thread.
pub fn note_parse(text: &str, errors: bool, minor: u32) {
    NOTE.with(|n| {
        *n.borrow_mut() = Some(Note {
            len: text.len(),
            hash: hash(text),
            errors,
            minor,
        });
    });
}

/// `(errors, minor)` of the last parse when it was of `text`.
fn noted(text: &str) -> Option<(bool, u32)> {
    NOTE.with(|n| {
        let n = n.borrow();
        let n = n.as_ref()?;
        (n.len == text.len() && n.hash == hash(text)).then_some((n.errors, n.minor))
    })
}

/// The model of the file, built once and shared by the rules that run on it; `None` for a file
/// with syntax errors (the parser's diagnostic stands).
pub fn model(file: &FileContext<'_>) -> Option<Rc<Cx>> {
    // Every rule asks: the answer (also "none") is cached, so the text is hashed once per file.
    CACHE.with(|cache| {
        cache
            .get(file, || {
                let (errors, minor) = noted(file.text).unwrap_or((false, 14));
                let fatal = file
                    .root
                    .first_token()
                    .is_some_and(|t| t.kind() == k::UNKNOWN);
                if errors || fatal {
                    return None;
                }
                let name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let mut cx = Cx {
                    s: bind(file.root, file.grammar, table()),
                    root: file.root.clone(),
                    g: grammars::PYTHON.get(),
                    init: name == "__init__.py",
                    minor,
                    forward_errors: Vec::new(),
                    by_ref: OnceCell::new(),
                    qualified: RefCell::default(),
                };
                cx.forward_references();
                Some(Rc::new(cx))
            })
            .as_ref()
            .clone()
    })
}

impl Cx {
    /// The child node in the body item `label` (a field of the grammar).
    pub fn slot(&self, n: &SyntaxNode, label: &str) -> Option<SyntaxNode> {
        self.g.slot(n, label)?.into_node()
    }

    /// The span of `n` without the trivia around it.
    pub fn range(&self, n: &SyntaxNode) -> TextRange {
        core_rules::tree::significant_range(n, self.g)
    }

    /// The reference whose name starts at `start`.
    pub fn ref_at(&self, start: TextSize) -> Option<&core_semantics::scopes::Reference> {
        let map = self.by_ref.get_or_init(|| {
            (self.s.refs.iter().enumerate())
                .map(|(i, r)| (u32::from(r.range.start()), i))
                .collect()
        });
        map.get(&u32::from(start)).map(|i| &self.s.refs[*i])
    }

    /// Is the scope the body of a function or a lambda (code that runs when called)?
    pub fn is_function(&self, scope: ScopeId) -> bool {
        matches!(
            self.s.scope(scope).node.kind(),
            k::STMT_FUNCTION_DEF | k::EXPR_LAMBDA
        )
    }

    /// Does a function or lambda lie between the reference's scope and `scope` (the reference
    /// runs after the code of `scope` has run)?
    pub fn deferred(&self, from: ScopeId, scope: ScopeId) -> bool {
        self.s
            .ancestors(from)
            .take_while(|s| *s != scope)
            .any(|s| self.is_function(s))
    }

    /// Quoted annotations (and the strings in typing generics and `cast`) name things: the
    /// names in them are references from the scope the string is written in.
    fn forward_references(&mut self) {
        let texts = std::mem::take(&mut self.s.texts);
        for t in texts {
            let node = t.node(&self.root);
            crate::forward::references(self, &node, t.scope);
        }
    }

    /// The name a reference to a dotted name stands for through imports: `typing.Literal` for
    /// `Literal` imported from typing, `print` for the builtin; `None` for anything else.
    pub fn qualified(&self, node: &SyntaxNode) -> Option<String> {
        let key = (u32::from(node.text_range().start()), node.kind().0);
        if let Some(found) = self.qualified.borrow().get(&key) {
            return found.as_deref().map(str::to_owned);
        }
        let found = self.qualify(node);
        self.qualified
            .borrow_mut()
            .insert(key, found.as_deref().map(Rc::from));
        found
    }

    fn qualify(&self, node: &SyntaxNode) -> Option<String> {
        match node.kind() {
            k::EXPR_NAME => {
                let r = self.ref_at(self.range(node).start())?;
                match r.symbol {
                    Some(sym) => {
                        let d = self.s.symbol(sym).decls.last()?;
                        crate::imports::target(self, d)
                    }
                    None => crate::names::is_builtin(&r.name).then(|| r.name.to_string()),
                }
            }
            k::EXPR_ATTRIBUTE => {
                let object = self.slot(node, "object")?;
                let member = self.slot(node, "member")?;
                Some(format!(
                    "{}.{}",
                    self.qualified(&object)?,
                    last_name(&member)?
                ))
            }
            k::PARENTHESIZED => self.qualified(&node.first_child()?),
            _ => None,
        }
    }
}

impl Cx {
    pub(crate) fn add_reference(
        &mut self,
        name: &str,
        range: TextRange,
        at: &SyntaxNode,
        scope: ScopeId,
    ) {
        self.s.add_reference(
            name,
            range,
            (at.text_range(), at.kind()),
            Access::READ.union(Access::TYPE),
            scope,
        );
    }
}
