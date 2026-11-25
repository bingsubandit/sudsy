//! A language's symbol data as text (`symbols.rows` next to its `lang.yaml`): fact lines (`key
//! words...`) and one row per node kind (`Kind role key=Kind,Kind word=text`). `//` starts a
//! comment. Kind names are checked against the grammar when the [`Table`] is built; an unknown
//! kind, role or key, or a key a role does not take, is an error that names the line.
//!
//! concept: symbol-tables
//! what: Which node kinds declare types, members, functions, constants and imports, and a language's name rules, as rows resolved against the grammar.
//! tools: none (language facts as rows)
//! reads: file:symbols.rows
//!
//! Facts: `separator X`, `absolute X` (what a name starts with to be absolute), `relative W` (a
//! first segment that stands for the current namespace), `lexical` (a name is looked up in every
//! enclosing scope, not in the namespace only), `fold KIND...` and `fallback KIND...` (name kinds:
//! `type function constant member`), `constructor NAME`, `sigil CHARS`, `dynamic-members NAME...`,
//! `declaring-calls NAME...`, `promotion WORD...` (modifiers that make a constructor parameter a
//! property), `use-kinds WORD=KIND...`, `packages SYSTEM MANIFEST` (the package system that finds the files of names or imports: `composer`, `import-paths`; the manifest names the file in the project's root folder).
//!
//! Roles (keys they take): `namespace` (`name`, flag `file`: the whole file), `type KIND`
//! (`name`, `modifiers`, word `name-after=function`: the name follows that keyword), `heritage` (`names`, word `relation=extends|implements`), `function`
//! and `method` (`name`, `params`, `modifiers`, `refs`), `param` (`name`, `optional`, `variadic`,
//! `modifiers`), `property`, `const` and `case` (`item`, `name`, `modifiers`, `refs`), `global-const` (`item`, `name`: a
//! constant of the namespace), `mixin`
//! (`names`, `adapt`), `use` (an import statement), `use-group` (`names`: the prefix of a group),
//! `use-kind` (a node whose own keyword says which names it imports), `use-item` (`names`,
//! `alias`), `file-import` (`names`, `modifiers`), `reference` (a node whose text is a name the file writes), `transparent` (its children count as its
//! parent's), `ignore` (declares nothing; its `refs` still count as names the file writes). A `refs` entry is a kind (the own children of that kind) or `label:Kind` (the grammar's slot `label`, when it holds a `Kind`).

use super::TypeKind;
use core_syntax::{Grammar, SyntaxKind};
use std::sync::OnceLock;

/// The namespaces a name lives in: its kind picks the case rule and the fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    Type,
    Function,
    Constant,
    Member,
}

impl NameKind {
    fn of(word: &str) -> Option<NameKind> {
        Some(match word {
            "type" => NameKind::Type,
            "function" => NameKind::Function,
            "constant" => NameKind::Constant,
            "member" => NameKind::Member,
            _ => return None,
        })
    }

    pub(crate) const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// What a language says in its fact lines.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub separator: String,
    pub absolute: String,
    pub relative: String,
    pub lexical: bool,
    pub(crate) fold: u8,
    pub(crate) fallback: u8,
    pub constructor: String,
    /// Characters a variable's name starts with that are no part of the property's name.
    pub sigil: String,
    pub dynamic_members: Vec<String>,
    pub declaring_calls: Vec<String>,
    pub promotion: Vec<String>,
    /// The package system of the language (see `core_packages::open`); empty for none.
    pub packages: String,
    /// The file that marks the root folder of a project of that package system.
    pub manifest: String,
    pub(crate) use_kinds: Vec<(String, NameKind)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Namespace,
    Type,
    Heritage,
    Function,
    Method,
    Param,
    Property,
    Const,
    GlobalConst,
    Case,
    Mixin,
    Use,
    UseGroup,
    UseKind,
    UseItem,
    FileImport,
    Reference,
    Transparent,
    Ignore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Name,
    Item,
    Params,
    Optional,
    Variadic,
    Modifiers,
    Names,
    Adapt,
    Refs,
    Alias,
}

const KEYS: [(&str, Key); 10] = [
    ("name", Key::Name),
    ("item", Key::Item),
    ("params", Key::Params),
    ("optional", Key::Optional),
    ("variadic", Key::Variadic),
    ("modifiers", Key::Modifiers),
    ("names", Key::Names),
    ("adapt", Key::Adapt),
    ("refs", Key::Refs),
    ("alias", Key::Alias),
];

/// One row: what a node kind is and the kinds it keeps its parts in.
#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub role: Role,
    pub type_kind: Option<TypeKind>,
    pub file: bool,
    /// The name is the first name token after the own token with this text (`function`).
    pub after: String,
    pub relation: Option<super::Relation>,
    /// `refs` entries written `label:Kind`: the child of the grammar's slot `label`, when it is
    /// a token or node of `Kind`.
    slots: Vec<(String, SyntaxKind)>,
    kinds: [Vec<SyntaxKind>; KEYS.len()],
}

impl Row {
    pub fn kinds(&self, key: Key) -> &[SyntaxKind] {
        &self.kinds[key as usize]
    }

    pub fn slots(&self) -> &[(String, SyntaxKind)] {
        &self.slots
    }

    pub fn has(&self, key: Key, kind: SyntaxKind) -> bool {
        self.kinds(key).contains(&kind)
    }
}

/// The rows of a language resolved against its grammar.
#[derive(Debug, Default)]
pub struct Table {
    pub facts: Facts,
    rows: Vec<Option<Row>>,
    /// Node kinds that make a declaration inside them conditional (the grammar's `#conditional`,
    /// `#function`, `#loop`, `#try` and `#switch`).
    conditional: Vec<bool>,
    calls: Vec<bool>,
}

fn allowed(role: Role) -> &'static [Key] {
    use Key::*;
    match role {
        Role::Namespace => &[Name],
        Role::Type => &[Name, Modifiers],
        Role::Heritage | Role::UseGroup => &[Names],
        Role::Function | Role::Method => &[Name, Params, Modifiers, Refs],
        Role::Param => &[Name, Optional, Variadic, Modifiers, Refs],
        Role::Property | Role::Const | Role::Case => &[Item, Name, Modifiers, Refs],
        Role::GlobalConst => &[Item, Name],
        Role::Mixin => &[Names, Adapt],
        Role::UseItem => &[Names, Alias],
        Role::FileImport => &[Names, Modifiers],
        Role::Ignore => &[Refs],
        Role::Use | Role::UseKind | Role::Reference | Role::Transparent => &[],
    }
}

fn role_of(word: &str) -> Option<Role> {
    Some(match word {
        "namespace" => Role::Namespace,
        "type" => Role::Type,
        "heritage" => Role::Heritage,
        "function" => Role::Function,
        "method" => Role::Method,
        "param" => Role::Param,
        "property" => Role::Property,
        "const" => Role::Const,
        "global-const" => Role::GlobalConst,
        "case" => Role::Case,
        "mixin" => Role::Mixin,
        "use" => Role::Use,
        "use-group" => Role::UseGroup,
        "use-kind" => Role::UseKind,
        "use-item" => Role::UseItem,
        "file-import" => Role::FileImport,
        "reference" => Role::Reference,
        "transparent" => Role::Transparent,
        "ignore" => Role::Ignore,
        _ => return None,
    })
}

impl Table {
    /// Reads the rows of `source` against `grammar`; the error names the line.
    pub fn parse(grammar: &Grammar, source: &str) -> Result<Table, String> {
        let mut table = Table::default();
        let size = grammar.nodes().len();
        table.rows = vec![None; size];
        for (n, raw) in source.lines().enumerate() {
            let line = raw.split("//").next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let at = |e: String| format!("line {}: {e}", n + 1);
            let mut words = line.split_whitespace();
            let head = words.next().unwrap_or("");
            if head.starts_with(|c: char| c.is_ascii_uppercase()) {
                table.row(grammar, head, words).map_err(at)?;
            } else {
                table.fact(head, words).map_err(at)?;
            }
        }
        if table.facts.separator.is_empty() {
            return Err("no `separator` fact".into());
        }
        table.conditional = vec![false; size];
        table.calls = vec![false; size];
        for def in grammar.nodes() {
            let Some(kind) = grammar.kind_by_name(&def.name) else {
                continue;
            };
            let i = kind.index() as usize;
            table.conditional[i] = ["conditional", "function", "loop", "try", "switch"]
                .iter()
                .any(|t| grammar.has_tag(kind, t));
            table.calls[i] = grammar.has_tag(kind, "call");
        }
        Ok(table)
    }

    fn fact<'a>(&mut self, key: &str, words: impl Iterator<Item = &'a str>) -> Result<(), String> {
        let words: Vec<&str> = words.collect();
        let one = || {
            words
                .first()
                .map(|w| w.to_string())
                .ok_or_else(|| format!("`{key}` needs a word"))
        };
        let list = || words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        let kinds = || {
            words
                .iter()
                .map(|w| NameKind::of(w).ok_or_else(|| format!("unknown name kind `{w}`")))
                .collect::<Result<Vec<_>, _>>()
        };
        let f = &mut self.facts;
        match key {
            "separator" => f.separator = one()?,
            "absolute" => f.absolute = one()?,
            "relative" => f.relative = one()?,
            "lexical" => f.lexical = true,
            "fold" => f.fold = kinds()?.iter().fold(0, |m, k| m | k.bit()),
            "fallback" => f.fallback = kinds()?.iter().fold(0, |m, k| m | k.bit()),
            "constructor" => f.constructor = one()?,
            "sigil" => f.sigil = one()?,
            "dynamic-members" => f.dynamic_members = list(),
            "declaring-calls" => f.declaring_calls = list(),
            "promotion" => f.promotion = list(),
            "packages" => {
                f.packages = one()?;
                f.manifest = words
                    .get(1)
                    .map(|w| w.to_string())
                    .ok_or("`packages` needs the manifest file after the system")?;
            }
            "use-kinds" => {
                for w in &words {
                    let (word, kind) = w
                        .split_once('=')
                        .and_then(|(a, b)| Some((a, NameKind::of(b)?)))
                        .ok_or_else(|| format!("`{w}` is no `word=kind`"))?;
                    f.use_kinds.push((word.to_string(), kind));
                }
            }
            _ => return Err(format!("unknown fact `{key}`")),
        }
        Ok(())
    }

    fn row<'a>(
        &mut self,
        grammar: &Grammar,
        kind: &str,
        mut words: impl Iterator<Item = &'a str>,
    ) -> Result<(), String> {
        let node = grammar
            .kind_by_name(kind)
            .filter(|k| !k.is_token())
            .ok_or_else(|| format!("`{kind}` is no node kind of the grammar"))?;
        let role_word = words.next().ok_or("a row needs a role")?;
        let role = role_of(role_word).ok_or_else(|| format!("unknown role `{role_word}`"))?;
        let mut row = Row {
            role,
            type_kind: None,
            file: false,
            after: String::new(),
            relation: None,
            slots: Vec::new(),
            kinds: Default::default(),
        };
        for w in words {
            if let Some((key, value)) = w.split_once('=') {
                if key == "name-after" && matches!(role, Role::Type | Role::Function | Role::Method)
                {
                    row.after = value.to_string();
                    continue;
                }
                if key == "relation" {
                    row.relation = Some(match value {
                        "extends" => super::Relation::Extends,
                        "implements" => super::Relation::Implements,
                        _ => return Err(format!("unknown relation `{value}`")),
                    });
                    continue;
                }
                let (_, k) = KEYS
                    .iter()
                    .find(|(name, _)| *name == key)
                    .ok_or_else(|| format!("unknown key `{key}`"))?;
                if !allowed(role).contains(k) {
                    return Err(format!("`{key}` is no key of the role `{role_word}`"));
                }
                for name in value.split(',') {
                    if let Some((label, kind)) = name.split_once(':').filter(|_| *k == Key::Refs) {
                        grammar
                            .field_at(node, label)
                            .ok_or_else(|| format!("`{kind}` has no slot `{label}`"))?;
                        let found = grammar
                            .kind_by_name(kind)
                            .ok_or_else(|| format!("`{kind}` is no kind of the grammar"))?;
                        row.slots.push((label.to_string(), found));
                        continue;
                    }
                    let found = grammar
                        .kind_by_name(name)
                        .ok_or_else(|| format!("`{name}` is no kind of the grammar"))?;
                    row.kinds[*k as usize].push(found);
                }
            } else if w == "file" && role == Role::Namespace {
                row.file = true;
            } else if role == Role::Type && row.type_kind.is_none() {
                row.type_kind =
                    Some(TypeKind::of(w).ok_or_else(|| format!("unknown type kind `{w}`"))?);
            } else {
                return Err(format!("unexpected `{w}`"));
            }
        }
        if role == Role::Type && row.type_kind.is_none() {
            return Err("a `type` row names its kind (`class`, `message`, ...)".into());
        }
        if matches!(role, Role::Type | Role::Function | Role::Method)
            && row.kinds(Key::Name).is_empty()
        {
            return Err(format!("a `{role_word}` row needs `name=`"));
        }
        if role == Role::Heritage && row.relation.is_none() {
            return Err("a `heritage` row needs `relation=`".into());
        }
        let slot = &mut self.rows[node.index() as usize];
        if slot.is_some() {
            return Err(format!("`{kind}` has a row already"));
        }
        *slot = Some(row);
        Ok(())
    }

    pub(crate) fn row_of(&self, kind: SyntaxKind) -> Option<&Row> {
        self.rows.get(kind.index() as usize)?.as_ref()
    }

    pub(crate) fn makes_conditional(&self, kind: SyntaxKind) -> bool {
        self.conditional
            .get(kind.index() as usize)
            .copied()
            .unwrap_or(false)
    }

    /// Names are visible through the files a file imports (Protobuf), not through the project.
    pub fn sees_imports_only(&self) -> bool {
        self.rows
            .iter()
            .flatten()
            .any(|r| r.role == Role::FileImport)
    }

    pub(crate) fn is_call(&self, kind: SyntaxKind) -> bool {
        self.calls
            .get(kind.index() as usize)
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn use_kind(&self, word: &str) -> Option<NameKind> {
        let word = word.to_ascii_lowercase();
        self.facts
            .use_kinds
            .iter()
            .find(|(w, _)| *w == word)
            .map(|(_, k)| *k)
    }
}

/// A language's `symbols.rows` text; the table is built on the first use, against the grammar of
/// the file that asks.
pub struct SymbolLang {
    source: &'static str,
    table: OnceLock<Table>,
}

impl SymbolLang {
    pub const fn new(source: &'static str) -> SymbolLang {
        SymbolLang {
            source,
            table: OnceLock::new(),
        }
    }

    pub fn source(&self) -> &'static str {
        self.source
    }

    /// The rows against `grammar` (panics on a row the grammar does not hold: the build checks
    /// them first).
    pub fn table(&self, grammar: &Grammar) -> &Table {
        self.table.get_or_init(|| {
            Table::parse(grammar, self.source).unwrap_or_else(|e| panic!("symbols.rows: {e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAMMAR: &str = "language t 200 \"T\"\ntoken Whitespace #trivia #whitespace\ntoken Newline #trivia #newline\ntoken Ident\ntoken Eof\ntoken Unknown #error\nnode Doc\nnode Cls\nnode Fn\n";

    fn parse(rows: &str) -> Result<Table, String> {
        Table::parse(&Grammar::parse(GRAMMAR).unwrap(), rows)
    }

    #[test]
    fn rows_are_checked_against_the_grammar() {
        let ok = "separator .\nCls type class name=Ident\nFn function name=Ident // a note\n";
        assert!(parse(ok).is_ok());
        let bad = |rows: &str, message: &str| {
            let e = parse(&format!("separator .\n{rows}")).unwrap_err();
            assert!(e.contains(message), "{e}");
        };
        bad("Nope type class name=Ident", "`Nope` is no node kind");
        bad("Cls type class name=Missing", "`Missing` is no kind");
        bad(
            "Cls type class name=Ident bogus=Ident",
            "unknown key `bogus`",
        );
        bad(
            "Cls type class name=Ident params=Ident",
            "is no key of the role",
        );
        bad("Cls type name=Ident", "names its kind");
        bad("Cls type class", "needs `name=`");
        bad("Cls frobnicate", "unknown role");
        bad("Cls ignore\nCls ignore", "has a row already");
        bad("fold nothing", "unknown name kind");
        bad("whatever x", "unknown fact");
        assert!(parse("Cls ignore").unwrap_err().contains("separator"));
    }
}
