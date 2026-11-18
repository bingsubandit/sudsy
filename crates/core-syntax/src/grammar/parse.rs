use super::*;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::HashSet;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("grammar line {line}: {message}")]
pub struct GrammarError {
    pub line: usize,
    pub message: String,
}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, GrammarError> {
    Err(GrammarError {
        line,
        message: message.into(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Lexeme<'a> {
    Ident(&'a str),
    Str(Cow<'a, str>),
    Tag(&'a str),
    Int(u32),
    Sym(char),
}

/// One declaration: its first line and its lexemes, a range of the shared lexeme list.
struct Decl {
    line: usize,
    start: usize,
    end: usize,
}

struct Lexed<'a> {
    lexemes: Vec<Lexeme<'a>>,
    decls: Vec<Decl>,
}

impl<'a> Lexed<'a> {
    fn of(&self, d: &Decl) -> &[Lexeme<'a>] {
        &self.lexemes[d.start..d.end]
    }
}

fn lex_line<'a>(
    line_no: usize,
    text: &'a str,
    out: &mut Vec<Lexeme<'a>>,
) -> Result<(), GrammarError> {
    let b = text.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'-';
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            c if c.is_ascii_whitespace() => i += 1,
            b'/' if b.get(i + 1) == Some(&b'/') => break,
            b'"' => {
                i += 1;
                let start = i;
                // Owned only when an escape forces a copy.
                let mut owned: Option<String> = None;
                loop {
                    match b.get(i) {
                        None => return err(line_no, "unterminated string"),
                        Some(b'"') => break,
                        Some(b'\\') => {
                            let s = owned.get_or_insert_with(|| text[start..i].to_string());
                            i += 1;
                            match b.get(i) {
                                Some(&e @ (b'"' | b'\\')) => s.push(e as char),
                                _ => return err(line_no, "bad escape (use \\\" or \\\\)"),
                            }
                            i += 1;
                        }
                        Some(_) => {
                            let from = i;
                            while i < b.len() && b[i] != b'"' && b[i] != b'\\' {
                                i += 1;
                            }
                            if let Some(s) = owned.as_mut() {
                                s.push_str(&text[from..i]);
                            }
                        }
                    }
                }
                out.push(Lexeme::Str(match owned {
                    Some(s) => Cow::Owned(s),
                    None => Cow::Borrowed(&text[start..i]),
                }));
                i += 1;
            }
            b'#' => {
                i += 1;
                let start = i;
                while i < b.len() && word(b[i]) {
                    i += 1;
                }
                if i == start {
                    return err(line_no, "empty tag");
                }
                out.push(Lexeme::Tag(&text[start..i]));
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                match text[start..i].parse() {
                    Ok(n) => out.push(Lexeme::Int(n)),
                    Err(_) => return err(line_no, "number too large"),
                }
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (word(b[i]) || b[i] == b'_') {
                    i += 1;
                }
                out.push(Lexeme::Ident(&text[start..i]));
            }
            b'=' | b':' | b'?' | b'*' => {
                out.push(Lexeme::Sym(c as char));
                i += 1;
            }
            _ => {
                let other = text[i..].chars().next().unwrap_or('?');
                return err(line_no, format!("unexpected character {other:?}"));
            }
        }
    }
    Ok(())
}

fn split_decls(source: &str) -> Result<Lexed<'_>, GrammarError> {
    let mut lexemes = Vec::with_capacity(source.len() / 5);
    let mut decls: Vec<Decl> = Vec::with_capacity(source.len() / 60);
    for (i, raw) in source.lines().enumerate() {
        let line_no = i + 1;
        let start = lexemes.len();
        lex_line(line_no, raw, &mut lexemes)?;
        if lexemes.len() == start {
            continue;
        }
        if raw.starts_with(char::is_whitespace) {
            match decls.last_mut() {
                Some(d) => d.end = lexemes.len(),
                None => return err(line_no, "continuation line before any declaration"),
            }
        } else {
            decls.push(Decl {
                line: line_no,
                start,
                end: lexemes.len(),
            });
        }
    }
    Ok(Lexed { lexemes, decls })
}

/// SHA-256 over the lexemes of a grammar source (the grammar's version).
pub(super) fn content_hash(source: &str) -> String {
    use std::fmt::Write;
    let lexed = split_decls(source).expect("a parsed grammar lexes again");
    let mut h = Sha256::new();
    let mut buf = [0u8; 4];
    let mut num = String::with_capacity(10);
    for d in &lexed.decls {
        for l in lexed.of(d) {
            let (tag, text): (u8, &str) = match l {
                Lexeme::Ident(s) => (b'i', s),
                Lexeme::Str(s) => (b's', s),
                Lexeme::Tag(s) => (b't', s),
                Lexeme::Int(n) => {
                    num.clear();
                    let _ = write!(num, "{n}");
                    (b'n', &num)
                }
                Lexeme::Sym(c) => (b'y', c.encode_utf8(&mut buf)),
            };
            h.update([tag]);
            h.update(text.as_bytes());
            h.update([0]);
        }
        h.update(b"\n");
    }
    let mut out = String::with_capacity(64);
    for b in h.finalize() {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Cursor over one declaration's lexemes.
struct Cursor<'a> {
    line: usize,
    rest: &'a [Lexeme<'a>],
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&'a Lexeme<'a>> {
        self.rest.first()
    }
    fn next(&mut self) -> Option<&'a Lexeme<'a>> {
        let l = self.rest.first();
        if l.is_some() {
            self.rest = &self.rest[1..];
        }
        l
    }
    fn ident(&mut self, what: &str) -> Result<&'a str, GrammarError> {
        match self.next() {
            Some(Lexeme::Ident(s)) => Ok(s),
            _ => err(self.line, format!("expected {what}")),
        }
    }
    fn string(&mut self, what: &str) -> Result<&'a str, GrammarError> {
        match self.next() {
            Some(Lexeme::Str(s)) => Ok(s),
            _ => err(self.line, format!("expected {what}")),
        }
    }
    fn tags(&mut self) -> Vec<&'a str> {
        let mut tags = Vec::new();
        while let Some(Lexeme::Tag(t)) = self.peek() {
            tags.push(*t);
            self.next();
        }
        tags
    }
}

/// An item before names are resolved to kinds.
struct RawItem<'a> {
    label: Option<&'a str>,
    field: Option<&'a str>,
    target: RawTarget<'a>,
    cardinality: Cardinality,
}
enum RawTarget<'a> {
    Name(&'a str),
    Tag(&'a str),
}

struct RawKind<'a> {
    line: usize,
    name: &'a str,
    is_token: bool,
    text: Option<&'a str>,
    tags: Vec<&'a str>,
    body: Vec<RawItem<'a>>,
}

fn parse_body<'a>(c: &mut Cursor<'a>) -> Result<Vec<RawItem<'a>>, GrammarError> {
    let mut items = Vec::new();
    while let Some(l) = c.next() {
        let mut field = None;
        let (label, target) = match l {
            Lexeme::Tag(t) => (None, RawTarget::Tag(t)),
            Lexeme::Ident(first) => {
                if c.peek() == Some(&Lexeme::Sym('=')) {
                    c.next();
                    field = Some(c.ident("a role field after `=`")?);
                    if c.peek() != Some(&Lexeme::Sym(':')) {
                        return err(c.line, "expected `:` after the role field");
                    }
                }
                if c.peek() == Some(&Lexeme::Sym(':')) {
                    c.next();
                    let target = match c.next() {
                        Some(Lexeme::Ident(n)) => RawTarget::Name(n),
                        Some(Lexeme::Tag(t)) => RawTarget::Tag(t),
                        _ => return err(c.line, "expected a kind or #tag after the label"),
                    };
                    (Some(*first), target)
                } else {
                    (None, RawTarget::Name(first))
                }
            }
            _ => return err(c.line, "expected a body item"),
        };
        let cardinality = match c.peek() {
            Some(Lexeme::Sym('?')) => {
                c.next();
                Cardinality::Optional
            }
            Some(Lexeme::Sym('*')) => {
                c.next();
                Cardinality::Many
            }
            _ => Cardinality::One,
        };
        items.push(RawItem {
            label,
            field,
            target,
            cardinality,
        });
    }
    Ok(items)
}

pub(super) fn parse(source: &str) -> Result<Grammar, GrammarError> {
    let lexed = split_decls(source)?;

    let mut language: Option<(String, LanguageId, String)> = None;
    let mut comments = CommentSyntax::default();
    let mut suppressions = Vec::new();
    let mut requires_roles = false;
    let mut operators: HashMap<String, String> = HashMap::new();
    let mut hoisted: Vec<String> = Vec::new();
    let mut precedence_rows: Vec<PrecedenceRow> = Vec::new();
    let mut keys = KeySyntax::default();
    let mut strings: Vec<StringSyntax> = Vec::new();
    let mut trivia: Vec<TriviaRow> = Vec::new();
    let mut limits = Limits::default();
    let mut raw: Vec<RawKind> = Vec::with_capacity(lexed.decls.len());

    for d in &lexed.decls {
        let mut c = Cursor {
            line: d.line,
            rest: lexed.of(d),
        };
        let keyword = c.ident("a declaration keyword")?;
        match keyword {
            "language" => {
                let name = c.ident("language name")?;
                let id = match c.next() {
                    Some(Lexeme::Int(n)) if (1..=LanguageId::MAX as u32).contains(n) => *n as u8,
                    _ => return err(d.line, "language id must be 1..=31"),
                };
                let title = match c.peek() {
                    Some(Lexeme::Str(_)) => c.string("display name")?,
                    _ => name,
                };
                if language
                    .replace((name.to_string(), LanguageId::new(id), title.to_string()))
                    .is_some()
                {
                    return err(d.line, "duplicate language declaration");
                }
            }
            "comments" => {
                while let Some(l) = c.next() {
                    match l {
                        Lexeme::Ident("line") => {
                            comments.line = Some(c.string("line comment prefix")?.to_string())
                        }
                        Lexeme::Ident("block") => {
                            comments.block = Some((
                                c.string("block comment start")?.to_string(),
                                c.string("block comment end")?.to_string(),
                            ))
                        }
                        _ => return err(d.line, "expected `line` or `block`"),
                    }
                }
            }
            "roles" => requires_roles = true,
            "keys" => {
                while let Some(Lexeme::Ident(option)) = c.next() {
                    match *option {
                        "lowercase" => keys.lowercase = true,
                        "decode" => keys.decode = true,
                        "strip" => keys.strip = Some(c.string("a prefix")?.to_string()),
                        "alias" => keys.alias_name = Some(c.ident("a node kind")?.to_string()),
                        "merge" => keys.merge = Some(c.string("a merge key")?.to_string()),
                        "accessors" => {
                            while let Some(Lexeme::Str(w)) = c.peek() {
                                keys.accessors.push(w.to_string());
                                c.next();
                            }
                        }
                        other => return err(d.line, format!("unknown keys option `{other}`")),
                    }
                }
            }
            "hoisted" => {
                while c.peek().is_some() {
                    hoisted.push(c.string("a declaration keyword")?.to_string());
                }
            }
            "operators" => {
                let kind = c.ident("an operator kind")?;
                if !crate::roles::OPERATOR_KINDS.iter().any(|k| k.0 == kind) {
                    return err(d.line, format!("unknown operator kind `{kind}`"));
                }
                while c.peek().is_some() {
                    let text = c.string("operator text")?;
                    if operators
                        .insert(text.to_string(), kind.to_string())
                        .is_some()
                    {
                        return err(d.line, format!("operator `{text}` has two kinds"));
                    }
                }
            }
            "precedence" => {
                let level = match c.next() {
                    Some(Lexeme::Int(n)) if *n <= u8::MAX as u32 => *n as u8,
                    _ => return err(d.line, "precedence level must be 0..=255"),
                };
                let assoc = match c.ident("an associativity")? {
                    "left" => Assoc::Left,
                    "right" => Assoc::Right,
                    "same" => Assoc::Same,
                    "none" => Assoc::None,
                    other => return err(d.line, format!("unknown associativity `{other}`")),
                };
                let mut ops = Vec::new();
                while c.peek().is_some() {
                    let text = c.string("operator text")?;
                    if precedence_rows
                        .iter()
                        .any(|r| r.operators.iter().any(|o| o == text))
                        || ops.iter().any(|o| o == text)
                    {
                        return err(d.line, format!("operator `{text}` has two precedences"));
                    }
                    ops.push(text.to_string());
                }
                precedence_rows.push(PrecedenceRow {
                    level,
                    assoc,
                    operators: ops,
                });
            }
            "string" => strings.extend(string_row(&mut c)?),
            "trivia" => trivia.push(trivia_row(&mut c)?),
            "limits" => limits = limits_row(&mut c)?,
            "suppress" => {
                let role = c.ident("suppression role")?;
                let marker = c.string("suppression marker")?;
                suppressions.push(Suppression {
                    role: role.to_string(),
                    marker: marker.to_string(),
                });
            }
            "token" | "node" => {
                let is_token = keyword == "token";
                let name = c.ident("a kind name")?;
                let mut text = None;
                if is_token && let Some(Lexeme::Str(s)) = c.peek() {
                    text = Some(&**s);
                    c.next();
                }
                let tags = c.tags();
                let mut body = Vec::new();
                if c.peek() == Some(&Lexeme::Sym('=')) {
                    if is_token {
                        return err(d.line, "tokens have no body");
                    }
                    c.next();
                    body = parse_body(&mut c)?;
                }
                if c.peek().is_some() {
                    return err(d.line, "unexpected trailing input");
                }
                raw.push(RawKind {
                    line: d.line,
                    name,
                    is_token,
                    text,
                    tags,
                    body,
                });
            }
            other => return err(d.line, format!("unknown declaration `{other}`")),
        }
    }

    let Some((language, language_id, title)) = language else {
        return err(1, "missing `language` declaration");
    };
    let mut grammar = resolve(
        language,
        language_id,
        title,
        comments,
        suppressions,
        requires_roles,
        operators,
        hoisted,
        precedence_rows,
        keys,
        raw,
        source,
    )?;
    let mut rows = Vec::with_capacity(strings.len());
    for row in strings {
        match grammar.by_name.get(&row.token) {
            Some(kind) => rows.push((*kind, row)),
            None => return err(1, format!("string row of unknown token `{}`", row.token)),
        }
    }
    rows.sort_by_key(|(kind, _)| kind.index());
    grammar.string_first = vec![u32::MAX; grammar.tokens.len()];
    for (i, (kind, _)) in rows.iter().enumerate().rev() {
        grammar.string_first[kind.index() as usize] = i as u32;
    }
    grammar.strings = rows;
    for row in &mut trivia {
        row.kind = grammar.by_name.get(&row.token).copied();
        if row.kind.is_none() {
            return err(1, format!("trivia row of unknown token `{}`", row.token));
        }
    }
    grammar.trivia = TriviaSyntax::new(trivia);
    grammar.limits = limits;
    Ok(grammar)
}

/// A `trivia <Token> attribute...` row (see [`TriviaRow`]): `line "p"...`, `not "p"...`,
/// `first "p"...`, `block "open" "close"`, `nested`, `lines "open" "close"`, `continuation`,
/// `unicode`.
fn trivia_row(c: &mut Cursor<'_>) -> Result<TriviaRow, GrammarError> {
    let mut row = TriviaRow {
        token: c.ident("a token kind")?.to_string(),
        ..TriviaRow::default()
    };
    let strings = |c: &mut Cursor<'_>| {
        let mut out = Vec::new();
        while let Some(Lexeme::Str(s)) = c.peek() {
            out.push(s.to_string());
            c.next();
        }
        out
    };
    while c.peek().is_some() {
        match c.ident("a trivia attribute")? {
            "line" => row.line = strings(c),
            "not" => row.not = strings(c),
            "first" => row.first = strings(c),
            "block" => row.block = Some((c.string("open")?.into(), c.string("close")?.into())),
            "lines" => row.lines = Some((c.string("open")?.into(), c.string("close")?.into())),
            "nested" => row.nested = true,
            "continuation" => row.continuation = true,
            "unicode" => row.unicode = true,
            other => return err(c.line, format!("unknown trivia attribute `{other}`")),
        }
    }
    Ok(row)
}

/// A `limits` row: `stack <MB>`, `bytes <MB>`, `depth <n>`, `run "chars"`, `tagged "c" <n>`.
fn limits_row(c: &mut Cursor<'_>) -> Result<Limits, GrammarError> {
    let mut l = Limits::default();
    let line = c.line;
    let int = |c: &mut Cursor<'_>| match c.next() {
        Some(Lexeme::Int(n)) => Ok(*n as usize),
        _ => err(line, "expected a number"),
    };
    while c.peek().is_some() {
        match c.ident("a limit")? {
            "stack" => l.stack_mb = int(c)?,
            "bytes" => l.bytes_mb = int(c)?,
            "depth" => l.depth = int(c)?,
            "run" => c
                .string("characters")?
                .bytes()
                .for_each(|b| l.run[b as usize] = true),
            "tagged" => {
                let mark = c.string("a mark")?.as_bytes()[0];
                l.tagged = Some((mark, int(c)?));
            }
            other => return err(line, format!("unknown limit `{other}`")),
        }
    }
    Ok(l)
}

/// A `string <Token>... attribute...` row (one entry per token) (see [`StringSyntax`] for the meaning of each attribute):
/// `quote "c"...`, `triple`, `prefix "letters"`, `raw "letters"`, `double`, `escape "c"`,
/// `map <"c"|code> <code>...`, `hex "c" <digits>...`, `octal <digits>`, `named "c"`, `surrogates`,
/// `continuation`, `fold`, `unknown keep|drop|invalid`, `replace <code>`.
fn string_row(c: &mut Cursor<'_>) -> Result<Vec<StringSyntax>, GrammarError> {
    let line = c.line;
    let mut tokens = vec![c.ident("a token kind")?];
    while let Some(Lexeme::Ident(t)) = c.peek() {
        if !t.starts_with(char::is_uppercase) {
            break;
        }
        tokens.push(t);
        c.next();
    }
    let mut s = StringSyntax::default();
    let ch = |l: Option<&Lexeme<'_>>| match l {
        Some(Lexeme::Str(t)) if t.chars().count() == 1 => t.chars().next(),
        Some(Lexeme::Int(n)) => char::from_u32(*n),
        _ => None,
    };
    let int = |l: Option<&Lexeme<'_>>| match l {
        Some(Lexeme::Int(n)) => Some(*n as usize),
        _ => None,
    };
    while c.peek().is_some() {
        let key = c.ident("a string attribute")?;
        let bad = || GrammarError {
            line,
            message: format!("bad `{key}` in a string row"),
        };
        match key {
            "quote" => {
                while let Some(Lexeme::Str(_)) = c.peek() {
                    s.quotes.extend(ch(c.next()));
                }
            }
            "prefix" => s.prefix = c.string("prefix letters")?.to_string(),
            "raw" => s.raw = c.string("raw prefix letters")?.to_string(),
            "triple" => s.triple = true,
            "double" => s.doubled = true,
            "surrogates" => s.surrogates = true,
            "continuation" => s.continuation = true,
            "fold" => s.fold = true,
            "escape" => s.escape = Some(ch(c.next()).ok_or_else(bad)?),
            "named" => s.named = Some(ch(c.next()).ok_or_else(bad)?),
            "octal" => s.octal = int(c.next()).ok_or_else(bad)?,
            "replace" => s.replacement = Some(ch(c.next()).ok_or_else(bad)?),
            "unknown" => {
                s.unknown = match c.ident("keep, drop or invalid")? {
                    "keep" => Unknown::Keep,
                    "drop" => Unknown::Drop,
                    "invalid" => Unknown::Invalid,
                    _ => return Err(bad()),
                }
            }
            "map" | "hex" => {
                while matches!(c.peek(), Some(Lexeme::Str(_) | Lexeme::Int(_))) {
                    let from = ch(c.next()).ok_or_else(bad)?;
                    let to = c.next();
                    if key == "hex" {
                        s.hex.push((from, int(to).ok_or_else(bad)?));
                    } else {
                        s.map.push((from, ch(to).ok_or_else(bad)?));
                    }
                }
            }
            other => return err(line, format!("unknown string attribute `{other}`")),
        }
    }
    s.simple = s.prefix.is_empty()
        && !(s.triple || s.doubled || s.fold || !s.raw.is_empty())
        && !s.quotes.is_empty()
        && s.quotes.iter().all(char::is_ascii)
        && s.escape.is_none_or(|e| e.is_ascii());
    Ok(tokens
        .into_iter()
        .map(|t| StringSyntax {
            token: t.to_string(),
            ..s.clone()
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    language: String,
    language_id: LanguageId,
    title: String,
    comments: CommentSyntax,
    suppressions: Vec<Suppression>,
    requires_roles: bool,
    operators: HashMap<String, String>,
    hoisted: Vec<String>,
    precedence_rows: Vec<PrecedenceRow>,
    keys: KeySyntax,
    raw: Vec<RawKind>,
    source: &str,
) -> Result<Grammar, GrammarError> {
    // Pass 1: assign ids.
    let mut by_name: HashMap<String, SyntaxKind> = HashMap::new();
    let (mut n_nodes, mut n_tokens) = (0u32, 0u32);
    let mut kinds = Vec::new();
    for r in &raw {
        let counter = if r.is_token {
            &mut n_tokens
        } else {
            &mut n_nodes
        };
        if *counter >= SyntaxKind::MAX_INDEX {
            return err(r.line, "too many kinds of this class (max 8388608)");
        }
        let kind = SyntaxKind::new(language_id, r.is_token, *counter);
        *counter += 1;
        if by_name.insert(r.name.to_string(), kind).is_some() {
            return err(r.line, format!("duplicate kind `{}`", r.name));
        }
        kinds.push(kind);
    }

    // Tag membership: tag -> Some(is_token) when its members are uniform, None when mixed.
    // Mixed tags are fine as categories but cannot be used as a field type.
    let mut tag_class: HashMap<&str, Option<bool>> = HashMap::new();
    for r in &raw {
        for t in &r.tags {
            let class = tag_class.entry(*t).or_insert(Some(r.is_token));
            if *class != Some(r.is_token) {
                *class = None;
            }
        }
    }

    // One shared string per distinct tag.
    let interned: HashMap<&str, Arc<str>> = tag_class.keys().map(|t| (*t, Arc::from(*t))).collect();

    // Pass 2: resolve bodies.
    let mut nodes = Vec::new();
    let mut tokens = Vec::new();
    for (r, kind) in raw.iter().zip(kinds) {
        let mut body = Vec::new();
        let mut labels: HashSet<&str> = HashSet::new();
        for it in &r.body {
            let target = match &it.target {
                RawTarget::Name(n) => match by_name.get(*n) {
                    Some(k) => Target::Kind(*k),
                    None => return err(r.line, format!("unknown kind `{n}`")),
                },
                RawTarget::Tag(t) => match tag_class.get(t) {
                    Some(Some(_)) => Target::Tag(interned[t].clone()),
                    // A union of nodes and tokens: found by `Grammar::slot`, no typed accessor.
                    Some(None) => Target::Tag(interned[t].clone()),
                    None => return err(r.line, format!("no kind carries tag #{t}")),
                },
            };
            // Accessor names are unique; several items may fill one role field.
            if let Some(l) = it.label
                && !labels.insert(l)
            {
                return err(r.line, format!("duplicate label `{l}`"));
            }
            body.push(Item {
                label: it.label.map(str::to_string),
                field: it.field.map(str::to_string),
                target,
                cardinality: it.cardinality.clone(),
            });
        }
        let def = KindDef {
            name: r.name.to_string(),
            kind,
            text: r.text.map(str::to_string),
            tags: r.tags.iter().map(|t| interned[t].clone()).collect(),
            body,
        };
        if r.is_token {
            tokens.push(def)
        } else {
            nodes.push(def)
        }
    }

    let mut grammar = Grammar {
        language,
        language_id,
        title,
        comments,
        suppressions,
        requires_roles,
        operators,
        hoisted,
        precedence_rows,
        keys,
        strings: Vec::new(),
        string_first: Vec::new(),
        trivia: TriviaSyntax::default(),
        limits: Limits::default(),
        token_flags: super::hot_flags(&tokens),
        node_flags: super::hot_flags(&nodes),
        nodes,
        tokens,
        by_name,
        source: source.into(),
        hash: OnceLock::new(),
        bodies: Vec::new(),
        body_start: Vec::new(),
        pool: Vec::new(),
        skip: Vec::new(),
        tag_nodes: HashMap::new(),
    };
    grammar.resolve();
    grammar.keys.alias = grammar
        .keys
        .alias_name
        .as_deref()
        .and_then(|n| grammar.by_name.get(n).copied());
    if let Some(kind) = grammar.keys.alias {
        grammar.keys.alias_in = grammar
            .nodes
            .iter()
            .filter(|d| d.body.iter().any(|i| i.target == Target::Kind(kind)))
            .map(|d| d.kind)
            .collect();
    }
    grammar.keys.plain = !grammar.keys.lowercase
        && !grammar.keys.decode
        && grammar.keys.strip.is_none()
        && grammar.keys.alias_name.is_none();
    grammar.keys.parts = grammar.tokens.iter().any(|d| d.has_tag("key-part"));
    Ok(grammar)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
// sample
language mini 7
comments line "//" block "/*" "*/"
suppress disable-next-line "sudsy-disable-next-line"
token Space #trivia
token Comma "," #separator
token Str #literal
node Pair #key-value = key:Str Comma value:#value
node Array #value = items:#value* // trailing comment
node Lit #value = value:#literal
node Long =
   a:Str
   b:Str?
"#;

    #[test]
    fn loads_a_grammar() {
        let g = Grammar::parse(SAMPLE).unwrap();
        assert_eq!(g.language, "mini");
        assert_eq!(g.language_id, LanguageId::new(7));
        assert_eq!(g.comments.line.as_deref(), Some("//"));
        assert_eq!(g.comments.block, Some(("/*".into(), "*/".into())));
        assert_eq!(g.suppressions.len(), 1);
        assert_eq!(g.tokens().len(), 3);
        assert_eq!(g.nodes().len(), 4);
        let comma = g.kind_by_name("Comma").unwrap();
        assert!(comma.is_token());
        assert_eq!(g.def(comma).unwrap().text.as_deref(), Some(","));
        assert!(g.has_tag(comma, "separator"));
        let pair = g.def(g.kind_by_name("Pair").unwrap()).unwrap();
        assert_eq!(pair.fields().count(), 2);
        assert_eq!(
            g.def(g.kind_by_name("Long").unwrap()).unwrap().body.len(),
            2
        );
        assert!(g.is_trivia(g.kind_by_name("Space").unwrap()));
        assert_eq!(g.kinds_with_tag("value").count(), 2);
    }

    #[test]
    fn hash_ignores_comments_and_layout_but_not_order() {
        let a = Grammar::parse(SAMPLE).unwrap();
        let b = Grammar::parse(
            &SAMPLE
                .replace("// sample", "// other\n\n")
                .replace("  ", " "),
        )
        .unwrap();
        assert_eq!(a.hash(), b.hash());
        let swapped = SAMPLE.replace(
            "token Space #trivia\ntoken Comma \",\" #separator",
            "token Comma \",\" #separator\ntoken Space #trivia",
        );
        assert_ne!(a.hash(), Grammar::parse(&swapped).unwrap().hash());
    }

    #[test]
    fn checks_roles_and_resolves_field_aliases() {
        let head = "language x 1\nroles\ntoken T\nnode Id #reference = name:T\n";
        let ok = format!(
            "{head}node Bin #binary = left:Id operator:T r=right:Id\nnode Skip #opaque = a:T"
        );
        let g = Grammar::parse(&ok).unwrap();
        assert!(g.check_roles().is_empty());
        let bad = format!(
            "{head}node Bin #binary = left:Id operator:T\nnode None = a:T\nnode Both #opaque #reference = name:T"
        );
        let errors = Grammar::parse(&bad).unwrap().check_roles().join("; ");
        for m in [
            "Bin: role #binary needs field `right`",
            "None: no role",
            "Both: #opaque with a role",
        ] {
            assert!(errors.contains(m), "{errors}");
        }
        assert!(
            Grammar::parse(&bad.replace("roles\n", ""))
                .unwrap()
                .check_roles()
                .is_empty()
        );
    }

    #[test]
    fn precedence_lines_give_level_and_associativity() {
        let g = Grammar::parse(
            "language x 1\nprecedence 1 left \"+\" \"-\"\nprecedence 2 same \"*\"\nprecedence 2 none \"%\"",
        )
        .unwrap();
        let p = g.precedence("-").unwrap();
        assert_eq!((p.level, p.assoc), (1, Assoc::Left));
        assert_eq!(g.precedence("%").unwrap().assoc, Assoc::None);
        assert!(g.precedence("/").is_none());
        assert!(
            Grammar::parse("language x 1\nprecedence 1 left \"+\"\nprecedence 2 left \"+\"")
                .is_err()
        );
        assert!(Grammar::parse("language x 1\nprecedence 1 wide \"+\"").is_err());
    }

    #[test]
    fn operators_map_text_to_a_kind() {
        let g = Grammar::parse(
            "language x 1\noperators equality \"==\" \"!=\"\noperators logical \"&&\"",
        )
        .unwrap();
        assert_eq!(g.operator_kind("!="), Some("equality"));
        assert_eq!(g.operator_kind("&&"), Some("logical"));
        assert_eq!(g.operator_kind("+"), Some("arithmetic"));
        assert_eq!(g.operator_kind("as"), None);
        assert!(Grammar::parse("language x 1\noperators nope \"+\"").is_err());
        assert!(
            Grammar::parse("language x 1\noperators logical \"+\"\noperators equality \"+\"")
                .is_err()
        );
    }

    #[test]
    fn a_tag_of_nodes_and_tokens_is_a_union_item() {
        let g = "language x 1\ntoken T #a\nnode N #a = u:#a*";
        assert!(Grammar::parse(g).is_ok());
    }

    #[test]
    fn rejects_bad_grammars() {
        let cases = [
            ("node A", "missing `language`"),
            ("language x 1\nnode A\nnode A", "duplicate kind"),
            ("language x 1\nnode A = b:B", "unknown kind"),
            ("language x 1\nnode A = b:#nope", "no kind carries"),
            ("language x 1\ntoken T = A", "tokens have no body"),
            ("language x 0", "language id"),
            ("language x 1\ntoken T\nnode N = a:T a:T", "duplicate label"),
            ("language x 1\nnode N = \"s\"", "expected a body item"),
            ("  node N", "continuation"),
        ];
        for (src, msg) in cases {
            let e = Grammar::parse(src).unwrap_err();
            assert!(e.message.contains(msg), "{src:?}: {e}");
        }
    }
}
