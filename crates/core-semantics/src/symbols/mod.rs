//! What a file declares to other files: the types (qualified name, kind, flags, parents, used
//! mixins, members), functions and constants, the files it imports and whether a declaration is
//! conditional. Built from the Sudsy tree by the kind sets and roles of a language's
//! `symbols.rows` ([`SymbolLang`]), never from a parser's own tree. Names only, no offsets: an
//! edit inside a body leaves the value equal, so nothing that reads it re-runs. Names a file
//! writes are resolved at extraction (namespace, imports, enclosing types) into the qualified
//! names they can stand for ([`Name`]); case folding and the fallback to the global namespace
//! are options of the language ([`Facts`]).
//!
//! concept: symbol-model
//! what: The model of what a file declares to other files: types with parents and members, functions, constants, imports; names only.
//! tools: PHPStan class.notFound, buf IMPORT_USED
//! reads: file:symbols.rows

mod extract;
mod resolve;
mod rows;

pub use extract::{extract, extract_sites};
pub use resolve::Alias;
pub use rows::{Facts, NameKind, SymbolLang, Table};

/// What the index knows about a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer<T> {
    Known(T),
    /// Proved not to exist.
    Absent,
    Unknown,
}

impl<T> Answer<T> {
    /// Whether the answer proves the name does not exist (the only answer a rule reports on).
    pub fn is_absent(&self) -> bool {
        matches!(self, Answer::Absent)
    }
}

/// What kind of type a declaration is. The words of a language's `type` rows name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Class,
    Interface,
    Trait,
    Enum,
    Message,
    Service,
}

impl TypeKind {
    pub(crate) fn of(word: &str) -> Option<TypeKind> {
        Some(match word {
            "class" => TypeKind::Class,
            "interface" => TypeKind::Interface,
            "trait" => TypeKind::Trait,
            "enum" => TypeKind::Enum,
            "message" => TypeKind::Message,
            "service" => TypeKind::Service,
            _ => return None,
        })
    }
}

/// The qualified names a written name can stand for, in the order they are tried (one for most
/// names; a name that falls back to the global namespace, or is looked up through enclosing
/// scopes, has several).
pub type Name = Box<[Box<str>]>;

/// How a parent is written: `extends` or `implements`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Extends,
    Implements,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parent {
    pub relation: Relation,
    pub name: Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    Method,
    Property,
    Const,
    Case,
}

/// Member and type flags.
pub struct Flags;

impl Flags {
    pub const STATIC: u8 = 1;
    pub const ABSTRACT: u8 = 2;
    pub const FINAL: u8 = 4;
    pub const PRIVATE: u8 = 8;
    pub const PROTECTED: u8 = 16;
    pub const READONLY: u8 = 32;
}

/// The parameters of a function or method: the fewest and most positional arguments a call may
/// pass, and whether a variadic parameter takes any number more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub min: u16,
    pub max: u16,
    pub variadic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: Box<str>,
    pub kind: MemberKind,
    pub flags: u8,
    /// Of a method.
    pub params: Option<Params>,
    /// The types a member names (a field's type, an rpc's request and response).
    pub refs: Vec<Name>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeSymbol {
    /// Qualified, as written in the file (not case folded).
    pub name: Box<str>,
    pub kind: TypeKind,
    pub flags: u8,
    pub parents: Vec<Parent>,
    /// Types whose members it takes in (PHP `use Trait;`).
    pub mixins: Vec<Name>,
    pub members: Vec<Member>,
    /// Declared inside a branch, a loop, a `try` or a function: the file does not prove it exists.
    pub conditional: bool,
    /// Its members are not those it lists: mixin adaptations (`insteadof`, `as`).
    pub adapted: bool,
    /// A member kind the rows do not know sits in its body: its member list may miss names.
    pub incomplete: bool,
    /// A magic member (`__call`, `__get`) answers any name.
    pub dynamic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionSymbol {
    pub name: Box<str>,
    pub params: Option<Params>,
    pub conditional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstSymbol {
    pub name: Box<str>,
    pub conditional: bool,
}

/// A file that the file imports (`import "a/b.proto";`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileImport {
    pub path: Box<str>,
    /// `import public`: what the imported file imports is visible too.
    pub public: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileSymbols {
    pub types: Vec<TypeSymbol>,
    pub functions: Vec<FunctionSymbol>,
    pub constants: Vec<ConstSymbol>,
    pub imports: Vec<FileImport>,
    /// Calls that declare names at run time (`define`, `class_alias`), sorted, lower case: a file
    /// that has one declares names this list does not show.
    pub declaring_calls: Vec<Box<str>>,
}

impl FileSymbols {
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
            && self.functions.is_empty()
            && self.constants.is_empty()
            && self.imports.is_empty()
            && self.declaring_calls.is_empty()
    }
}

impl TypeSymbol {
    /// The member called `name` (compared as the language compares member names).
    pub fn member(&self, facts: &Facts, name: &str) -> Option<&Member> {
        let key = facts.key(NameKind::Member, name);
        self.members
            .iter()
            .find(|m| facts.key(NameKind::Member, &m.name) == key)
    }
}

/// An import statement of a file, with where it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportSite {
    pub path: Box<str>,
    pub public: bool,
    pub range: core_syntax::TextRange,
}

/// A method of a named type, with where it is written and the types it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberSite {
    /// The qualified name of the type that holds it.
    pub owner: Box<str>,
    pub name: Box<str>,
    pub refs: Vec<Name>,
    pub range: core_syntax::TextRange,
}

/// A name the file writes where the rows say it is a reference, with the text as written and
/// where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseSite {
    /// The qualified names the written name can stand for.
    pub name: Name,
    pub written: Box<str>,
    pub range: core_syntax::TextRange,
}

/// What a file writes, read from its tree by the rules that look at use (an import nothing uses,
/// a type that two methods name): unlike [`FileSymbols`] it holds offsets and every name of a
/// body, so nothing that depends on the symbols reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileSites {
    pub imports: Vec<ImportSite>,
    /// Every name the file writes in a place the rows call a reference, resolved.
    pub uses: Vec<UseSite>,
    pub methods: Vec<MemberSite>,
}
