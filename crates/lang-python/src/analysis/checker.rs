//! The walk that fills the model: bindings in source order, function bodies and quoted or
//! `__future__` annotations after the code around them, as pyflakes and ruff read a file.

use super::model::{Found, Kind, Model, New, ScopeKind, is_dummy};
use super::names::is_builtin_class;
use super::{dot_format, formats, syntax};

mod lints;
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{
    self as ast, ExceptHandler, Expr, ExprContext, Pattern, PythonVersion, Stmt, name::Name,
};
use ruff_python_parser::semantic_errors::SemanticSyntaxChecker;
use ruff_python_parser::typing::{ParsedAnnotation, parse_type_annotation};
use ruff_text_size::{Ranged, TextRange};
use rustc_hash::FxHashSet;
use std::cell::RefCell;
use std::collections::VecDeque;

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
];

/// The names any `global` statement of the file declares.
struct Globals<'a>(FxHashSet<&'a str>);

impl<'a> Visitor<'a> for Globals<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::Global(g) = stmt {
            self.0.extend(g.names.iter().map(ast::Identifier::as_str));
        }
        visitor::walk_stmt(self, stmt);
    }
}

/// A string, bytes or number literal, a collection or comprehension (not a generator), or a
/// tuple of constants: what `is` never matches by identity.
fn is_fresh(e: &Expr) -> bool {
    fn constant(e: &Expr) -> bool {
        match e {
            Expr::Tuple(t) => t.elts.iter().all(constant),
            _ => e.is_literal_expr(),
        }
    }
    match e {
        Expr::List(_)
        | Expr::ListComp(_)
        | Expr::Set(_)
        | Expr::SetComp(_)
        | Expr::Dict(_)
        | Expr::DictComp(_) => true,
        Expr::Tuple(_) => constant(e),
        _ => {
            e.is_literal_expr()
                && !matches!(
                    e,
                    Expr::NoneLiteral(_) | Expr::BooleanLiteral(_) | Expr::EllipsisLiteral(_)
                )
        }
    }
}

/// How the next name in a target binds.
#[derive(Clone, Copy)]
pub(super) struct Store {
    kind: Kind,
    /// The value assigned is a tuple or list: its targets are not an unpacking.
    literal: bool,
}

enum Work<'a> {
    Function(&'a ast::StmtFunctionDef),
    Lambda(&'a ast::ExprLambda),
    Annotation(&'a Expr),
    Quoted(Box<ParsedAnnotation>),
}

/// Code to read after the module: the scopes around it and the state it was met in.
struct Task<'a> {
    work: Work<'a>,
    stack: Vec<usize>,
    branch: u32,
    cond_depth: u32,
    protocol: bool,
}

pub struct Checker<'a> {
    pub(super) m: Model<'a>,
    pub(super) src: &'a str,
    pub(super) stub: bool,
    init: bool,
    pub(super) target: PythonVersion,
    deferred: VecDeque<Task<'a>>,
    pub(super) future_annotations: bool,
    /// Reading an annotation: a string is a forward reference.
    annotation: bool,
    /// Reading a deferred annotation: a module-level name wins over a nearer local one.
    lazy: bool,
    store: Store,
    tuple_depth: u32,
    /// Inside the body of a `Protocol` class (or a method of one).
    protocol: bool,
    /// A `...` is not a placeholder to remove here: an abstract method or a protocol.
    ellipsis_ok: bool,
    /// The function whose body is read, `""` elsewhere.
    fn_name: &'a str,
    /// An expression of the test of an `if` that only raises.
    exception_check: bool,
    pub(super) loop_depth: u32,
    cond_depth: u32,
    /// Inside the body of a `try` that handles `NameError`.
    name_error: u32,
    /// Names a `global` statement anywhere declares: defined module-wide, whatever the order.
    globals: FxHashSet<&'a str>,
    pub(super) sem: SemanticSyntaxChecker,
    pub(super) sem_errors: RefCell<Vec<ruff_python_parser::semantic_errors::SemanticSyntaxError>>,
}

impl<'a> Checker<'a> {
    pub fn new(src: &'a str, path: &std::path::Path, target: PythonVersion) -> Self {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        Checker {
            m: Model::new(),
            src,
            stub: path.extension().is_some_and(|e| e == "pyi"),
            init: name == "__init__.py",
            target,
            deferred: VecDeque::new(),
            future_annotations: false,
            annotation: false,
            lazy: false,
            store: Store {
                kind: Kind::Assignment,
                literal: false,
            },
            tuple_depth: 0,
            protocol: false,
            ellipsis_ok: false,
            fn_name: "",
            exception_check: false,
            loop_depth: 0,
            cond_depth: 0,
            name_error: 0,
            globals: FxHashSet::default(),
            sem: SemanticSyntaxChecker::new(),
            sem_errors: RefCell::new(Vec::new()),
        }
    }

    /// Reads the module and everything deferred, then reports what the whole file shows.
    pub fn run(&mut self, body: &'a [Stmt]) {
        let mut globals = Globals(FxHashSet::default());
        globals.visit_body(body);
        self.globals = globals.0;
        syntax::module_imports(&mut self.m, body);
        self.visit_body(body);
        while let Some(task) = self.deferred.pop_front() {
            self.m.stack = task.stack;
            self.m.branch = task.branch;
            self.cond_depth = task.cond_depth;
            (self.loop_depth, self.tuple_depth, self.name_error) = (0, 0, 0);
            self.annotation = false;
            self.protocol = task.protocol;
            self.fn_name = "";
            self.ellipsis_ok = false;
            self.lazy = matches!(task.work, Work::Annotation(_) | Work::Quoted(_));
            match task.work {
                Work::Function(f) => {
                    self.fn_name = f.name.as_str();
                    self.ellipsis_ok = self.protocol || self.is_abstract(f);
                    self.m.push_scope(ScopeKind::Function {
                        is_async: f.is_async,
                    });
                    self.bind_parameters(&f.parameters);
                    self.visit_body(&f.body);
                    self.m.pop_scope();
                }
                Work::Lambda(l) => {
                    self.m.push_scope(ScopeKind::Lambda);
                    if let Some(p) = &l.parameters {
                        self.bind_parameters(p);
                    }
                    self.visit_expr(&l.body);
                    self.m.pop_scope();
                }
                Work::Annotation(e) => self.annotation_now(e),
                Work::Quoted(parsed) => self.quoted(parsed.expression()),
            }
        }
        super::finish::finish(&mut self.m, self.init);
    }

    fn defer(&mut self, work: Work<'a>) {
        self.deferred.push_back(Task {
            work,
            stack: self.m.stack.clone(),
            branch: self.m.branch,
            cond_depth: self.cond_depth,
            protocol: self.protocol,
        });
    }

    fn annotations_deferred(&self) -> bool {
        self.future_annotations || self.stub || self.target >= PythonVersion::PY314
    }

    fn annotation_now(&mut self, expr: &'a Expr) {
        let saved = std::mem::replace(&mut self.annotation, true);
        self.visit_expr(expr);
        self.annotation = saved;
    }

    /// The qualified name an expression refers to through imports (`typing.Literal`), or a
    /// builtin's own name.
    pub(super) fn qualified(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Name(n) => match self.m.lookup(n.id.as_str()) {
                Found::Bound(b) => {
                    let b = &self.m.bindings[b];
                    (b.kind.is_import() || b.kind == Kind::Global && false)
                        .then(|| b.target.to_string())
                }
                Found::Builtin => Some(n.id.to_string()),
                Found::Missing | Found::Unbound(_) => None,
            },
            Expr::Attribute(a) => self.qualified(&a.value).map(|q| format!("{q}.{}", a.attr)),
            _ => None,
        }
    }

    pub(super) fn is_builtin_name(&self, name: &str) -> bool {
        matches!(self.m.lookup(name), Found::Builtin)
    }

    fn load(&mut self, name: &str, range: TextRange) {
        if self.lazy
            && let Some(&b) = self.m.scopes[0].bindings.get(name)
            && self.m.bindings[b].kind != Kind::Annotation
        {
            return self.m.mark_used(b, range);
        }
        match self.m.lookup(name) {
            Found::Bound(b) => self.m.mark_used(b, range),
            Found::Builtin => {}
            Found::Missing => self.undefined(name, range, false),
            Found::Unbound(b) => {
                self.m.bindings[b].used = true;
                self.undefined(name, range, true);
            }
        }
    }

    fn undefined(&mut self, name: &str, range: TextRange, unbound: bool) {
        let cur = self.m.current();
        let class = self.m.scopes[cur].kind == ScopeKind::Class;
        let in_class = self
            .m
            .stack
            .iter()
            .any(|&s| self.m.scopes[s].kind == ScopeKind::Class);
        if (class && matches!(name, "__module__" | "__qualname__"))
            || (in_class && name == "__class__")
            || (self.init && name == "__path__")
            || self.globals.contains(name)
        {
            return;
        }
        let mut starred = false;
        for &s in self.m.stack.iter().filter(|_| !unbound) {
            for star in &mut self.m.scopes[s].stars {
                star.2 = true;
                starred = true;
            }
        }
        if starred {
            self.m
                .hit("undefined-local-with-import-star-usage", range, name);
        } else if self.name_error == 0 {
            self.m.hit("undefined-name", range, name);
        }
    }

    fn delete(&mut self, name: &'a str, range: TextRange) {
        let cur = self.m.current();
        if self.m.scopes[cur].declared.contains_key(name) {
            return;
        }
        if let (true, Some(&b)) = (self.cond_depth > 0, self.m.scopes[cur].bindings.get(name)) {
            // Whether the branch runs is unknown: the binding stays, counted as read.
            self.m.bindings[b].used = true;
            return;
        }
        match self.m.scopes[cur].bindings.remove(name) {
            // Only the scope's own binding can be deleted, builtins and outer names included.
            None => self.undefined(name, range, true),
            Some(b) if self.m.bindings[b].kind == Kind::Annotation => {
                self.undefined(name, range, true)
            }
            Some(_) => {}
        }
    }

    fn bind_store(&mut self, name: &'a str, range: TextRange) {
        syntax::ambiguous_name(&mut self.m, name, range);
        let mut new = New::of(self.store.kind, range);
        new.unpacked = self.tuple_depth > 0 && !self.store.literal;
        self.m.add(name, new);
    }

    /// Visits an assignment target; its names bind as `kind`.
    fn store_target(&mut self, target: &'a Expr, kind: Kind, literal: bool) {
        let saved = (self.store, self.tuple_depth);
        self.store = Store { kind, literal };
        self.tuple_depth = 0;
        self.visit_expr(target);
        (self.store, self.tuple_depth) = saved;
    }

    fn bind_parameters(&mut self, p: &'a ast::Parameters) {
        for param in p.iter().map(|p| p.as_parameter()) {
            syntax::ambiguous_name(&mut self.m, param.name.as_str(), param.name.range());
            self.m.add(
                param.name.as_str(),
                New::of(Kind::Argument, param.name.range()),
            );
        }
    }

    fn function_def(&mut self, f: &'a ast::StmtFunctionDef) {
        let mut overload = false;
        for d in &f.decorator_list {
            self.visit_decorator(d);
            overload |= self.qualified(&d.expression).is_some_and(|q| {
                matches!(q.as_str(), "typing.overload" | "typing_extensions.overload")
            });
        }
        syntax::ambiguous_def(
            &mut self.m,
            f.name.as_str(),
            f.name.range(),
            "ambiguous-function-name",
        );
        let mut new = New::of(Kind::FunctionDef, f.name.range());
        new.overload = overload;
        self.m.add(f.name.as_str(), new);
        let type_scope = f.type_params.as_deref().map(|tp| self.type_params(tp));
        // Defaults are evaluated here, annotations here or after the module.
        self.lint_defaults(&f.parameters);
        for p in f.parameters.iter_non_variadic_params() {
            if let Some(d) = p.default.as_deref() {
                self.visit_expr(d);
            }
        }
        for p in f.parameters.iter().map(|p| p.as_parameter()) {
            if let Some(a) = p.annotation.as_deref() {
                self.visit_annotation(a);
            }
        }
        if let Some(r) = f.returns.as_deref() {
            self.visit_annotation(r);
        }
        self.defer(Work::Function(f));
        if type_scope.is_some() {
            self.m.pop_scope();
        }
    }

    /// Opens the scope of a type parameter list and binds its names.
    fn type_params(&mut self, tp: &'a ast::TypeParams) -> usize {
        let id = self.m.push_scope(ScopeKind::Type);
        for p in &tp.type_params {
            let name = match p {
                ast::TypeParam::TypeVar(t) => &t.name,
                ast::TypeParam::TypeVarTuple(t) => &t.name,
                ast::TypeParam::ParamSpec(t) => &t.name,
            };
            if let Some(b) = self
                .m
                .add(name.as_str(), New::of(Kind::TypeParam, name.range()))
            {
                self.m.bindings[b].used = true;
            }
        }
        // Bounds and defaults are read lazily, as the annotations are.
        for p in &tp.type_params {
            let (bound, default) = match p {
                ast::TypeParam::TypeVar(t) => (t.bound.as_deref(), t.default.as_deref()),
                ast::TypeParam::TypeVarTuple(t) => (None, t.default.as_deref()),
                ast::TypeParam::ParamSpec(t) => (None, t.default.as_deref()),
            };
            for e in bound.into_iter().chain(default) {
                self.defer(Work::Annotation(e));
            }
        }
        id
    }

    fn class_def(&mut self, c: &'a ast::StmtClassDef) {
        self.lint_class(c);
        for d in &c.decorator_list {
            self.visit_decorator(d);
        }
        let type_scope = c.type_params.as_deref().map(|tp| self.type_params(tp));
        if let Some(a) = &c.arguments {
            self.visit_arguments(a);
        }
        self.compound(&c.body);
        self.m.push_scope(ScopeKind::Class);
        let saved = std::mem::replace(&mut self.loop_depth, 0);
        let protocol = self.is_protocol(c);
        let outer = (self.protocol, self.ellipsis_ok);
        (self.protocol, self.ellipsis_ok) = (protocol, protocol);
        self.visit_body(&c.body);
        (self.protocol, self.ellipsis_ok) = outer;
        self.loop_depth = saved;
        self.m.pop_scope();
        if type_scope.is_some() {
            self.m.pop_scope();
        }
        syntax::ambiguous_def(
            &mut self.m,
            c.name.as_str(),
            c.name.range(),
            "ambiguous-class-name",
        );
        self.m
            .add(c.name.as_str(), New::of(Kind::ClassDef, c.name.range()));
    }

    fn import(&mut self, i: &'a ast::StmtImport) {
        for a in &i.names {
            let full = a.name.as_str();
            let (name, range, kind, target) = match &a.asname {
                Some(asname) => (asname.as_str(), asname.range(), Kind::Import, full),
                None => {
                    let first = full.split('.').next().unwrap_or(full);
                    let kind = if first.len() < full.len() {
                        Kind::SubmoduleImport
                    } else {
                        Kind::Import
                    };
                    (first, a.name.range(), kind, first)
                }
            };
            let mut new = New::of(kind, range);
            new.names = Some((full.into(), target.into()));
            new.explicit_export = a.asname.as_ref().is_some_and(|n| n.as_str() == full);
            self.m.add(name, new);
        }
    }

    fn import_from(&mut self, i: &'a ast::StmtImportFrom, stmt: &Stmt) {
        let module = i.module.as_ref().map_or("", |m| m.as_str());
        let dots = ".".repeat(i.level as usize);
        let future = i.level == 0 && module == "__future__";
        for a in &i.names {
            if a.name.as_str() == "*" {
                let cur = self.m.current();
                if cur != 0 {
                    self.m.hit(
                        "undefined-local-with-nested-import-star-usage",
                        stmt.range(),
                        format!("{dots}{module}"),
                    );
                }
                self.m.hit(
                    "undefined-local-with-import-star",
                    stmt.range(),
                    format!("{dots}{module}"),
                );
                self.m.scopes[cur]
                    .stars
                    .push((format!("{dots}{module}"), stmt.range(), false));
                continue;
            }
            let full = format!("{dots}{module}.{}", a.name);
            let local = a.asname.as_ref().unwrap_or(&a.name);
            if future {
                self.future_annotations |= a.name.as_str() == "annotations";
                if let Some(b) = self
                    .m
                    .add(local.as_str(), New::of(Kind::FutureImport, local.range()))
                {
                    self.m.bindings[b].used = true;
                }
                continue;
            }
            let mut new = New::of(Kind::FromImport, local.range());
            new.names = Some((full.as_str().into(), full.as_str().into()));
            new.explicit_export = a
                .asname
                .as_ref()
                .is_some_and(|n| n.as_str() == a.name.as_str());
            self.m.add(local.as_str(), new);
        }
    }

    fn declare(&mut self, names: &'a [ast::Identifier], is_global: bool) {
        let cur = self.m.current();
        for n in names {
            let name = n.as_str();
            syntax::ambiguous_name(&mut self.m, name, n.range());
            if cur == 0 {
                continue;
            }
            self.m.scopes[cur]
                .declared
                .insert(name, (is_global, n.range()));
            if is_global {
                if !self.m.scopes[0].bindings.contains_key(name) {
                    self.m.add(name, New::of(Kind::Global, n.range()));
                }
            } else if let Found::Bound(b) = self.m.lookup(name) {
                self.m.bindings[b].used = true;
            }
        }
    }

    /// Adds the names of the strings an assignment to `__all__` lists (ruff keeps those of earlier
    /// assignments too).
    fn exports(&mut self, value: &Expr) {
        fn collect(e: &Expr, out: &mut Vec<(String, TextRange)>) {
            match e {
                Expr::List(l) => l.elts.iter().for_each(|e| collect(e, out)),
                Expr::Tuple(t) => t.elts.iter().for_each(|e| collect(e, out)),
                Expr::BinOp(b) if b.op == ast::Operator::Add => {
                    collect(&b.left, out);
                    collect(&b.right, out);
                }
                Expr::StringLiteral(s) => out.push((s.value.to_str().to_string(), s.range())),
                _ => {}
            }
        }
        let mut names = Vec::new();
        collect(value, &mut names);
        let exports = &mut self.m.scopes[0].exports;
        exports.append(&mut names);
    }

    fn compound(&mut self, body: &'a [Stmt]) {
        syntax::compound_header(&mut self.m, self.src, body);
    }

    fn comprehension(
        &mut self,
        elts: &[&'a Expr],
        generators: &'a [ast::Comprehension],
        is_gen: bool,
    ) {
        let Some(first) = generators.first() else {
            return;
        };
        self.visit_expr(&first.iter);
        let is_async = generators.iter().any(|g| g.is_async);
        self.m.push_scope(ScopeKind::Generator { is_gen, is_async });
        for (i, g) in generators.iter().enumerate() {
            if i > 0 {
                self.visit_expr(&g.iter);
            }
            self.store_target(&g.target, Kind::Comprehension, false);
            for cond in &g.ifs {
                self.visit_expr(cond);
            }
        }
        for e in elts {
            self.visit_expr(e);
        }
        self.m.pop_scope();
    }

    fn string_annotation(&mut self, s: &'a ast::ExprStringLiteral) {
        match parse_type_annotation(s, self.src) {
            Ok(parsed) => self.defer(Work::Quoted(Box::new(parsed))),
            Err(e) => self
                .m
                .hit("forward-annotation-syntax-error", s.range(), e.to_string()),
        }
    }

    /// Reads the expression of a quoted annotation (it is not part of the AST being walked).
    fn quoted(&mut self, expr: &Expr) {
        Quoted { ck: self }.visit_expr(expr);
    }

    fn is_type(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(n) => self.is_builtin_name(n.id.as_str()) && is_builtin_class(n.id.as_str()),
            Expr::Call(c) => {
                matches!(&*c.func, Expr::Name(f) if f.id.as_str() == "type" && self.is_builtin_name("type"))
            }
            _ => false,
        }
    }

    /// E721 (a type compared with `==`) and F632 (`is` with a literal or a fresh collection).
    fn compare(&mut self, c: &ast::ExprCompare) {
        let mut operands = c.operands.iter();
        let Some(mut left) = operands.next() else {
            return;
        };
        let mut types = false;
        for (op, right) in c.ops.iter().zip(operands) {
            match op {
                ast::CmpOp::Eq | ast::CmpOp::NotEq => {
                    types |= self.is_type(left) || self.is_type(right)
                }
                ast::CmpOp::Is | ast::CmpOp::IsNot if is_fresh(left) || is_fresh(right) => {
                    let range = TextRange::new(left.start(), right.end());
                    self.m.hit("is-literal", range, "");
                }
                _ => {}
            }
            left = right;
        }
        if types {
            self.m.hit("type-comparison", c.range(), "");
        }
    }

    fn handles_name_error(handlers: &[ExceptHandler]) -> bool {
        handlers.iter().any(|h| {
            let ExceptHandler::ExceptHandler(h) = h;
            match h.type_.as_deref() {
                Some(Expr::Name(n)) => n.id.as_str() == "NameError",
                Some(Expr::Tuple(t)) => t
                    .elts
                    .iter()
                    .any(|e| matches!(e, Expr::Name(n) if n.id.as_str() == "NameError")),
                _ => false,
            }
        })
    }

    fn try_stmt(&mut self, t: &'a ast::StmtTry) {
        let saved = self.m.branch;
        let handles = Self::handles_name_error(&t.handlers);
        let body_branch = self.m.new_branch();
        self.name_error += u32::from(handles);
        self.compound(&t.body);
        self.visit_body(&t.body);
        self.name_error -= u32::from(handles);
        // `del` in a handler or the `else` is conditional; in the body and `finally` it is not.
        self.cond_depth += 1;
        for (i, h) in t.handlers.iter().enumerate() {
            let ExceptHandler::ExceptHandler(eh) = h;
            if eh.type_.is_none() {
                if i + 1 < t.handlers.len() {
                    self.m.hit("default-except-not-last", eh.range(), "");
                }
                if !eh
                    .body
                    .iter()
                    .any(|s| matches!(s, Stmt::Raise(r) if r.exc.is_none()))
                {
                    let at = eh.start();
                    self.m.hit("bare-except", TextRange::at(at, 6.into()), "");
                }
            }
            self.m.new_branch();
            self.compound(&eh.body);
            self.lint_handler(eh, t.is_star);
            self.visit_except_handler(h);
        }
        self.m.branch = body_branch;
        self.compound(&t.orelse);
        self.visit_body(&t.orelse);
        self.cond_depth -= 1;
        self.m.new_branch();
        self.compound(&t.finalbody);
        self.visit_body(&t.finalbody);
        self.m.branch = saved;
    }

    fn if_stmt(&mut self, i: &'a ast::StmtIf) {
        let raises = matches!(i.body[..], [Stmt::Raise(_)]);
        self.exception_check = raises;
        self.visit_expr(&i.test);
        self.exception_check = false;
        let saved = self.m.branch;
        self.cond_depth += 1;
        self.m.new_branch();
        self.compound(&i.body);
        self.visit_body(&i.body);
        for clause in &i.elif_else_clauses {
            self.m.new_branch();
            if let Some(t) = &clause.test {
                self.exception_check = raises;
                self.visit_expr(t);
                self.exception_check = false;
            }
            self.compound(&clause.body);
            self.visit_body(&clause.body);
        }
        self.cond_depth -= 1;
        self.m.branch = saved;
    }

    fn loop_body(&mut self, body: &'a [Stmt]) {
        self.loop_depth += 1;
        self.compound(body);
        self.visit_body(body);
        self.loop_depth -= 1;
    }
}

impl<'a> Visitor<'a> for Checker<'a> {
    fn visit_body(&mut self, body: &'a [Stmt]) {
        self.lint_body(body);
        visitor::walk_body(self, body);
    }

    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        self.lint_stmt(stmt);
        self.semantic_stmt(stmt);
        match stmt {
            Stmt::Global(g) => self.declare(&g.names, true),
            Stmt::Nonlocal(n) => self.declare(&n.names, false),
            Stmt::FunctionDef(f) => self.function_def(f),
            Stmt::ClassDef(c) => self.class_def(c),
            Stmt::Import(i) => self.import(i),
            Stmt::ImportFrom(i) => self.import_from(i, stmt),
            Stmt::Assign(a) => {
                if let ([Expr::Name(_)], Expr::Lambda(_)) = (&a.targets[..], &*a.value) {
                    self.m.hit("lambda-assignment", a.range(), "");
                }
                self.visit_expr(&a.value);
                let literal = matches!(&*a.value, Expr::Tuple(_) | Expr::List(_));
                for t in &a.targets {
                    self.store_target(t, Kind::Assignment, literal);
                    if self.m.current() == 0
                        && matches!(t, Expr::Name(n) if n.id.as_str() == "__all__")
                    {
                        self.exports(&a.value);
                    }
                }
            }
            Stmt::AugAssign(a) => {
                self.visit_expr(&a.value);
                if let Expr::Name(n) = &*a.target {
                    self.load(n.id.as_str(), n.range());
                    if self.m.current() == 0 && n.id.as_str() == "__all__" {
                        self.exports(&a.value);
                    }
                }
                self.store_target(&a.target, Kind::Assignment, false);
            }
            Stmt::AnnAssign(a) => {
                if let Some(v) = &a.value {
                    if matches!((&*a.target, &**v), (Expr::Name(_), Expr::Lambda(_)))
                        && self.m.scopes[self.m.current()].kind != ScopeKind::Class
                    {
                        self.m.hit("lambda-assignment", a.range(), "");
                    }
                    self.visit_expr(v);
                }
                self.visit_annotation(&a.annotation);
                let kind = if a.value.is_some() {
                    Kind::Assignment
                } else {
                    Kind::Annotation
                };
                self.store_target(&a.target, kind, false);
                if let (Some(v), Expr::Name(n)) = (&a.value, &*a.target)
                    && self.m.current() == 0
                    && n.id.as_str() == "__all__"
                {
                    self.exports(v);
                }
            }
            Stmt::For(f) => {
                self.visit_expr(&f.iter);
                self.store_target(&f.target, Kind::LoopVar, false);
                self.loop_body(&f.body);
                self.compound(&f.orelse);
                self.visit_body(&f.orelse);
            }
            Stmt::While(w) => {
                self.visit_expr(&w.test);
                self.cond_depth += 1;
                self.loop_body(&w.body);
                self.compound(&w.orelse);
                self.visit_body(&w.orelse);
                self.cond_depth -= 1;
            }
            Stmt::If(i) => self.if_stmt(i),
            Stmt::With(w) => {
                for item in &w.items {
                    self.visit_expr(&item.context_expr);
                    if let Some(v) = &item.optional_vars {
                        self.store_target(v, Kind::WithItem, false);
                    }
                }
                let saved = self.m.branch;
                self.m.new_branch();
                self.compound(&w.body);
                self.visit_body(&w.body);
                self.m.branch = saved;
            }
            Stmt::Try(t) => self.try_stmt(t),
            Stmt::Match(m) => {
                self.visit_expr(&m.subject);
                let saved = self.m.branch;
                for case in &m.cases {
                    self.m.new_branch();
                    self.compound(&case.body);
                    self.visit_match_case(case);
                }
                self.m.branch = saved;
            }
            Stmt::TypeAlias(t) => {
                if let Expr::Name(n) = &*t.name {
                    syntax::ambiguous_name(&mut self.m, n.id.as_str(), n.range());
                    self.m
                        .add(n.id.as_str(), New::of(Kind::Assignment, n.range()));
                }
                let scope = t.type_params.as_deref().map(|tp| self.type_params(tp));
                self.defer(Work::Annotation(&t.value));
                if scope.is_some() {
                    self.m.pop_scope();
                }
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_annotation(&mut self, expr: &'a Expr) {
        if self.annotations_deferred() {
            self.defer(Work::Annotation(expr));
        } else {
            self.annotation_now(expr);
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        self.lint_expr(expr);
        self.semantic_expr(expr);
        match expr {
            Expr::Name(n) => match n.ctx {
                ExprContext::Store => self.bind_store(n.id.as_str(), n.range()),
                ExprContext::Del => self.delete(n.id.as_str(), n.range()),
                _ => self.load(n.id.as_str(), n.range()),
            },
            Expr::Tuple(ast::ExprTuple {
                ctx: ExprContext::Store,
                ..
            })
            | Expr::List(ast::ExprList {
                ctx: ExprContext::Store,
                ..
            }) => {
                self.tuple_depth += 1;
                visitor::walk_expr(self, expr);
                self.tuple_depth -= 1;
            }
            Expr::Lambda(l) => {
                if let Some(p) = &l.parameters {
                    for d in p.iter_non_variadic_params() {
                        if let Some(d) = d.default.as_deref() {
                            self.visit_expr(d);
                        }
                    }
                    for p in p.iter().map(|p| p.as_parameter()) {
                        syntax::ambiguous_name(&mut self.m, p.name.as_str(), p.name.range());
                    }
                }
                self.defer(Work::Lambda(l));
            }
            Expr::FString(f) => {
                if f.value
                    .f_strings()
                    .all(|p| p.elements.interpolations().next().is_none())
                {
                    for part in f.value.f_strings() {
                        self.m.hit("f-string-missing-placeholders", part.range, "");
                    }
                }
                visitor::walk_expr(self, expr);
            }
            Expr::ListComp(c) => self.comprehension(&[&c.elt], &c.generators, false),
            Expr::SetComp(c) => self.comprehension(&[&c.elt], &c.generators, false),
            Expr::Generator(c) => self.comprehension(&[&c.elt], &c.generators, true),
            Expr::DictComp(c) => {
                let mut elts: Vec<&Expr> = c.key.as_deref().into_iter().collect();
                elts.push(&c.value);
                self.comprehension(&elts, &c.generators, false);
            }
            Expr::Named(n) => {
                self.visit_expr(&n.value);
                self.store_target(&n.target, Kind::NamedExpr, false);
            }
            Expr::StringLiteral(s) if self.annotation => self.string_annotation(s),
            Expr::Subscript(s) if self.annotation || self.is_generic(&s.value) => {
                self.annotated_subscript(s);
            }
            Expr::Call(c) => self.call(c, expr),
            Expr::Compare(c) => {
                self.compare(c);
                visitor::walk_expr(self, expr);
            }
            Expr::BinOp(b) => {
                if b.op == ast::Operator::RShift
                    && matches!(&*b.left, Expr::Name(n) if n.id.as_str() == "print")
                    && self.is_builtin_name("print")
                {
                    self.m.hit("invalid-print-syntax", b.range(), "");
                }
                formats::binop(&mut self.m, b);
                visitor::walk_expr(self, expr);
            }
            _ => visitor::walk_expr(self, expr),
        }
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        let ExceptHandler::ExceptHandler(h) = handler;
        if let Some(t) = &h.type_ {
            self.visit_expr(t);
        }
        let Some(name) = &h.name else {
            self.visit_body(&h.body);
            return;
        };
        let n = name.as_str();
        syntax::ambiguous_name(&mut self.m, n, name.range());
        let cur = self.m.current();
        let prev = self.m.scopes[cur].bindings.remove(n);
        let own = self.m.add(n, New::of(Kind::Except, name.range()));
        self.visit_body(&h.body);
        // Reads of a rebinding inside the handler do not count for the name the handler bound.
        if self.m.scopes[cur].bindings.remove(n).is_some()
            && own.is_some_and(|b| !self.m.bindings[b].used)
        {
            self.m.hit("unused-variable", name.range(), n);
        }
        if let Some(p) = prev {
            self.m.scopes[cur].bindings.insert(n, p);
        }
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        visitor::walk_pattern(self, pattern);
        let bound = match pattern {
            Pattern::MatchAs(a) => a.name.as_ref(),
            Pattern::MatchStar(s) => s.name.as_ref(),
            Pattern::MatchMapping(m) => m.rest.as_ref(),
            _ => None,
        };
        if let Some(name) = bound {
            self.m
                .add(name.as_str(), New::of(Kind::Assignment, name.range()));
        }
    }
}

impl<'a> Checker<'a> {
    fn call(&mut self, c: &'a ast::ExprCall, expr: &'a Expr) {
        if let Expr::Name(f) = &*c.func
            && f.id.as_str() == "locals"
            && self.is_builtin_name("locals")
        {
            let cur = self.m.current();
            self.m.scopes[cur].uses_locals = true;
        }
        dot_format::check(&mut self.m, c);
        let cast = !self.annotation
            && self
                .qualified(&c.func)
                .is_some_and(|q| matches!(q.as_str(), "typing.cast" | "typing_extensions.cast"));
        if self.annotation {
            // Arguments of a call inside an annotation are values, not types.
            self.visit_expr(&c.func);
            let saved = std::mem::replace(&mut self.annotation, false);
            self.visit_arguments(&c.arguments);
            self.annotation = saved;
        } else if cast {
            self.visit_expr(&c.func);
            for (i, a) in c.arguments.args.iter().enumerate() {
                match a {
                    Expr::StringLiteral(s) if i == 0 => self.string_annotation(s),
                    _ => self.visit_expr(a),
                }
            }
            for k in &c.arguments.keywords {
                self.visit_keyword(k);
            }
        } else {
            visitor::walk_expr(self, expr);
        }
    }

    /// Is the expression a typing generic (`Optional`, `Union`, `collections.abc.Mapping`, ...),
    /// whose subscript holds types even outside an annotation?
    fn is_generic(&self, expr: &Expr) -> bool {
        self.qualified(expr).is_some_and(|q| {
            let (module, name) = q.rsplit_once('.').unwrap_or(("", &q));
            match module {
                "typing" | "typing_extensions" => GENERICS.contains(&name),
                "collections.abc" | "collections" | "contextlib" => true,
                "" => matches!(
                    name,
                    "list" | "dict" | "set" | "frozenset" | "tuple" | "type"
                ),
                _ => false,
            }
        })
    }

    /// The slice of a typing generic holds types (so quoted names are forward references),
    /// `Literal[...]` values and `Annotated[T, ...]` a type in its first item only.
    fn annotated_subscript(&mut self, s: &'a ast::ExprSubscript) {
        let q = self.qualified(&s.value).unwrap_or_default();
        let typing = q.starts_with("typing");
        let form = q.rsplit('.').next().unwrap_or("");
        let outer = std::mem::replace(&mut self.annotation, false);
        self.visit_expr(&s.value);
        match (typing, form) {
            (true, "Literal") => self.visit_expr(&s.slice),
            (true, "Annotated") => match &*s.slice {
                Expr::Tuple(t) => {
                    for (i, e) in t.elts.iter().enumerate() {
                        self.annotation = outer && i == 0;
                        self.visit_expr(e);
                    }
                }
                other => {
                    self.annotation = outer;
                    self.visit_expr(other);
                }
            },
            _ => {
                self.annotation = true;
                self.visit_expr(&s.slice);
            }
        }
        self.annotation = outer;
    }
}

/// The expression of a quoted annotation, read at the end of the file.
struct Quoted<'c, 'a> {
    ck: &'c mut Checker<'a>,
}

impl<'f> Visitor<'f> for Quoted<'_, '_> {
    fn visit_expr(&mut self, expr: &'f Expr) {
        match expr {
            Expr::Name(n) if n.ctx == ExprContext::Load => {
                self.ck.load(n.id.as_str(), n.range());
            }
            Expr::StringLiteral(s) => {
                if let Ok(parsed) = parse_type_annotation(s, self.ck.src) {
                    self.ck.quoted(parsed.expression());
                } else {
                    self.ck
                        .m
                        .hit("forward-annotation-syntax-error", s.range(), "");
                }
            }
            Expr::Subscript(s) => {
                let form = self.ck.qualified(&s.value).unwrap_or_default();
                self.visit_expr(&s.value);
                if form.starts_with("typing") && form.ends_with(".Literal") {
                    return;
                }
                if form.starts_with("typing")
                    && form.ends_with(".Annotated")
                    && let Expr::Tuple(t) = &*s.slice
                    && let Some(first) = t.elts.first()
                {
                    self.visit_expr(first);
                    return;
                }
                self.visit_expr(&s.slice);
            }
            Expr::Call(c) => self.visit_expr(&c.func),
            _ => visitor::walk_expr(self, expr),
        }
    }
}

#[allow(dead_code)]
fn _unused(_: &Name) {
    let _ = is_dummy("");
}
