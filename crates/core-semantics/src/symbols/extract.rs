//! The extraction: one walk of a file's tree that records what the rows of the language say is a
//! declaration (see [`super::rows`]). Namespaces and imports are in force for what follows them,
//! a declaration inside a branch, a loop, a `try` or a function is conditional, and a type's body
//! is read member by member; a node of a type's body that no row names makes the type
//! incomplete, so a name it might hold is never proved absent.
//!
//! concept: symbol-extraction
//! what: One walk of a tree that records the types, members, functions, constants and imports the rows name.
//! tools: PHPStan class.notFound, buf IMPORT_USED
//! reads: file:symbols.rows

use super::resolve::Alias;
use super::rows::{Facts, Key, NameKind, Role, Row, Table};
use super::{
    ConstSymbol, FileImport, FileSites, FileSymbols, Flags, FunctionSymbol, ImportSite, Member,
    MemberKind, MemberSite, Name, Params, Parent, TypeKind, TypeSymbol, UseSite,
};
use core_syntax::roles::code_text;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, WalkEvent};

/// The symbols the tree under `root` declares, by the rows of `table`.
pub fn extract(root: &SyntaxNode, grammar: &Grammar, table: &Table) -> FileSymbols {
    run(root, grammar, table, false).0
}

/// What the file writes: its imports with their places, the names it uses and its methods with
/// theirs. Read by the rules that look at use, from the tree they are given.
pub fn extract_sites(root: &SyntaxNode, grammar: &Grammar, table: &Table) -> FileSites {
    run(root, grammar, table, true).1.unwrap_or_default()
}

fn run(
    root: &SyntaxNode,
    grammar: &Grammar,
    table: &Table,
    sites: bool,
) -> (FileSymbols, Option<FileSites>) {
    let mut ex = Extractor {
        grammar,
        table,
        facts: &table.facts,
        out: FileSymbols::default(),
        sites: sites.then(FileSites::default),
        frames: vec![Frame::default()],
        types: Vec::new(),
        depth: 0,
    };
    ex.frames[0].ns = ex.file_namespace(root);
    ex.walk(root);
    ex.out.declaring_calls.sort();
    ex.out.declaring_calls.dedup();
    (ex.out, ex.sites)
}

#[derive(Default)]
struct Frame {
    node: Option<SyntaxNode>,
    ns: String,
    aliases: Vec<Alias>,
}

struct Extractor<'a> {
    grammar: &'a Grammar,
    table: &'a Table,
    facts: &'a Facts,
    out: FileSymbols,
    /// Set when the caller reads what the file writes (see [`extract_sites`]).
    sites: Option<FileSites>,
    frames: Vec<Frame>,
    /// The named types the walk is inside, with their qualified names.
    types: Vec<(SyntaxNode, String)>,
    /// How many enclosing nodes make a declaration conditional.
    depth: u32,
}

/// The text of the first own child (token, or node as its code text) of one of `kinds`, after the
/// own token `after` when there is one.
fn name_in(node: &SyntaxNode, kinds: &[SyntaxKind], after: &str) -> Option<String> {
    let mut seen = after.is_empty();
    node.children_with_tokens().find_map(|e| match e {
        SyntaxElement::Token(t) if kinds.contains(&t.kind()) && seen => Some(t.text().to_string()),
        SyntaxElement::Token(t) => {
            seen |= t.text().eq_ignore_ascii_case(after);
            None
        }
        SyntaxElement::Node(n) if seen && kinds.contains(&n.kind()) => Some(code_text(&n)),
        _ => None,
    })
}

/// The own children of `kinds`, as texts.
fn names_in<'n>(
    node: &'n SyntaxNode,
    kinds: &'n [SyntaxKind],
) -> impl Iterator<Item = String> + 'n {
    node.children_with_tokens().filter_map(move |e| match e {
        SyntaxElement::Token(t) if kinds.contains(&t.kind()) => Some(t.text().to_string()),
        SyntaxElement::Node(n) if kinds.contains(&n.kind()) => Some(code_text(&n)),
        _ => None,
    })
}

impl Extractor<'_> {
    fn file_namespace(&self, root: &SyntaxNode) -> String {
        root.children()
            .filter_map(|n| {
                let row = self.table.row_of(n.kind())?;
                (row.role == Role::Namespace && row.file)
                    .then(|| name_in(&n, row.kinds(Key::Name), ""))
                    .flatten()
            })
            .next()
            .unwrap_or_default()
    }

    fn walk(&mut self, root: &SyntaxNode) {
        let mut events = root.preorder();
        while let Some(event) = events.next() {
            match event {
                WalkEvent::Enter(node) => {
                    let kind = node.kind();
                    if let Some(row) = self.table.row_of(kind) {
                        self.note_uses(&node, row);
                        match row.role {
                            Role::Namespace if !row.file => {
                                let ns = name_in(&node, row.kinds(Key::Name), &row.after)
                                    .unwrap_or_default();
                                self.frames.push(Frame {
                                    node: Some(node.clone()),
                                    ns,
                                    aliases: Vec::new(),
                                });
                            }
                            Role::Type => self.enter_type(&node, row),
                            Role::Function => self.function(&node, row),
                            Role::GlobalConst => self.global_consts(&node, row),
                            Role::Use => {
                                self.read_use(&node, "", NameKind::Type);
                                events.skip_subtree();
                            }
                            Role::FileImport => self.file_import(&node, row),
                            _ => {}
                        }
                    } else if self.table.is_call(kind) && !self.facts.declaring_calls.is_empty() {
                        self.declaring_call(&node);
                    }
                    if self.table.makes_conditional(kind) {
                        self.depth += 1;
                    }
                }
                WalkEvent::Leave(node) => {
                    if self.table.makes_conditional(node.kind()) {
                        self.depth -= 1;
                    }
                    if self
                        .frames
                        .last()
                        .is_some_and(|f| f.node.as_ref() == Some(&node))
                    {
                        self.frames.pop();
                    }
                    if self.types.last().is_some_and(|(n, _)| *n == node) {
                        self.types.pop();
                    }
                }
            }
        }
    }

    /// The scope a written name is looked up from, and a declared name is qualified by.
    fn scope(&self) -> &str {
        match self.types.last() {
            Some((_, name)) if self.facts.lexical => name,
            _ => self.frames.last().map_or("", |f| f.ns.as_str()),
        }
    }

    fn name(&self, written: &str, kind: NameKind) -> Name {
        let aliases = self.frames.last().map_or(&[][..], |f| &f.aliases);
        self.facts
            .resolve(written, kind, self.scope(), aliases)
            .into_iter()
            .map(String::into_boxed_str)
            .collect()
    }

    fn modifiers(&self, node: &SyntaxNode, row: &Row) -> u8 {
        let mut flags = 0;
        for word in names_in(node, row.kinds(Key::Modifiers)) {
            flags |= match word.to_ascii_lowercase().as_str() {
                "static" => Flags::STATIC,
                "abstract" => Flags::ABSTRACT,
                "final" => Flags::FINAL,
                "private" => Flags::PRIVATE,
                "protected" => Flags::PROTECTED,
                "readonly" => Flags::READONLY,
                _ => 0,
            };
        }
        flags
    }

    fn params(&self, node: &SyntaxNode, row: &Row) -> Option<Params> {
        let list = node.children().find(|c| row.has(Key::Params, c.kind()))?;
        let (mut min, mut max, mut variadic) = (0u16, 0u16, false);
        for param in list.children() {
            let Some(p) = self
                .table
                .row_of(param.kind())
                .filter(|r| r.role == Role::Param)
            else {
                continue;
            };
            let optional = param.children().any(|c| p.has(Key::Optional, c.kind()));
            let rest = param
                .children_with_tokens()
                .any(|e| p.has(Key::Variadic, e.kind()));
            if rest {
                variadic = true;
                continue;
            }
            max += 1;
            if !optional {
                min = max;
            }
        }
        Some(Params { min, max, variadic })
    }

    /// The names a node writes, for the sites: its own text when the row is a reference, and the
    /// types its `refs` name.
    fn note_uses(&mut self, node: &SyntaxNode, row: &Row) {
        if self.sites.is_none() {
            return;
        }
        let mut written = self.written_refs(node, row);
        if row.role == Role::Reference {
            written.push((code_text(node), self.span(node)));
        }
        let uses: Vec<UseSite> = written
            .into_iter()
            .map(|(text, range)| UseSite {
                name: self.name(&text, NameKind::Type),
                written: text.into(),
                range,
            })
            .collect();
        if let Some(sites) = &mut self.sites {
            sites.uses.extend(uses);
        }
    }

    /// The names the `refs` of a row take from a node, with where each is written: the own
    /// children of the listed kinds and the slots' children.
    fn written_refs(&self, node: &SyntaxNode, row: &Row) -> Vec<(String, core_syntax::TextRange)> {
        let kinds = row.kinds(Key::Refs);
        let mut out: Vec<(String, core_syntax::TextRange)> = node
            .children_with_tokens()
            .filter_map(|e| match e {
                SyntaxElement::Token(t) if kinds.contains(&t.kind()) => {
                    Some((t.text().to_string(), t.text_range()))
                }
                SyntaxElement::Node(n) if kinds.contains(&n.kind()) => {
                    Some((code_text(&n), self.span(&n)))
                }
                _ => None,
            })
            .collect();
        for (label, kind) in row.slots() {
            match self.grammar.slot(node, label) {
                Some(SyntaxElement::Token(t)) if t.kind() == *kind => {
                    out.push((t.text().to_string(), t.text_range()));
                }
                Some(SyntaxElement::Node(n)) if n.kind() == *kind => {
                    out.push((code_text(&n), self.span(&n)));
                }
                _ => {}
            }
        }
        out
    }

    /// Where a node is written, without the comments and blank space around it.
    fn span(&self, node: &SyntaxNode) -> core_syntax::TextRange {
        core_syntax::code_span(node, self.grammar).unwrap_or_else(|| node.text_range())
    }

    fn refs(&self, node: &SyntaxNode, row: &Row) -> Vec<Name> {
        self.written_refs(node, row)
            .into_iter()
            .map(|(w, _)| self.name(&w, NameKind::Type))
            .collect()
    }

    fn enter_type(&mut self, node: &SyntaxNode, row: &Row) {
        let conditional = self.depth > 0;
        let Some(name) = name_in(node, row.kinds(Key::Name), &row.after) else {
            self.types.push((node.clone(), self.scope().to_string()));
            return;
        };
        let qualified = self.facts.qualify(self.scope(), &name);
        let kind = row.type_kind.unwrap_or(TypeKind::Class);
        let mut ty = TypeSymbol {
            name: qualified.clone().into(),
            kind,
            flags: self.modifiers(node, row),
            parents: Vec::new(),
            mixins: Vec::new(),
            members: Vec::new(),
            conditional,
            adapted: false,
            incomplete: false,
            dynamic: false,
        };
        // Names in the heritage and in members resolve from the type's own scope; the walk pops
        // it when it leaves the node.
        self.types.push((node.clone(), qualified));
        self.members(node, row.kinds(Key::Name), &mut ty);
        ty.dynamic = ty.members.iter().any(|m| {
            m.kind == MemberKind::Method
                && self
                    .facts
                    .dynamic_members
                    .iter()
                    .any(|d| self.facts.key(NameKind::Member, &m.name) == d.as_str())
        });
        self.out.types.push(ty);
    }

    fn members(&mut self, node: &SyntaxNode, own: &[SyntaxKind], ty: &mut TypeSymbol) {
        for child in node.children() {
            if own.contains(&child.kind()) {
                continue;
            }
            let Some(row) = self.table.row_of(child.kind()) else {
                ty.incomplete = true;
                continue;
            };
            match row.role {
                Role::Transparent => self.members(&child, own, ty),
                Role::Heritage => {
                    let relation = row.relation.unwrap_or(super::Relation::Extends);
                    for written in names_in(&child, row.kinds(Key::Names)) {
                        ty.parents.push(Parent {
                            relation,
                            name: self.name(&written, NameKind::Type),
                        });
                    }
                }
                Role::Mixin => {
                    for written in names_in(&child, row.kinds(Key::Names)) {
                        ty.mixins.push(self.name(&written, NameKind::Type));
                    }
                    ty.adapted |= child.children().any(|c| row.has(Key::Adapt, c.kind()));
                }
                Role::Method => self.method(&child, row, ty),
                Role::Property => self.items(&child, row, MemberKind::Property, ty),
                Role::Const => self.items(&child, row, MemberKind::Const, ty),
                Role::Case => self.items(&child, row, MemberKind::Case, ty),
                _ => {}
            }
        }
    }

    fn method(&mut self, node: &SyntaxNode, row: &Row, ty: &mut TypeSymbol) {
        let Some(name) = name_in(node, row.kinds(Key::Name), &row.after) else {
            ty.incomplete = true;
            return;
        };
        let mut flags = self.modifiers(node, row);
        if ty.kind == TypeKind::Interface {
            flags |= Flags::ABSTRACT;
        }
        if !self.facts.constructor.is_empty()
            && self.facts.key(NameKind::Member, &name) == self.facts.constructor.as_str()
        {
            self.promoted(node, row, ty);
        }
        let refs = self.refs(node, row);
        let range = self.span(node);
        if let Some(sites) = &mut self.sites {
            sites.methods.push(MemberSite {
                owner: ty.name.clone(),
                name: name.clone().into(),
                refs: refs.clone(),
                range,
            });
        }
        ty.members.push(Member {
            name: name.into(),
            kind: MemberKind::Method,
            flags,
            params: self.params(node, row),
            refs,
        });
    }

    /// The parameters of a constructor that carry a promotion modifier are properties.
    fn promoted(&self, node: &SyntaxNode, row: &Row, ty: &mut TypeSymbol) {
        let Some(list) = node.children().find(|c| row.has(Key::Params, c.kind())) else {
            return;
        };
        for param in list.children() {
            let Some(p) = self
                .table
                .row_of(param.kind())
                .filter(|r| r.role == Role::Param)
            else {
                continue;
            };
            let words: Vec<String> = names_in(&param, p.kinds(Key::Modifiers))
                .map(|w| w.to_ascii_lowercase())
                .collect();
            if !words.iter().any(|w| self.facts.promotion.contains(w)) {
                continue;
            }
            if let Some(name) = name_in(&param, p.kinds(Key::Name), "") {
                ty.members.push(Member {
                    name: self.bare(&name).into(),
                    kind: MemberKind::Property,
                    flags: self.modifiers(&param, p),
                    params: None,
                    refs: Vec::new(),
                });
            }
        }
    }

    /// The name without the sigil a language writes in front of variables.
    fn bare(&self, name: &str) -> String {
        name.trim_start_matches(|c| self.facts.sigil.contains(c))
            .to_string()
    }

    fn items(&mut self, node: &SyntaxNode, row: &Row, kind: MemberKind, ty: &mut TypeSymbol) {
        let flags = self.modifiers(node, row);
        let items: Vec<SyntaxNode> = if row.kinds(Key::Item).is_empty() {
            vec![node.clone()]
        } else {
            node.children()
                .filter(|c| row.has(Key::Item, c.kind()))
                .collect()
        };
        for item in items {
            let Some(name) = name_in(&item, row.kinds(Key::Name), "") else {
                ty.incomplete = true;
                continue;
            };
            let name = if kind == MemberKind::Property {
                self.bare(&name)
            } else {
                name
            };
            ty.members.push(Member {
                name: name.into(),
                kind,
                flags,
                params: None,
                refs: self.refs(node, row),
            });
        }
    }

    fn function(&mut self, node: &SyntaxNode, row: &Row) {
        let Some(name) = name_in(node, row.kinds(Key::Name), &row.after) else {
            return;
        };
        self.out.functions.push(FunctionSymbol {
            name: self.facts.qualify(self.scope(), &name).into(),
            params: self.params(node, row),
            conditional: self.depth > 0,
        });
    }

    fn global_consts(&mut self, node: &SyntaxNode, row: &Row) {
        for item in self.items_below(node, row) {
            if let Some(name) = name_in(&item, row.kinds(Key::Name), "") {
                self.out.constants.push(ConstSymbol {
                    name: self.facts.qualify(self.scope(), &name).into(),
                    conditional: self.depth > 0,
                });
            }
        }
    }

    /// The items of a `global-const` row among the children of `node`, and of the children that
    /// the rows make transparent (the body of a Protobuf `extend`).
    fn items_below(&self, node: &SyntaxNode, row: &Row) -> Vec<SyntaxNode> {
        let mut items = Vec::new();
        for child in node.children() {
            if row.has(Key::Item, child.kind()) {
                items.push(child);
            } else if self
                .table
                .row_of(child.kind())
                .is_some_and(|r| r.role == Role::Transparent)
            {
                items.extend(self.items_below(&child, row));
            }
        }
        items
    }

    fn read_use(&mut self, node: &SyntaxNode, prefix: &str, kind: NameKind) {
        for child in node.children() {
            let Some(row) = self.table.row_of(child.kind()) else {
                self.read_use(&child, prefix, kind);
                continue;
            };
            match row.role {
                Role::UseGroup => {
                    let own = name_in(&child, row.kinds(Key::Names), "").unwrap_or_default();
                    let own = own.trim_end_matches(self.facts.separator.as_str());
                    let own = own
                        .strip_prefix(self.facts.absolute.as_str())
                        .unwrap_or(own);
                    let joined = if prefix.is_empty() {
                        own.to_string()
                    } else {
                        self.facts.qualify(prefix, own)
                    };
                    self.read_use(&child, &joined, kind);
                }
                Role::UseKind => {
                    let kind = child
                        .children_with_tokens()
                        .filter_map(|e| e.into_token())
                        .find_map(|t| self.table.use_kind(t.text()))
                        .unwrap_or(kind);
                    self.read_use(&child, prefix, kind);
                }
                Role::UseItem => self.use_item(&child, row, prefix, kind),
                _ => self.read_use(&child, prefix, kind),
            }
        }
    }

    fn use_item(&mut self, node: &SyntaxNode, row: &Row, prefix: &str, kind: NameKind) {
        let Some(written) = name_in(node, row.kinds(Key::Names), "") else {
            return;
        };
        let written = written
            .strip_prefix(self.facts.absolute.as_str())
            .unwrap_or(&written);
        let target = if prefix.is_empty() {
            written.to_string()
        } else {
            self.facts.qualify(prefix, written)
        };
        let alias = node
            .children()
            .find(|c| row.has(Key::Alias, c.kind()))
            .and_then(|a| name_in(&a, row.kinds(Key::Names), ""))
            .unwrap_or_else(|| {
                written
                    .rsplit(self.facts.separator.as_str())
                    .next()
                    .unwrap_or(written)
                    .to_string()
            });
        let name = self.facts.key(kind, &alias).into_owned();
        if let Some(frame) = self.frames.last_mut() {
            frame.aliases.push(Alias { kind, name, target });
        }
    }

    fn file_import(&mut self, node: &SyntaxNode, row: &Row) {
        for element in node.children_with_tokens() {
            let SyntaxElement::Token(token) = element else {
                continue;
            };
            if !row.has(Key::Names, token.kind()) {
                continue;
            }
            if let Some(path) = self.grammar.string_value(token.kind(), token.text()) {
                let public = names_in(node, row.kinds(Key::Modifiers)).any(|w| w == "public");
                let range = self.span(node);
                if let Some(sites) = &mut self.sites {
                    sites.imports.push(ImportSite {
                        path: path.as_ref().into(),
                        public,
                        range,
                    });
                }
                self.out.imports.push(FileImport {
                    path: path.into_owned().into(),
                    public,
                });
            }
        }
    }

    fn declaring_call(&mut self, node: &SyntaxNode) {
        let Some(callee) = self.grammar.slot(node, "callee") else {
            return;
        };
        let text = match callee {
            SyntaxElement::Token(t) => t.text().to_string(),
            SyntaxElement::Node(n) => code_text(&n),
        };
        let text = text
            .strip_prefix(self.facts.absolute.as_str())
            .unwrap_or(&text);
        let text = text.to_ascii_lowercase();
        if self.facts.declaring_calls.contains(&text) {
            self.out.declaring_calls.push(text.into());
        }
    }
}
