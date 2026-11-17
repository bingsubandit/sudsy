//! What a declaration of the model is, in ruff's terms: the kind of binding, where it takes
//! effect, which branch it is in, and whether anything read it. The model resolves by name; the
//! order of the code (a read between two assignments) is read off the positions here.

use crate::generated::kind as k;
use crate::imports::{self, Form, Import};
use crate::semantic::Cx;
use core_semantics::scopes::{Decl, Flags, SymbolId};
use core_syntax::{SyntaxNode, TextRange, TextSize};
use std::cell::OnceCell;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Import(Form),
    FunctionDef,
    ClassDef,
    Assignment,
    NamedExpr,
    /// `x: int` with no value.
    Annotation,
    Argument,
    LoopVar,
    WithItem,
    /// `except E as name`.
    Except,
    Comprehension,
    TypeParam,
    /// A name a `global` statement adds to the module.
    Global,
}

impl Kind {
    pub fn is_import(self) -> bool {
        matches!(self, Kind::Import(_))
    }
}

/// The compound statement (or clause) a node is in, within its scope: bindings of different
/// branches do not redefine each other. `None` outside any.
pub type Branch = Option<(TextSize, u8)>;

pub struct Binding {
    pub kind: Kind,
    /// From here on a read finds the binding (the end of the statement that makes it).
    pub effect: TextSize,
    pub branch: Branch,
    /// Part of a tuple or list target whose value is not a literal sequence.
    pub unpacked: bool,
    pub import: Option<Import>,
    /// What a finding points at: the name, or the whole dotted name of `import a.b`.
    pub range: TextRange,
}

/// The statement or clause a name of a target belongs to, through tuples, lists and parentheses;
/// whether it is inside a tuple or list.
fn host(binding: &SyntaxNode) -> Option<(SyntaxNode, bool)> {
    let mut nested = false;
    for a in binding.ancestors().skip(1) {
        match a.kind() {
            k::EXPR_TUPLE | k::EXPR_LIST => nested = true,
            k::PARENTHESIZED | k::EXPR_STARRED => {}
            _ => return Some((a, nested)),
        }
    }
    None
}

fn is_stmt(cx: &Cx, n: &SyntaxNode) -> bool {
    cx.g.name(n.kind()).starts_with("Stmt")
}

pub fn branch(cx: &Cx, n: &SyntaxNode) -> Branch {
    let mut child = n.clone();
    for a in n.ancestors().skip(1) {
        let start = a.text_range().start();
        match a.kind() {
            k::ELIF_ELSE_CLAUSE | k::MATCH_CASE | k::EXCEPT_HANDLER_EXCEPT_HANDLER => {
                return Some((start, 0));
            }
            k::STMT_IF | k::STMT_WITH if is_stmt(cx, &child) => return Some((start, 0)),
            k::STMT_TRY if is_stmt(cx, &child) => {
                let after_finally = a
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == k::FINALLY)
                    .is_some_and(|f| child.text_range().start() >= f.text_range().end());
                return Some((start, if after_finally { 2 } else { 0 }));
            }
            k::STMT_FUNCTION_DEF
            | k::STMT_CLASS_DEF
            | k::EXPR_LAMBDA
            | k::MOD_MODULE
            | k::EXPR_LIST_COMP
            | k::EXPR_SET_COMP
            | k::EXPR_DICT_COMP
            | k::EXPR_GENERATOR => return None,
            _ => {}
        }
        child = a;
    }
    None
}

/// Describes a declaration.
pub fn binding(cx: &Cx, d: &Decl) -> Binding {
    let node = d.node(&cx.root);
    let name_start = d.name.start();
    let f = d.flags;
    let mut b = Binding {
        kind: Kind::Assignment,
        effect: name_start,
        branch: branch(cx, &d.binding(&cx.root)),
        unpacked: false,
        import: None,
        range: d.name,
    };
    if f.contains(Flags::IMPORT) {
        b.import = imports::describe(cx, d);
        b.kind = Kind::Import(b.import.as_ref().map_or(Form::Import, |i| i.form));
        b.effect = node.text_range().end();
        if b.kind == Kind::Import(Form::Submodule)
            && let Some(name) = cx.slot(&node, "name")
        {
            b.range = cx.range(&name);
        }
    } else if f.contains(Flags::FUNCTION) {
        b.kind = Kind::FunctionDef;
    } else if f.contains(Flags::CLASS) {
        b.kind = Kind::ClassDef;
    } else if f.contains(Flags::PARAMETER) {
        b.kind = Kind::Argument;
    } else if f.contains(Flags::CATCH) {
        b.kind = Kind::Except;
    } else if f.contains(Flags::TYPE_PARAMETER) {
        b.kind = Kind::TypeParam;
    } else if f.contains(Flags::REDIRECT) {
        b.kind = Kind::Global;
    } else if node.kind() == k::EXPR_NAME {
        let Some((h, nested)) = host(&node) else {
            return b;
        };
        b.effect = h.text_range().end();
        match h.kind() {
            k::STMT_ANN_ASSIGN if f.contains(Flags::BARE) => b.kind = Kind::Annotation,
            k::STMT_ASSIGN => {
                let literal = h
                    .children()
                    .last()
                    .is_some_and(|v| matches!(v.kind(), k::EXPR_TUPLE | k::EXPR_LIST));
                b.unpacked = nested && !literal;
            }
            k::STMT_FOR => {
                b.kind = Kind::LoopVar;
                b.unpacked = nested;
                b.effect = cx
                    .slot(&h, "iter")
                    .map_or(b.effect, |i| i.text_range().end());
            }
            k::COMPREHENSION => {
                b.kind = Kind::Comprehension;
                b.unpacked = nested;
                // The element comes first in the text and reads the target.
                b.effect = h.parent().map_or(b.effect, |c| c.text_range().start());
            }
            k::WITH_ITEM => {
                b.kind = Kind::WithItem;
                b.unpacked = nested;
            }
            k::EXPR_NAMED => b.kind = Kind::NamedExpr,
            _ => {}
        }
    }
    b
}

/// The bindings of the declarations of the symbols, each worked out when first asked for.
pub struct Bindings<'a> {
    cx: &'a Cx,
    v: Vec<OnceCell<Vec<Binding>>>,
}

impl<'a> Bindings<'a> {
    pub fn new(cx: &'a Cx) -> Self {
        Bindings {
            cx,
            v: cx.s.symbol_ids().map(|_| OnceCell::new()).collect(),
        }
    }

    /// One for each declaration of the symbol, in order.
    pub fn get(&self, id: SymbolId) -> &[Binding] {
        self.v[id.0 as usize].get_or_init(|| {
            (self.cx.s.symbol(id).decls.iter())
                .map(|d| binding(self.cx, d))
                .collect()
        })
    }
}

/// A declaration that takes effect, with what it is and whether anything read it.
pub struct Live<'a> {
    pub decl: &'a Decl,
    pub b: &'a Binding,
    /// Read, or made by a statement of another scope (`global`), or inherited from the binding it
    /// replaces: a rebinding inherits what read the old one.
    pub used: bool,
}

/// The declarations of a symbol that take effect, in source order: an annotation without a value
/// does not replace a binding.
pub fn live<'a>(cx: &'a Cx, bs: &'a Bindings<'_>, id: SymbolId) -> Vec<Live<'a>> {
    let sym = cx.s.symbol(id);
    let mut out: Vec<Live<'a>> = Vec::with_capacity(sym.decls.len());
    for (d, b) in sym.decls.iter().zip(bs.get(id)) {
        if b.kind == Kind::Annotation && !out.is_empty() {
            continue;
        }
        let used = d.from != sym.scope;
        out.push(Live { decl: d, b, used });
    }
    let mut direct = vec![false; out.len()];
    for r in cx.s.refs_of(id).filter(|r| r.is_read()) {
        // A read that runs later than the code of the scope (a function, an annotation) finds the
        // last binding; one before the first binding finds none of these.
        let i = if r.is_type() || cx.deferred(r.scope, sym.scope) {
            out.len() - 1
        } else {
            match out.iter().rposition(|l| l.b.effect <= r.range.start()) {
                Some(i) => i,
                None => continue,
            }
        };
        direct[i] = true;
    }
    let mut inherited = false;
    for (l, d) in out.iter_mut().zip(direct) {
        inherited |= d || l.used;
        l.used = inherited;
    }
    out
}

/// `_`, `__`, `_ignored`: the names ruff's `dummy-variable-rgx` accepts.
pub fn is_dummy(name: &str) -> bool {
    name.starts_with('_')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && (name.bytes().all(|b| b == b'_') || name.ends_with(|c: char| c.is_ascii_alphanumeric()))
}
