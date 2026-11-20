//! A language's scope data as text (`scope.rows` next to its `lang.yaml`): one line per node kind
//! (`Kind attribute...`) and one per fact (`hosts A B`). [`Spec::parse`] reads it into the
//! [`Row`]s of the binder and the facts the scope rules ask for; kind names are resolved against
//! the grammar when the [`super::Table`] is built. `//` starts a comment.
//!
//! concept: scope-tables
//! what: Which node kinds open scopes, declare, refer to or assign names, as rows resolved against the grammar.
//! tools: none (language facts as rows)
//! reads: file:scope.rows
//!
//! Row attributes (a bare word is a flag, `key=value` has a value; several `child`, `token`,
//! `keyword`, `name`, `parent-place`, `parent-flags` and `unchecked` attributes are a list):
//! - flags of the row: `var-scope`, `closed`, `hidden`, `inert`, `dynamic`, `binding`,
//!   `reference`, `text`, `op-node`, `redirect=top|outer`;
//! - `declares=FLAGS@PLACE`, `expr=FLAGS@PLACE`, `implicit=FLAGS@PLACE`: `FLAGS` are names of
//!   [`Flags`] joined with `|` (`value|function`), `PLACE` is `scope` (default), `parent`, `var`,
//!   `unseen` or `enclosing.Kind`;
//! - of a declaration: `keyword=text:FLAGS@PLACE`, `keyword-on-parent`, `parent-place=Kind:PLACE`,
//!   `parent-flags=Kind:FLAGS`, `name-child`, `name=SEL`, `name-last`, `skip-token=text`,
//!   `unchecked=Kind`;
//! - `child=SEL/FLOW` (the child nodes) and `token=SEL/FLOW` (the own name tokens).
//!
//! `SEL`: `all`, `first`, `last`, `before-op`, `except-last`, `field.name`, `kind.Kind`,
//! `last-before.text`, `after.text`, `with.text`. `FLOW`: `write`, `read-write`, `assign.text`,
//! `type`, `query`, `outside`, `delete`, `write-bare`, `through`, `through-type`, `write-var`,
//! `outside-query`, `assigns`, `declare`, `skip`, `probe`, `tag`, `capture`, or
//! `?PRED:FLOW:FLOW` (the first flow when the predicate holds for the row's node). `PRED`:
//! `has-token.text`, `first-of-kind`, `head-in.SET` (the node's first child ends with a name of
//! the set), `callee-is.text` (the node's parent's first child ends with that name).
//! Text with punctuation is quoted (`after.":"`).
//!
//! Fact lines: `names`, `hosts`, `dynamic`, `conventions`, `labels`, `methods`, `blocks`,
//! `redeclared-in`, `parameter-properties`, `globals` (words), `bare`, `top-unchecked`, `docs`,
//! `markup`, `interpolated`, `unused-prefix` (one word), `flavor NAME suffix...`,
//! `scope-name Kind name...`, `globals-if FLAVOR name...`, `set NAME word...`, and
//! `ordered key=value...` (see [`Ordered`]; `overload`, `ignored`, `value-form`, `note-form` repeat).

use super::Flags;
use super::table::{Declares, Flow, Place, Pred, Redirect, Row, Sel};
use std::collections::HashMap;

/// What a language says when the order of its code decides what a name means: the facts its
/// tags do not carry.
#[derive(Default)]
pub struct Ordered {
    /// The module-level list of the names a module exports (`__all__`); empty: none.
    pub exports: &'static str,
    /// The import module whose names are directives, never unused or redefined (`__future__`).
    pub pragma: &'static str,
    /// The exception whose handler makes a read of a missing name no finding (`NameError`).
    pub guard: &'static str,
    /// Decorators (names through imports) that make a redefined function no finding.
    pub overloads: &'static [&'static str],
    /// Names meant to be unused besides those that start with `_`.
    pub ignored: &'static [&'static str],
    /// Subscripted forms whose items are values, not types (`Literal`).
    pub value_forms: &'static [&'static str],
    /// Subscripted forms whose items after the first are values (`Annotated`).
    pub note_forms: &'static [&'static str],
    /// A string in a type position is code: its names are reads.
    pub quoted: bool,
}

type Words = &'static [&'static str];

/// The parsed text of a `scope.rows` file.
#[derive(Default)]
pub struct Spec {
    pub rows: &'static [Row],
    pub name_tokens: Words,
    pub hosts: Words,
    pub bare: bool,
    pub dynamic: Words,
    pub docs: &'static str,
    pub conventions: Words,
    pub globals: Words,
    pub conditional_globals: Vec<(&'static str, Words)>,
    pub flavors: Vec<(&'static str, Words)>,
    pub scope_names: Vec<(&'static str, &'static str)>,
    pub redeclared_in: Words,
    pub parameter_properties: Words,
    pub markup: &'static str,
    pub top_unchecked: bool,
    pub labels: Words,
    pub methods: Words,
    pub blocks: Words,
    pub interpolated: &'static str,
    pub unused_prefix: &'static str,
    pub ordered: Option<Ordered>,
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// The words of a line: split at spaces outside quotes, quotes kept.
fn words_into(line: &'static str, out: &mut Vec<&'static str>) {
    out.clear();
    let (mut start, mut quoted) = (None, false);
    for (i, c) in line.char_indices() {
        match c {
            '"' => {
                quoted = !quoted;
                start.get_or_insert(i);
            }
            c if c.is_whitespace() && !quoted => {
                if let Some(s) = start.take() {
                    out.push(&line[s..i]);
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    out.extend(start.map(|s| &line[s..]));
}

/// `word` split at the first `sep` outside quotes.
fn split(word: &'static str, sep: char) -> (&'static str, Option<&'static str>) {
    let mut quoted = false;
    for (i, c) in word.char_indices() {
        match c {
            '"' => quoted = !quoted,
            c if c == sep && !quoted => return (&word[..i], Some(&word[i + c.len_utf8()..])),
            _ => {}
        }
    }
    (word, None)
}

fn text(word: &'static str) -> &'static str {
    word.strip_prefix('"')
        .and_then(|w| w.strip_suffix('"'))
        .unwrap_or(word)
}

fn flag(name: &str) -> Result<Flags, String> {
    Ok(match name {
        "value" => Flags::VALUE,
        "type" => Flags::TYPE,
        "const" => Flags::CONST,
        "var" => Flags::VAR,
        "let" => Flags::LET,
        "function" => Flags::FUNCTION,
        "class" => Flags::CLASS,
        "parameter" => Flags::PARAMETER,
        "import" => Flags::IMPORT,
        "catch" => Flags::CATCH,
        "enum" => Flags::ENUM,
        "enum-member" => Flags::ENUM_MEMBER,
        "namespace" => Flags::NAMESPACE,
        "interface" => Flags::INTERFACE,
        "type-alias" => Flags::TYPE_ALIAS,
        "type-parameter" => Flags::TYPE_PARAMETER,
        "expression-name" => Flags::EXPRESSION_NAME,
        "redirect" => Flags::REDIRECT,
        "bare" => Flags::BARE,
        "unchecked" => Flags::UNCHECKED,
        other => return Err(format!("unknown flag `{other}`")),
    })
}

fn flags(list: &str) -> Result<Flags, String> {
    list.split('|')
        .try_fold(Flags(0), |all, name| Ok(all | flag(name)?))
}

fn place(name: &'static str) -> Result<Place, String> {
    Ok(match name {
        "scope" => Place::Scope,
        "parent" => Place::Parent,
        "var" => Place::Var,
        "unseen" => Place::Unseen,
        other => match other.strip_prefix("enclosing.") {
            Some(kind) => Place::Enclosing(kind),
            None => return Err(format!("unknown place `{other}`")),
        },
    })
}

/// `FLAGS@PLACE`.
fn flags_place(value: &'static str) -> Result<(Flags, Place), String> {
    let (f, p) = split(value, '@');
    Ok((flags(f)?, p.map_or(Ok(Place::Scope), place)?))
}

/// `head.arg`: the arg unquoted.
fn call(word: &'static str) -> (&'static str, &'static str) {
    let (head, arg) = split(word, '.');
    (head, arg.map_or("", text))
}

fn sel(word: &'static str) -> Result<Sel, String> {
    let (head, arg) = call(word);
    Ok(match head {
        "all" => Sel::All,
        "first" => Sel::First,
        "last" => Sel::Last,
        "before-op" => Sel::BeforeOp,
        "except-last" => Sel::ExceptLast,
        "field" => Sel::Field(arg),
        "kind" => Sel::Kind(arg),
        "last-before" => Sel::LastBefore(arg),
        "after" => Sel::AfterText(arg),
        "with" => Sel::WithToken(arg),
        other => return Err(format!("unknown selector `{other}`")),
    })
}

fn simple_flow(word: &'static str) -> Result<Flow, String> {
    let (head, arg) = call(word);
    Ok(match head {
        "write" => Flow::Write,
        "read-write" => Flow::ReadWrite,
        "assign" => Flow::Assign(arg),
        "type" => Flow::Type,
        "query" => Flow::Query,
        "outside" => Flow::Outside,
        "delete" => Flow::Delete,
        "write-bare" => Flow::WriteBare,
        "through" => Flow::Through,
        "through-type" => Flow::ThroughType,
        "write-var" => Flow::WriteVar,
        "outside-query" => Flow::OutsideQuery,
        "assigns" => Flow::Assigns,
        "declare" => Flow::Declare,
        "skip" => Flow::Skip,
        "probe" => Flow::Probe,
        "tag" => Flow::Tag,
        "capture" => Flow::Capture,
        other => return Err(format!("unknown flow `{other}`")),
    })
}

fn flow(word: &'static str, sets: &HashMap<&str, Words>) -> Result<Flow, String> {
    let Some(cond) = word.strip_prefix('?') else {
        return simple_flow(word);
    };
    let (pred, rest) = split(cond, ':');
    let (yes, no) = split(rest.unwrap_or(""), ':');
    let (head, arg) = call(pred);
    let pred = match head {
        "has-token" => Pred::HasToken(arg),
        "first-of-kind" => Pred::FirstOfKind,
        "head-in" => Pred::HeadIn(
            sets.get(arg)
                .ok_or_else(|| format!("unknown set `{arg}`"))?,
        ),
        "callee-is" => Pred::CalleeIs(match sets.get(arg) {
            Some(set) => set,
            None => leak(vec![arg]),
        }),
        other => return Err(format!("unknown predicate `{other}`")),
    };
    let both = |w: &'static str| simple_flow(w).map(|f| &*Box::leak(Box::new(f)));
    Ok(Flow::Cond(
        Box::leak(Box::new(pred)),
        both(yes)?,
        both(no.ok_or("a conditional flow needs two flows")?)?,
    ))
}

/// `SEL/FLOW`.
fn child(value: &'static str, sets: &HashMap<&str, Words>) -> Result<(Sel, Flow), String> {
    let (s, f) = split(value, '/');
    Ok((sel(s)?, flow(f.ok_or("expected SEL/FLOW")?, sets)?))
}

fn pair(value: &'static str) -> Result<(&'static str, &'static str), String> {
    let (a, b) = split(value, ':');
    Ok((a, b.ok_or("expected KIND:VALUE")?))
}

fn row(
    kind: &'static str,
    attrs: &[&'static str],
    sets: &HashMap<&str, Words>,
) -> Result<Row, String> {
    let mut row = Row { kind, ..Row::NONE };
    let mut d = Declares::NONE;
    let mut has_declares = false;
    // Exact capacities: the lists are leaked as boxed slices, which shrinks any excess.
    let count = |key: &str| {
        attrs
            .iter()
            .filter(|a| a.split_once('=').is_some_and(|(k, _)| k == key))
            .count()
    };
    let (mut children, mut tokens) = (
        Vec::with_capacity(count("child")),
        Vec::with_capacity(count("token")),
    );
    let (mut keywords, mut parent_places, mut parent_flags) = (
        Vec::with_capacity(count("keyword")),
        Vec::with_capacity(count("parent-place")),
        Vec::with_capacity(count("parent-flags")),
    );
    let (mut names, mut unchecked) = (
        Vec::with_capacity(count("name")),
        Vec::with_capacity(count("unchecked")),
    );
    for attr in attrs {
        let (key, value) = split(attr, '=');
        let v = value.unwrap_or("");
        match key {
            "var-scope" => row.var_scope = true,
            "closed" => row.closed = true,
            "hidden" => row.hidden = true,
            "inert" => row.inert = true,
            "dynamic" => row.dynamic = true,
            "binding" => row.binding = true,
            "reference" => row.reference = true,
            "text" => row.text = true,
            "op-node" => row.op_node = true,
            "redirect" => {
                row.redirect = Some(match v {
                    "top" => Redirect::Top,
                    "outer" => Redirect::Outer,
                    other => return Err(format!("unknown redirect `{other}`")),
                });
            }
            "implicit" => row.implicit = Some(flags_place(v)?),
            "declares" => {
                has_declares = true;
                (d.flags, d.place) = flags_place(v)?;
            }
            "expr" => d.expression = Some(flags_place(v)?),
            "keyword" => {
                let (k, rest) = pair(v)?;
                let (f, p) = flags_place(rest)?;
                keywords.push((text(k), f, p));
            }
            "keyword-on-parent" => d.keyword_on_parent = true,
            "parent-place" => {
                let (k, p) = pair(v)?;
                parent_places.push((k, place(p)?));
            }
            "parent-flags" => {
                let (k, f) = pair(v)?;
                parent_flags.push((k, flags(f)?));
            }
            "name-child" => d.name_child = true,
            "name" => names.push(sel(v)?),
            "name-last" => d.name_last = true,
            "skip-token" => d.skip_token = text(v),
            "unchecked" => unchecked.push(v),
            "child" => children.push(child(v, sets)?),
            "token" => tokens.push(child(v, sets)?),
            other => return Err(format!("unknown attribute `{other}`")),
        }
    }
    d.keywords = leak(keywords);
    d.parent_places = leak(parent_places);
    d.parent_flags = leak(parent_flags);
    d.names = leak(names);
    d.unchecked = leak(unchecked);
    row.declares = has_declares.then_some(d);
    row.children = leak(children);
    row.tokens = leak(tokens);
    Ok(row)
}

fn ordered(attrs: &[&'static str]) -> Result<Ordered, String> {
    let mut o = Ordered::default();
    let (mut overloads, mut ignored) = (Vec::new(), Vec::new());
    let (mut value_forms, mut note_forms) = (Vec::new(), Vec::new());
    for attr in attrs {
        let (key, value) = split(attr, '=');
        let v = value.map_or("", text);
        match key {
            "exports" => o.exports = v,
            "pragma" => o.pragma = v,
            "guard" => o.guard = v,
            "overload" => overloads.push(v),
            "ignored" => ignored.push(v),
            "value-form" => value_forms.push(v),
            "note-form" => note_forms.push(v),
            "quoted" => o.quoted = true,
            other => return Err(format!("unknown ordered attribute `{other}`")),
        }
    }
    o.overloads = leak(overloads);
    o.ignored = leak(ignored);
    o.value_forms = leak(value_forms);
    o.note_forms = leak(note_forms);
    Ok(o)
}

impl Spec {
    /// Whether a `scope.rows` file has an `ordered` line, read without parsing the file (a rule
    /// that is built for a language that never lints reads no more than this).
    pub fn is_ordered(source: &str) -> bool {
        source.lines().any(|l| {
            l.split("//")
                .next()
                .and_then(|l| l.split_whitespace().next())
                == Some("ordered")
        })
    }

    /// Read the text of a `scope.rows` file.
    pub fn parse(source: &'static str) -> Result<Spec, String> {
        let mut spec = Spec::default();
        let mut sets: HashMap<&str, Words> = HashMap::new();
        let mut w = Vec::new();
        let strip = |line: &'static str| line.split_once("//").map_or(line, |(l, _)| l);
        for line in source.lines().filter(|l| l.starts_with("set")) {
            words_into(strip(line), &mut w);
            if w.first() == Some(&"set") {
                sets.insert(w[1], leak(w[2..].to_vec()));
            }
        }
        let mut rows = Vec::with_capacity(source.lines().count());
        for (n, line) in source.lines().enumerate() {
            words_into(strip(line), &mut w);
            if w.is_empty() || w[0] == "set" {
                continue;
            }
            let n = n + 1;
            let at = |e: String| format!("scope.rows line {n}: {e}");
            let (head, rest) = (w[0], &w[1..]);
            let one = || text(rest.first().copied().unwrap_or(""));
            let list = || leak(rest.iter().map(|w| text(w)).collect::<Vec<_>>());
            match head {
                "names" => spec.name_tokens = list(),
                "hosts" => spec.hosts = list(),
                "dynamic" => spec.dynamic = list(),
                "conventions" => spec.conventions = list(),
                "labels" => spec.labels = list(),
                "methods" => spec.methods = list(),
                "blocks" => spec.blocks = list(),
                "redeclared-in" => spec.redeclared_in = list(),
                "parameter-properties" => spec.parameter_properties = list(),
                "globals" => spec.globals = list(),
                "bare" => spec.bare = true,
                "top-unchecked" => spec.top_unchecked = true,
                "docs" => spec.docs = one(),
                "markup" => spec.markup = one(),
                "interpolated" => spec.interpolated = one(),
                "unused-prefix" => spec.unused_prefix = one(),
                "flavor" => spec.flavors.push((one(), leak(rest[1..].to_vec()))),
                "globals-if" => spec
                    .conditional_globals
                    .push((one(), leak(rest[1..].to_vec()))),
                "scope-name" => spec
                    .scope_names
                    .extend(rest[1..].iter().map(|name| (text(rest[0]), *name))),
                "ordered" => spec.ordered = Some(ordered(rest).map_err(at)?),
                kind if kind.starts_with(|c: char| c.is_ascii_uppercase()) => {
                    rows.push(row(kind, rest, &sets).map_err(at)?);
                }
                other => return Err(at(format!("unknown line `{other}`"))),
            }
        }
        spec.rows = leak(rows);
        Ok(spec)
    }
}
