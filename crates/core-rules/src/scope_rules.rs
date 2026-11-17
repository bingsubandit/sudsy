//! The scope rules every language gets from a table: a language lists the rows of its
//! [`ScopeLang`] (which node kinds declare, refer to or assign names; `core_semantics::scopes`)
//! and the rules it wants, and the checks below read the model the binder builds, once per file.
//! A construct the table cannot decide turns a rule off for its scope, never guesses.

use crate::FileContext;
use crate::checks::FileCache;
use crate::diagnostic::Category;
use crate::rule::{Level, Rule, RuleContext, RuleMeta};
use crate::tree::significant_range;
use core_semantics::scopes::{Access, Flags, Model, Row, ScopeId, Table, bind};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;

/// What a language says to the shared scope rules.
pub struct ScopeLang {
    rows: &'static [Row],
    name_tokens: &'static [&'static str],
    hosts: &'static [&'static str],
    bare: bool,
    dynamic: &'static [&'static str],
    docs: &'static str,
    conventions: &'static [&'static str],
    globals: &'static [&'static str],
    labels: &'static [&'static str],
    methods: &'static [&'static str],
    blocks: &'static [&'static str],
    skip_unused: Option<fn(&SyntaxNode, SyntaxKind, &str) -> bool>,
    interpolated: &'static str,
    unused_prefix: &'static str,
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
            dynamic: &[],
            docs: "",
            conventions: &[],
            globals: &[],
            labels: &[],
            methods: &[],
            blocks: &[],
            skip_unused: None,
            interpolated: "",
            unused_prefix: "",
            table: OnceLock::new(),
        }
    }

    /// Names are bare tokens: every one that is not a declaration is a use (Java).
    pub const fn bare(mut self) -> Self {
        self.bare = true;
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

    /// Name tokens of these kinds are written with a trailing `:` (`key:`).
    pub const fn labels(mut self, kinds: &'static [&'static str]) -> Self {
        self.labels = kinds;
        self
    }

    /// The node kinds of methods and of blocks, whose parameters `unused-method-argument` and
    /// `unused-block-argument` check; `skip` answers for a method or block node whether its
    /// parameters are left alone (an empty body, a body that raises, a bare `super`).
    pub const fn arguments(
        mut self,
        methods: &'static [&'static str],
        blocks: &'static [&'static str],
        skip: fn(&SyntaxNode, SyntaxKind, &str) -> bool,
    ) -> Self {
        self.methods = methods;
        self.blocks = blocks;
        self.skip_unused = Some(skip);
        self
    }

    /// Locals whose name starts with this are meant to be unused: `unused-local-variable` leaves them.
    pub const fn unused_prefix(mut self, prefix: &'static str) -> Self {
        self.unused_prefix = prefix;
        self
    }

    /// Tokens of this kind hold code the tree has no nodes for (a heredoc body with `#{x}`):
    /// the names in their `#{...}` are reads.
    pub const fn interpolated(mut self, token: &'static str) -> Self {
        self.interpolated = token;
        self
    }

    fn table(&self, grammar: &Grammar) -> &Table {
        self.table.get_or_init(|| {
            Table::new(grammar, self.rows, self.hosts, self.name_tokens)
                .bare_names(self.bare)
                .dynamic_names(self.dynamic)
                .label_names(self.labels, grammar)
        })
    }
}

thread_local! {
    static CACHE: FileCache<Model> = const { FileCache::new() };
}

/// The model of the file, built by the first scope rule that runs on it.
fn model(lang: &ScopeLang, file: &FileContext<'_>) -> Rc<Model> {
    CACHE.with(|c| {
        c.get(file, || {
            let mut model = bind(file.root, file.grammar, lang.table(file.grammar));
            if !lang.interpolated.is_empty() {
                interpolation_reads(&mut model, file, lang.interpolated);
            }
            model
        })
    })
}

#[derive(Clone, Copy)]
enum Check {
    UnusedImport,
    UnusedLocal,
    ParameterAssignment,
    UndefinedVariable,
    UnusedMethodArgument,
    UnusedBlockArgument,
    UnderscorePrefixed,
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
        ScopeRule {
            check: Check::UnusedImport,
            lang,
        }
    }

    /// A local variable that is never read.
    pub const fn unused_local(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnusedLocal,
            lang,
        }
    }

    /// An assignment to a parameter.
    pub const fn parameter_assignment(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::ParameterAssignment,
            lang,
        }
    }

    /// A method parameter that is never read.
    pub const fn unused_method_argument(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnusedMethodArgument,
            lang,
        }
    }

    /// A block parameter that is never read.
    pub const fn unused_block_argument(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnusedBlockArgument,
            lang,
        }
    }

    /// A variable named with a leading `_` (meant to be unused) that is read.
    pub const fn underscore_prefixed(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnderscorePrefixed,
            lang,
        }
    }

    /// A variable read in a function that nothing in it ever assigns.
    pub const fn undefined_variable(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UndefinedVariable,
            lang,
        }
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
                 that is all (Checkstyle UnusedLocalVariable, RuboCop Lint/UselessAssignment). No fix.",
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
                 `parse_str`, `include` or `$$name` is not checked.",
                "function f() { return $total; }   // $total is not defined"
            ),
            Check::UnusedMethodArgument => meta!(
                "unused-method-argument",
                Cleanup,
                Warn,
                "A method parameter that the method never reads (RuboCop Lint/UnusedMethodArgument). \
                 Names that start with `_`, empty methods and methods that only raise \
                 NotImplementedError are left alone. No fix.",
                "def f(a, b)\n  a\nend   # b is not used"
            ),
            Check::UnusedBlockArgument => meta!(
                "unused-block-argument",
                Cleanup,
                Warn,
                "A block parameter that the block never reads (RuboCop Lint/UnusedBlockArgument). \
                 Names that start with `_` and empty blocks are left alone. No fix.",
                "items.each { |a, b| puts a }   # b is not used"
            ),
            Check::UnderscorePrefixed => meta!(
                "underscore-prefixed-variable-used",
                Style,
                Warn,
                "A variable whose name starts with `_`, which says it is unused, is read \
                 (RuboCop Lint/UnderscorePrefixedVariableName). No fix.",
                "def f(_a)\n  _a\nend"
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
                // Used by name, whatever the name resolves to: a local `mock` does not hide the
                // static import of the method `mock`.
                let mut used: Option<HashSet<&str>> = None;
                let mut docs: Option<String> = None;
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if !s.flags.contains(Flags::IMPORT) || lang.conventions.contains(&&*s.name) {
                        continue;
                    }
                    let used =
                        used.get_or_insert_with(|| m.refs.iter().map(|r| &*r.name).collect());
                    if used.contains(&*s.name) {
                        continue;
                    }
                    let docs = docs.get_or_insert_with(|| doc_text(file, lang.docs));
                    if mentions(docs, &s.name) {
                        continue;
                    }
                    for d in &s.decls {
                        let range = significant_range(&d.node(file.root), file.grammar);
                        ctx.report(range, format!("the import of `{}` is not used", s.name));
                    }
                }
            }
            Check::UnusedLocal => {
                let skip = Flags::PARAMETER | Flags::IMPORT;
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if !s.flags.contains(Flags::LET)
                        || s.flags.intersects(skip)
                        || &*s.name == "_"
                        || (!lang.unused_prefix.is_empty()
                            && s.name.starts_with(lang.unused_prefix))
                        || m.refs_of(id).any(|r| r.is_read())
                        || dynamic(s.scope)
                    {
                        continue;
                    }
                    for d in &s.decls {
                        ctx.report(
                            d.name,
                            format!("the local variable `{}` is never read", s.name),
                        );
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
            Check::UnusedMethodArgument | Check::UnusedBlockArgument => {
                let kinds = match self.check {
                    Check::UnusedMethodArgument => lang.methods,
                    _ => lang.blocks,
                };
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if !s.flags.contains(Flags::PARAMETER)
                        || s.name.starts_with('_')
                        || m.refs_of(id).any(|r| r.is_read())
                        || dynamic(s.scope)
                    {
                        continue;
                    }
                    let scope = &m.scope(s.scope).node;
                    if !kinds.contains(&file.grammar.name(scope.kind())) {
                        continue;
                    }
                    for d in &s.decls {
                        if lang
                            .skip_unused
                            .is_some_and(|skip| skip(scope, d.node_kind, &s.name))
                        {
                            continue;
                        }
                        ctx.report(d.name, format!("the parameter `{}` is never read", s.name));
                    }
                }
            }
            Check::UnderscorePrefixed => {
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if s.name.starts_with('_')
                        && s.flags.contains(Flags::VALUE)
                        && m.refs_of(id).any(|r| r.is_read())
                        && let Some(d) = s.decls.first()
                    {
                        ctx.report(d.name, format!("`{}` is read but named as unused", s.name));
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

/// The names written in the `#{...}` of the tokens of kind `token` are reads in the scope the
/// token is in.
fn interpolation_reads(model: &mut Model, file: &FileContext<'_>, token: &str) {
    let Some(kind) = file.grammar.kind_by_name(token) else {
        return;
    };
    let mut last = None;
    for t in file.text.match_indices("#{").filter_map(|(at, _)| {
        file.root
            .token_at_offset(core_syntax::TextSize::new(at as u32))
            .right_biased()
            .filter(|t| t.kind() == kind)
    }) {
        let at = t.text_range().start();
        if last.replace(at) == Some(at) {
            continue;
        }
        let Some(parent) = t.parent() else { continue };
        // The body comes after the line of its opener, which is where the names are in scope:
        // the scopes that touch that line see the reads.
        let end = usize::from(at).saturating_sub(1);
        let line = file.text[..end].rfind('\n').map_or(0, |i| i + 1);
        let scopes: Vec<ScopeId> = (0..model.scopes.len())
            .filter(|i| {
                let r = model.scopes[*i].node.text_range();
                usize::from(r.start()) < end && usize::from(r.end()) > line
            })
            .map(|i| ScopeId(i as u32))
            .collect();
        let text = t.text();
        let mut depth = 0usize;
        let mut word = None::<usize>;
        let mut words = Vec::new();
        for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
            if depth == 0 {
                if text[i..].starts_with("#{") {
                    depth = 1;
                }
                continue;
            }
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            let part = c.is_alphanumeric() || c == '_';
            match (word, part) {
                (None, true) => word = Some(i),
                (Some(start), false) => {
                    let before = &text[..start];
                    let member = before.ends_with('.') && !before.ends_with("..");
                    if !member && !before.ends_with([':', '@', '$', '&']) {
                        words.push((start, i));
                    }
                    word = None;
                }
                _ => {}
            }
        }
        for (start, end) in words {
            let name = &text[start..end];
            if name.starts_with(|c: char| c.is_lowercase() || c == '_') {
                let range = core_syntax::TextRange::at(
                    at + core_syntax::TextSize::new(start as u32),
                    core_syntax::TextSize::new((end - start) as u32),
                );
                for scope in &scopes {
                    model.add_reference(
                        name,
                        range,
                        (parent.text_range(), parent.kind()),
                        Access::READ,
                        *scope,
                    );
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
    for t in file
        .root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if t.text().starts_with(prefix) && file.grammar.is_trivia(t.kind()) {
            out.push_str(t.text());
            out.push('\n');
        }
    }
    out
}
