//! The scope rules every language gets from a table: a language lists the rows of its
//! [`ScopeLang`] (which node kinds declare, refer to or assign names; `core_semantics::scopes`)
//! and the rules it wants, and the checks below read the model the binder builds, once per file.
//! A construct the table cannot decide turns a rule off for its scope, never guesses.

mod cx;
mod ordering;
mod shadowed;
mod undefined;
mod unused;

use crate::checks::FileCache;
use crate::diagnostic::Category;
use crate::rule::{Level, Rule, RuleContext, RuleMeta};
use crate::tree::significant_range;
use crate::{FileContext, Params, Re};
use core_semantics::scopes::{Access, Flags, Model, Row, ScopeId, Table, bind};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};
use cx::Cx;
use std::collections::HashSet;
use std::path::Path;
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
    globals: &'static str,
    conditional_globals: &'static [(&'static str, &'static str)],
    flavors: &'static [(&'static str, &'static [&'static str])],
    scope_names: &'static [(&'static str, &'static str)],
    redeclared_in: &'static [&'static str],
    parameter_properties: &'static str,
    markup: &'static str,
    top_unchecked: bool,
    sets: OnceLock<GlobalSets>,
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
            globals: "",
            conditional_globals: &[],
            flavors: &[],
            scope_names: &[],
            redeclared_in: &[],
            parameter_properties: "",
            markup: "",
            top_unchecked: false,
            sets: OnceLock::new(),
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

    /// Names defined everywhere, separated by spaces (`$this`, superglobals, `Array`).
    pub const fn globals(mut self, names: &'static str) -> Self {
        self.globals = names;
        self
    }

    /// Names defined in the files of a flavor only (`commonjs`: `require module exports`).
    pub const fn globals_if(mut self, names: &'static [(&'static str, &'static str)]) -> Self {
        self.conditional_globals = names;
        self
    }

    /// Flavors of files by name suffix. `typed` files declare types (declaration merging,
    /// ambient names, type-only uses), `definition` files are ambient; any other flavor only
    /// selects `globals_if`.
    pub const fn flavors(
        mut self,
        flavors: &'static [(&'static str, &'static [&'static str])],
    ) -> Self {
        self.flavors = flavors;
        self
    }

    /// Names that the scopes of these node kinds bind (`arguments` in a function).
    pub const fn scope_names(mut self, names: &'static [(&'static str, &'static str)]) -> Self {
        self.scope_names = names;
        self
    }

    /// The node kinds of the scopes whose names `redeclared-variable` checks.
    pub const fn redeclared_in(mut self, kinds: &'static [&'static str]) -> Self {
        self.redeclared_in = kinds;
        self
    }

    /// Own tokens (separated by spaces) that make a parameter a property, which is used by being
    /// declared (`public`, `readonly`).
    pub const fn parameter_properties(mut self, tokens: &'static str) -> Self {
        self.parameter_properties = tokens;
        self
    }

    /// The import that the nodes tagged `#markup` use (the JSX pragma `React`), in typed files.
    pub const fn markup(mut self, import: &'static str) -> Self {
        self.markup = import;
        self
    }

    /// `undefined-variable` leaves the top scope alone: another file can define its names.
    pub const fn top_unchecked(mut self) -> Self {
        self.top_unchecked = true;
        self
    }

    /// The node kinds besides blocks whose child declarations are statements.
    pub const fn hosts(mut self, hosts: &'static [&'static str]) -> Self {
        self.hosts = hosts;
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

struct GlobalSets {
    base: HashSet<&'static str>,
    conditional: Vec<(&'static str, HashSet<&'static str>)>,
}

impl ScopeLang {
    /// The model of the tree under `root` (an embedded script, for its host's rules).
    pub fn bind(&self, root: &SyntaxNode, grammar: &Grammar) -> Model {
        bind(root, grammar, self.table(grammar))
    }

    fn flavor(&self, name: &str, path: &Path) -> bool {
        let path = path.to_string_lossy();
        self.flavors
            .iter()
            .any(|(n, suffixes)| *n == name && suffixes.iter().any(|s| path.ends_with(s)))
    }

    fn is_global(&self, name: &str, path: &Path) -> bool {
        let sets = self.sets.get_or_init(|| GlobalSets {
            base: self.globals.split_whitespace().collect(),
            conditional: self
                .conditional_globals
                .iter()
                .map(|(f, names)| (*f, names.split_whitespace().collect()))
                .collect(),
        });
        sets.base.contains(name)
            || sets
                .conditional
                .iter()
                .any(|(f, names)| names.contains(name) && self.flavor(f, path))
    }

    fn scope_name(&self, kind: &str, name: &str) -> bool {
        self.scope_names.contains(&(kind, name))
    }

    fn checks_redeclared(&self, kind: &str) -> bool {
        self.redeclared_in.contains(&kind)
    }

    fn skips_top(&self) -> bool {
        self.top_unchecked
    }

    fn property_tokens(&self) -> impl Iterator<Item = &'static str> {
        self.parameter_properties.split_whitespace()
    }

    /// Is `name` the import that the markup elements of the tree use (the JSX pragma)?
    pub fn markup_import(&self, name: &str, root: &SyntaxNode, grammar: &Grammar) -> bool {
        !self.markup.is_empty()
            && name == self.markup
            && root
                .descendants()
                .any(|n| grammar.has_tag(n.kind(), "markup"))
    }
}

thread_local! {
    static CACHE: FileCache<Model> = const { FileCache::new() };
}

/// The model of the file, built by the first scope rule that runs on it (and shared with the
/// rules of the language that read it).
pub fn model(lang: &ScopeLang, file: &FileContext<'_>) -> Rc<Model> {
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
    UnusedVariable,
    UnusedParameter,
    UseBeforeDefine,
    RedeclaredVariable,
    ShadowedVariable,
}

/// One scope rule of a language.
pub struct ScopeRule {
    check: Check,
    lang: &'static ScopeLang,
    /// The rule's metadata and message when it is a row of the language's rule files.
    row: Option<(&'static RuleMeta, String)>,
}

/// The `build` of a `scope` family over `lang`: the rule's params are `check` (the rule's id) and
/// `message`, which may use `{name}`, `{line}` (`shadowed-variable`) and `{action}` (`unused-*`).
pub fn build(
    lang: &'static ScopeLang,
    meta: &'static RuleMeta,
    mut p: Params,
) -> Result<Box<dyn Rule>, String> {
    let check = match p.text("check")?.as_str() {
        "undefined-variable" => Check::UndefinedVariable,
        "unused-variable" => Check::UnusedVariable,
        "unused-parameter" => Check::UnusedParameter,
        "use-before-define" => Check::UseBeforeDefine,
        "redeclared-variable" => Check::RedeclaredVariable,
        "shadowed-variable" => Check::ShadowedVariable,
        other => return Err(format!("param `check`: unknown check `{other}`")),
    };
    let message = p.text("message")?;
    p.done()?;
    Ok(Box::new(ScopeRule {
        check,
        lang,
        row: Some((meta, message)),
    }))
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
            row: None,
        }
    }

    /// A local variable that is never read.
    pub const fn unused_local(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnusedLocal,
            lang,
            row: None,
        }
    }

    /// An assignment to a parameter.
    pub const fn parameter_assignment(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::ParameterAssignment,
            lang,
            row: None,
        }
    }

    /// A method parameter that is never read.
    pub const fn unused_method_argument(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnusedMethodArgument,
            lang,
            row: None,
        }
    }

    /// A block parameter that is never read.
    pub const fn unused_block_argument(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnusedBlockArgument,
            lang,
            row: None,
        }
    }

    /// A variable named with a leading `_` (meant to be unused) that is read.
    pub const fn underscore_prefixed(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UnderscorePrefixed,
            lang,
            row: None,
        }
    }

    /// A variable read in a function that nothing in it ever assigns.
    pub const fn undefined_variable(lang: &'static ScopeLang) -> ScopeRule {
        ScopeRule {
            check: Check::UndefinedVariable,
            lang,
            row: None,
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

impl ScopeRule {
    /// The rule's message: the row's, or the `default` of a rule a language lists in Rust.
    fn template(&self, default: &str) -> String {
        self.row
            .as_ref()
            .map_or_else(|| default.to_string(), |(_, m)| m.clone())
    }
}

impl Rule for ScopeRule {
    fn meta(&self) -> &'static RuleMeta {
        if let Some((meta, _)) = &self.row {
            return meta;
        }
        match self.check {
            Check::UnusedVariable
            | Check::UnusedParameter
            | Check::UseBeforeDefine
            | Check::RedeclaredVariable
            | Check::ShadowedVariable => unreachable!("a row of the rule files gives its metadata"),
            Check::UnusedImport => meta!(
                "unused-import",
                Cleanup,
                Off,
                "An import whose name the file does not use (Checkstyle UnusedImports, ktlint \
                 no-unused-imports). Imports with `*` are not checked, and a name in a doc \
                 comment counts as a use where the language has doc comments. No fix: removing \
                 an import can change which declaration a name means.",
                "import java.util.List;   // List is not used"
            ),
            Check::UnusedLocal => meta!(
                "unused-local-variable",
                Cleanup,
                Off,
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
                Off,
                "A method parameter that the method never reads (RuboCop Lint/UnusedMethodArgument). \
                 Names that start with `_`, empty methods and methods that only raise \
                 NotImplementedError are left alone. No fix.",
                "def f(a, b)\n  a\nend   # b is not used"
            ),
            Check::UnusedBlockArgument => meta!(
                "unused-block-argument",
                Cleanup,
                Off,
                "A block parameter that the block never reads (RuboCop Lint/UnusedBlockArgument). \
                 Names that start with `_` and empty blocks are left alone. No fix.",
                "items.each { |a, b| puts a }   # b is not used"
            ),
            Check::UnderscorePrefixed => meta!(
                "underscore-prefixed-variable-used",
                Style,
                Off,
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
                let extra = ctx.options.list("globals");
                let template = self.template("the variable `{name}` is not defined");
                for (range, name) in undefined::run(&Cx::new(&m, file, lang), &extra) {
                    ctx.report(range, template.replace("{name}", &name));
                }
            }
            Check::UnusedVariable | Check::UnusedParameter => {
                let o = &ctx.options;
                let ignore = o.string("ignore_pattern");
                let parameters = matches!(self.check, Check::UnusedParameter);
                let opts = unused::Opts {
                    parameters,
                    args: match o.string("args") {
                        "all" => unused::Args::All,
                        "none" => unused::Args::None,
                        _ => unused::Args::AfterUsed,
                    },
                    caught_errors: o.string("caught_errors") != "none",
                    ignore_rest_siblings: o.bool("ignore_rest_siblings"),
                    ignore: (!ignore.is_empty())
                        .then(|| Re::lazy(ignore).ok())
                        .flatten(),
                };
                let template = self.template("'{name}' is {action}.");
                for h in unused::run(&Cx::new(&m, file, lang), &opts) {
                    let text = template
                        .replace("{name}", &h.name)
                        .replace("{action}", h.action);
                    ctx.report(h.range, text);
                }
            }
            Check::UseBeforeDefine => {
                let o = &ctx.options;
                let on = ordering::Kinds {
                    functions: o.bool("functions"),
                    classes: o.bool("classes"),
                    variables: o.bool("variables"),
                };
                let template = self.template("'{name}' was used before it was defined.");
                for (range, name) in ordering::use_before_define(&Cx::new(&m, file, lang), &on) {
                    ctx.report(range, template.replace("{name}", &name));
                }
            }
            Check::RedeclaredVariable => {
                let template = self.template("'{name}' is already defined.");
                for (range, name) in ordering::redeclared(&Cx::new(&m, file, lang)) {
                    ctx.report(range, template.replace("{name}", &name));
                }
            }
            Check::ShadowedVariable => {
                let o = &ctx.options;
                let opts = shadowed::Opts {
                    allow: o.list("allow"),
                    kinds: o.list("kinds"),
                };
                let template = self
                    .template("'{name}' is already declared in the upper scope on line {line}.");
                for h in shadowed::run(&Cx::new(&m, file, lang), &opts) {
                    let text = template
                        .replace("{name}", &h.name)
                        .replace("{line}", &h.line.to_string());
                    ctx.report(h.range, text);
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
