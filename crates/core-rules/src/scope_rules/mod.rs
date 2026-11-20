//! The scope rules every language gets from a table: a language lists the rows of its
//! [`ScopeLang`] (which node kinds declare, refer to or assign names; `core_semantics::scopes`)
//! and the rules it wants, and the checks below read the model the binder builds, once per file.
//! A construct the table cannot decide turns a rule off for its scope, never guesses.
//!
//! concept: scope-rules
//! what: Unused, undefined, shadowed, redeclared, use-before-define and ambiguous-name checks from a language scope table.
//! rules: unused-variable, unused-parameter, unused-import, undefined-variable, shadowed-variable, redeclared-variable, use-before-define, ambiguous-name
//! reads: family:scope

mod cx;
mod expansions;
mod ordered;
mod ordering;
mod shadowed;
mod undefined;
mod unused;

use crate::checks::FileCache;
use crate::diagnostic::Category;
use crate::rule::{Level, Rule, RuleContext, RuleMeta};
use crate::tree::significant_range;
use crate::{FileContext, Params, Re};
pub use core_semantics::scopes::Ordered;
use core_semantics::scopes::{Access, Flags, Model, ScopeId, Spec, Table, bind};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};
use cx::Cx;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;

/// What a language says to the shared scope rules: the text of its `scope.rows`
/// (`core_semantics::scopes::Spec`), the global names it keeps in files (`globals/*.txt`) and,
/// for what is code, a hook.
pub struct ScopeLang {
    source: &'static str,
    skip_unused: Option<fn(&SyntaxNode, SyntaxKind, &str) -> bool>,
    globals: &'static str,
    conditional_globals: &'static [(&'static str, &'static str)],
    expansions: &'static [&'static str],
    arithmetic: &'static [&'static str],
    identifiers: bool,
    lowercase_undefined: bool,
    first_undefined: bool,
    spec: OnceLock<Spec>,
    sets: OnceLock<GlobalSets>,
    table: OnceLock<Table>,
}

impl ScopeLang {
    /// The language's rows and facts (`scope.rows`).
    pub const fn new(source: &'static str) -> ScopeLang {
        ScopeLang {
            source,
            skip_unused: None,
            globals: "",
            conditional_globals: &[],
            expansions: &[],
            arithmetic: &[],
            identifiers: false,
            lowercase_undefined: false,
            first_undefined: false,
            spec: OnceLock::new(),
            sets: OnceLock::new(),
            table: OnceLock::new(),
        }
    }

    /// Names defined everywhere, separated by white space (a `globals/*.txt` file).
    pub const fn globals(mut self, names: &'static str) -> Self {
        self.globals = names;
        self
    }

    /// Names defined in the files of a flavor only, from files (`commonjs`: `require module exports`).
    pub const fn globals_if(mut self, names: &'static [(&'static str, &'static str)]) -> Self {
        self.conditional_globals = names;
        self
    }

    /// Tokens of these kinds hold reads in their text: `$x`, `${x:-d}`, `${#x}`, `${!x}`
    /// (a shell word or heredoc body). A name guarded by a default (`${name:-x}`) only probes.
    pub const fn expansions(mut self, kinds: &'static [&'static str]) -> Self {
        self.expansions = kinds;
        self
    }

    /// Tokens of these kinds are arithmetic expressions whose bare identifiers are reads.
    pub const fn arithmetic(mut self, kinds: &'static [&'static str]) -> Self {
        self.arithmetic = kinds;
        self
    }

    /// Name tokens are words: only identifiers, with an optional `[subscript]`, are names.
    pub const fn identifier_names(mut self) -> Self {
        self.identifiers = true;
        self
    }

    /// Names with an upper case letter are the environment's: `undefined-variable` leaves them.
    pub const fn lowercase_undefined(mut self) -> Self {
        self.lowercase_undefined = true;
        self
    }

    /// `undefined-variable` reports a name at its first read only.
    pub const fn first_undefined(mut self) -> Self {
        self.first_undefined = true;
        self
    }

    /// `skip` answers for a method or block node (the kinds the `methods` and `blocks` facts
    /// name) whether its parameters are left alone (an empty body, a body that raises, a bare
    /// `super`) in `unused-method-argument` and `unused-block-argument`.
    pub const fn skip_unused(mut self, skip: fn(&SyntaxNode, SyntaxKind, &str) -> bool) -> Self {
        self.skip_unused = Some(skip);
        self
    }

    /// Has the language an `ordered` row? Read from the text: building a rule must not parse the table.
    fn is_ordered(&self) -> bool {
        Spec::is_ordered(self.source)
    }

    fn spec(&self) -> &Spec {
        self.spec
            .get_or_init(|| Spec::parse(self.source).unwrap_or_else(|e| panic!("scope table: {e}")))
    }

    fn table(&self, grammar: &Grammar) -> &Table {
        self.table.get_or_init(|| {
            let s = self.spec();
            Table::new(grammar, s.rows, s.hosts, s.name_tokens)
                .bare_names(s.bare)
                .dynamic_names(s.dynamic)
                .identifier_names(self.identifiers)
                .label_names(s.labels, grammar)
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
        (self.spec().flavors.iter())
            .any(|(n, suffixes)| *n == name && suffixes.iter().any(|s| matches(&path, s)))
    }

    /// What the name an import declaration binds stands for (`a` for `import a.b`).
    pub fn import_target(
        &self,
        g: &Grammar,
        root: &SyntaxNode,
        d: &core_semantics::scopes::Decl,
    ) -> Option<String> {
        ordered::import_target(g, root, self.spec().ordered.as_ref()?.pragma, d)
    }

    /// Is `name` one the language (or the file's flavor) declares everywhere?
    pub fn is_global(&self, name: &str, path: &Path) -> bool {
        let sets = self.sets.get_or_init(|| {
            let spec = self.spec();
            GlobalSets {
                base: self
                    .globals
                    .split_whitespace()
                    .chain(spec.globals.iter().copied())
                    .collect(),
                conditional: (self.conditional_globals.iter())
                    .map(|(f, names)| (*f, names.split_whitespace().collect()))
                    .chain(
                        (spec.conditional_globals.iter())
                            .map(|(f, names)| (*f, names.iter().copied().collect())),
                    )
                    .collect(),
            }
        });
        sets.base.contains(name)
            || sets
                .conditional
                .iter()
                .any(|(f, names)| names.contains(name) && self.flavor(f, path))
    }

    fn scope_name(&self, kind: &str, name: &str) -> bool {
        self.spec().scope_names.contains(&(kind, name))
    }

    fn checks_redeclared(&self, kind: &str) -> bool {
        self.spec().redeclared_in.contains(&kind)
    }

    fn skips_top(&self) -> bool {
        self.spec().top_unchecked
    }

    fn property_tokens(&self) -> impl Iterator<Item = &'static str> {
        self.spec().parameter_properties.iter().copied()
    }

    /// Is `name` the import that the markup elements of the tree use (the JSX pragma)?
    pub fn markup_import(&self, name: &str, root: &SyntaxNode, grammar: &Grammar) -> bool {
        !self.spec().markup.is_empty()
            && name == self.spec().markup
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
            if lang.spec().ordered.as_ref().is_some_and(|o| o.quoted) {
                ordered::quoted::add_reads(&mut model, file.root, file.grammar, lang);
            }
            if !lang.expansions.is_empty() {
                expansions::add_reads(&mut model, file, lang);
            }
            if !lang.spec().interpolated.is_empty() {
                interpolation_reads(&mut model, file, lang.spec().interpolated);
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
    // The reports of `unused-variable` for an annotation with no value and for a name of a tuple
    // target, of `undefined-variable` for a name a wildcard import may bring, and the names a
    // module exports that nothing binds: ordered languages only.
    UnusedAnnotation,
    UnusedUnpacked,
    StarUsage,
    UndefinedExport,
    /// A declaration of the given names (`names`: look-alike letters) of the given kind.
    AmbiguousName(Declared, &'static [&'static str]),
}

/// What an `ambiguous-name` check looks at: functions, classes, or the other declarations that
/// are neither imports nor type parameters.
#[derive(Clone, Copy)]
enum Declared {
    Variable,
    Class,
    Function,
}

impl Declared {
    fn has(self, flags: Flags) -> bool {
        match self {
            Declared::Class => flags.contains(Flags::CLASS),
            Declared::Function => flags.contains(Flags::FUNCTION),
            Declared::Variable => !flags
                .intersects(Flags::FUNCTION | Flags::CLASS | Flags::IMPORT | Flags::TYPE_PARAMETER),
        }
    }
}

impl Check {
    /// What of the findings of an ordered language the check reports.
    fn which(self) -> Option<ordered::Which> {
        use ordered::Which as W;
        Some(match self {
            Check::UnusedImport => W::UnusedImport,
            Check::UnusedVariable => W::UnusedVariable,
            Check::UndefinedVariable => W::Undefined,
            Check::UseBeforeDefine => W::UseBeforeDefine,
            Check::RedeclaredVariable => W::Redeclared,
            Check::ShadowedVariable => W::Shadowed,
            Check::UnusedAnnotation => W::UnusedAnnotation,
            Check::UnusedUnpacked => W::UnusedUnpacked,
            Check::StarUsage => W::StarUsage,
            Check::UndefinedExport => W::UndefinedExport,
            _ => return None,
        })
    }
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
    let name = p.text("check")?;
    let mode = p.optional("mode");
    let check = match (name.as_str(), mode.as_deref()) {
        ("ambiguous-name", None) => {
            let names = p.text("names")?;
            let names: Vec<&'static str> = names
                .split_whitespace()
                .map(|n| &*Box::leak(n.to_string().into_boxed_str()))
                .collect();
            let declared = p.choice(
                "of",
                true,
                &[
                    ("binding", Declared::Variable),
                    ("class", Declared::Class),
                    ("function", Declared::Function),
                ],
            )?;
            Check::AmbiguousName(declared, Box::leak(names.into_boxed_slice()))
        }
        ("undefined-variable", Some("star-usage")) => Check::StarUsage,
        ("unused-variable", Some("annotation")) => Check::UnusedAnnotation,
        ("unused-variable", Some("unpacked")) => Check::UnusedUnpacked,
        (_, Some(mode)) => return Err(format!("param `mode`: unknown mode `{mode}`")),
        (name, None) => match name {
            "unused-import" if lang.is_ordered() => Check::UnusedImport,
            "undefined-export" if lang.is_ordered() => Check::UndefinedExport,
            "undefined-variable" => Check::UndefinedVariable,
            "unused-variable" => Check::UnusedVariable,
            "unused-local-variable" => Check::UnusedLocal,
            "unused-parameter" => Check::UnusedParameter,
            "use-before-define" => Check::UseBeforeDefine,
            "redeclared-variable" => Check::RedeclaredVariable,
            "shadowed-variable" => Check::ShadowedVariable,
            other => return Err(format!("param `check`: unknown check `{other}`")),
        },
    };
    if !lang.is_ordered()
        && check.which().is_some_and(|_| {
            matches!(
                check,
                Check::UnusedAnnotation | Check::UnusedUnpacked | Check::StarUsage
            )
        })
    {
        return Err("param `mode`: the language is not ordered".into());
    }
    let message = p.text("message")?;
    p.done()?;
    if meta.default_level != Level::Off {
        // A rule that is on will run: its table is read with the rule, not in the first file.
        lang.spec();
    }
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
/// Does the path end with the flavor's suffix? One that does not start with a dot (`__init__.py`)
/// is a whole file name.
fn matches(path: &str, suffix: &str) -> bool {
    path.strip_suffix(suffix).is_some_and(|head| {
        suffix.starts_with('.') || head.is_empty() || head.ends_with(['/', '\\'])
    })
}

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
            | Check::ShadowedVariable
            | Check::UnusedAnnotation
            | Check::UnusedUnpacked
            | Check::StarUsage
            | Check::UndefinedExport
            | Check::AmbiguousName(..) => {
                unreachable!("a row of the rule files gives its metadata")
            }
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

    fn bodies(&self) -> bool {
        false
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let lang = self.lang;
        let m = model(lang, file);
        if lang.spec().ordered.is_some()
            && let Some(which) = self.check.which()
        {
            let cx = Cx::new(&m, file, lang);
            let template = self.template("'{name}' is not defined");
            for (range, name) in ordered::findings(&cx, file).get(which) {
                ctx.report(*range, template.replace("{name}", name));
            }
            return;
        }
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
                    if !s.flags.contains(Flags::IMPORT)
                        || lang.spec().conventions.contains(&&*s.name)
                    {
                        continue;
                    }
                    let used =
                        used.get_or_insert_with(|| m.refs.iter().map(|r| &*r.name).collect());
                    if used.contains(&*s.name) {
                        continue;
                    }
                    let docs = docs.get_or_insert_with(|| doc_text(file, lang.spec().docs));
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
                let template = self.template("the local variable `{name}` is never read");
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if !s.flags.contains(Flags::LET)
                        || s.flags.intersects(skip)
                        || &*s.name == "_"
                        || lang.spec().conventions.contains(&&*s.name)
                        || (!lang.spec().unused_prefix.is_empty()
                            && s.name.starts_with(lang.spec().unused_prefix))
                        || m.refs_of(id).any(|r| r.is_read())
                        || dynamic(s.scope)
                    {
                        continue;
                    }
                    for d in &s.decls {
                        ctx.report(d.name, template.replace("{name}", &s.name));
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
                    Check::UnusedMethodArgument => lang.spec().methods,
                    _ => lang.spec().blocks,
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
            Check::UnusedAnnotation
            | Check::UnusedUnpacked
            | Check::StarUsage
            | Check::UndefinedExport => {}
            Check::AmbiguousName(declared, names) => {
                let template = self.template("ambiguous name `{name}`");
                let mut hits = Vec::new();
                for id in m.symbol_ids() {
                    let s = m.symbol(id);
                    if names.contains(&&*s.name) {
                        hits.extend(
                            (s.decls.iter().filter(|d| declared.has(d.flags)))
                                .map(|d| (d.name, template.replace("{name}", &s.name))),
                        );
                    }
                }
                hits.sort_by_key(|(r, _)| r.start());
                hits.dedup();
                for (range, text) in hits {
                    ctx.report(range, text);
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
