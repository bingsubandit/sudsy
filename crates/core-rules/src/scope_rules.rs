//! The scope rules every language gets from a table: a language lists the rows of its
//! [`ScopeLang`] (which node kinds declare, refer to or assign names; `core_semantics::scopes`)
//! and the rules it wants, and the checks below read the model the binder builds, once per file.
//! A construct the table cannot decide turns a rule off for its scope, never guesses.

use crate::checks::FileCache;
use crate::diagnostic::Category;
use crate::rule::{Level, Rule, RuleContext, RuleMeta};
use crate::FileContext;
use core_semantics::scopes::{Flags, Model, Row, Table, bind};
use core_syntax::Grammar;
use std::rc::Rc;
use std::sync::OnceLock;

/// What a language says to the shared scope rules.
pub struct ScopeLang {
    rows: &'static [Row],
    name_tokens: &'static [&'static str],
    hosts: &'static [&'static str],
    bare: bool,
    implicit: Option<Flags>,
    dynamic: &'static [&'static str],
    docs: &'static str,
    conventions: &'static [&'static str],
    globals: &'static [&'static str],
    table: OnceLock<Table>,
}

impl ScopeLang {
    /// Rows by node kind, and the token kinds a name is written with.
    pub const fn new(rows: &'static [Row], name_tokens: &'static [&'static str]) -> ScopeLang {
        ScopeLang {
            rows,
            name_tokens,
            hosts: &[],
            bare: false,
            implicit: None,
            dynamic: &[],
            docs: "",
            conventions: &[],
            globals: &[],
            table: OnceLock::new(),
        }
    }

    /// Names are bare tokens: every one that is not a declaration is a use (Java).
    pub const fn bare(mut self) -> Self {
        self.bare = true;
        self
    }

    /// A write to an undeclared name declares it with these flags (PHP).
    pub const fn implicit(mut self, flags: Flags) -> Self {
        self.implicit = Some(flags);
        self
    }

    /// Calling one of these makes the names of the scope unknowable (`extract`).
    pub const fn dynamic(mut self, names: &'static [&'static str]) -> Self {
        self.dynamic = names;
        self
    }

    /// Comments that start with this text (`/**`) use the imports they mention.
    pub const fn docs(mut self, prefix: &'static str) -> Self {
        self.docs = prefix;
        self
    }

    /// Imports of these names are used by convention, not by name (Kotlin `getValue`).
    pub const fn conventions(mut self, names: &'static [&'static str]) -> Self {
        self.conventions = names;
        self
    }

    /// Names defined everywhere (`$this`, superglobals).
    pub const fn globals(mut self, names: &'static [&'static str]) -> Self {
        self.globals = names;
        self
    }

    fn table(&self, grammar: &Grammar) -> &Table {
        self.table.get_or_init(|| {
            Table::new(grammar, self.rows, self.hosts, self.name_tokens)
                .bare_names(self.bare)
                .implicit(self.implicit)
                .dynamic_names(self.dynamic)
        })
    }
}

thread_local! {
    static CACHE: FileCache<Model> = const { FileCache::new() };
}

/// The model of the file, built by the first scope rule that runs on it.
fn model(lang: &ScopeLang, file: &FileContext<'_>) -> Rc<Model> {
    CACHE.with(|c| c.get(file, || bind(file.root, file.grammar, lang.table(file.grammar))))
}

#[derive(Clone, Copy)]
enum Check {
    UnusedImport,
    UnusedLocal,
    ParameterAssignment,
    UndefinedVariable,
}

/// One scope rule of a language.
pub struct ScopeRule {
    check: Check,
    lang: &'static ScopeLang,
}

macro_rules! meta {
    ($id:expr, $category:ident, $level:ident, $description:expr, $example:expr) => {{
        static META: RuleMeta = RuleMeta {
            id: $id,
            category: Category::$category,
            description: $description,
            example: $example,
            default_level: Level::$level,
            fix: None,
            requires: &[],
            uses: &[],
            options: &[],
        };
        &META
    }};
}

impl ScopeRule {
    /// An import none of the file's names uses (and no doc comment mentions).
    pub const fn unused_import(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule { check: Check::UnusedImport, lang }
    }

    /// A local variable that is never read.
    pub const fn unused_local(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule { check: Check::UnusedLocal, lang }
    }

    /// An assignment to a parameter.
    pub const fn parameter_assignment(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule { check: Check::ParameterAssignment, lang }
    }

    /// A variable read in a function that nothing in it ever assigns.
    pub const fn undefined_variable(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule { check: Check::UndefinedVariable, lang }
    }
}

/// Whether `text` contains `name` as a whole word.
fn mentions(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(i, _)| {
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        !word(text[..i].chars().next_back()) && !word(text[i + name.len()..].chars().next())
    })
}

impl Rule for ScopeRule {
    fn meta(&self) -> &'static RuleMeta {
        match self.check {
            Check::UnusedImport => meta!(
                "unused-import",
                Cleanup,
                Warn,
                "An import whose name the file does not use (Checkstyle UnusedImports, ktlint \
                 no-unused-imports). Imports with `*` are not checked, and a name in a doc \
                 comment counts as a use where the language has doc comments. No fix: removing \
                 an import can change which declaration a name means.",
                "import java.util.List;   // List is not used"
            ),
            Check::UnusedLocal => meta!(
                "unused-local-variable",
                Cleanup,
                Warn,
                "A local variable that is never read: it is declared, and perhaps assigned, and \
                 that is all (Checkstyle UnusedLocalVariable). No fix.",
                "int count = 0;   // never read"
            ),
            Check::ParameterAssignment => meta!(
                "parameter-assignment",
                Style,
                Off,
                "A parameter is assigned a new value (Checkstyle ParameterAssignment). No fix.",
                "void f(int n) { n = 1; }   // use another variable"
            ),
            Check::UndefinedVariable => meta!(
                "undefined-variable",
                Correctness,
                Warn,
                "A variable is read in a function that never assigns it, takes it as a parameter, \
                 imports it with `use` or declares it with `global` or `static` (PHPStan level 0, \
                 always undefined variables). A variable passed to a call counts as assigned, \
                 since the call can take it by reference; a function that uses `extract`, \
                 `compact`, `include` or `$$name` is not checked.",
                "function f() { return $total; }   // $total is not defined"
            ),
        }
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let lang = self.lang;
        let m = model(lang, file);
        let dynamic = |scope| {
            let mut open = true;
            m.ancestors(scope)
                .take_while(|s| std::mem::replace(&mut open, !m.scope(*s).closed))
                .any(|s| m.scope(s).dynamic)
        };
        match self.check {
            Check::UnusedImport => {
                let mut docs: Option<String> = None;
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if !s.flags.contains(Flags::IMPORT)
                        || !s.refs.is_empty()
                        || lang.conventions.contains(&&*s.name)
                    {
                        continue;
                    }
                    let docs = docs.get_or_insert_with(|| doc_text(file, lang.docs));
                    if mentions(docs, &s.name) {
                        continue;
                    }
                    for d in &s.decls {
                        ctx.report(d.node_range, format!("the import of `{}` is not used", s.name));
                    }
                }
            }
            Check::UnusedLocal => {
                let skip = Flags::PARAMETER | Flags::IMPORT | Flags::FIELD;
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if !s.flags.contains(Flags::LET)
                        || s.flags.intersects(skip)
                        || &*s.name == "_"
                        || m.refs_of(id).any(|r| r.is_read())
                        || dynamic(s.scope)
                    {
                        continue;
                    }
                    for d in &s.decls {
                        ctx.report(d.name, format!("the local variable `{}` is never read", s.name));
                    }
                }
            }
            Check::ParameterAssignment => {
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if !s.flags.contains(Flags::PARAMETER) {
                        continue;
                    }
                    for r in m.refs_of(id).filter(|r| r.is_write()) {
                        ctx.report(r.range, format!("the parameter `{}` is assigned", s.name));
                    }
                }
            }
            Check::UndefinedVariable => {
                for r in m.unresolved() {
                    let top = m.scope(m.var_scope(r.scope)).parent.is_none();
                    if r.is_write()
                        || !r.is_read()
                        || top
                        || lang.globals.contains(&&*r.name)
                        || dynamic(r.scope)
                    {
                        continue;
                    }
                    ctx.report(r.range, format!("the variable `{}` is not defined", r.name));
                }
            }
        }
    }
}

/// The text of the doc comments of the file (comments starting with `prefix`); empty when the
/// language has none.
fn doc_text(file: &FileContext<'_>, prefix: &str) -> String {
    let mut out = String::new();
    if prefix.is_empty() {
        return out;
    }
    for t in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
        if t.text().starts_with(prefix) && file.grammar.is_trivia(t.kind()) {
            out.push_str(t.text());
            out.push('\n');
        }
    }
    out
}
