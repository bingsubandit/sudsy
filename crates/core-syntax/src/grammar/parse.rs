use super::*;
use sha2::{Digest, Sha256};
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
    Str(String),
    Tag(&'a str),
    Int(u32),
    Sym(char),
}

struct Decl<'a> {
    line: usize,
    lexemes: Vec<Lexeme<'a>>,
}

/// Names and tags are slices of the source (ASCII); only strings are copied (they unescape).
fn lex_line<'a>(
    line_no: usize,
    text: &'a str,
    out: &mut Vec<Lexeme<'a>>,
) -> Result<(), GrammarError> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'/' if bytes.get(i + 1) == Some(&b'/') => break,
            b'"' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match bytes.get(i) {
                        None => return err(line_no, "unterminated string"),
                        Some(b'"') => break,
                        Some(b'\\') => {
                            i += 1;
                            match bytes.get(i) {
                                Some(&e @ (b'"' | b'\\')) => s.push(e as char),
                                _ => return err(line_no, "bad escape (use \\\" or \\\\)"),
                            }
                            i += 1;
                        }
                        Some(_) => {
                            let ch = text[i..].chars().next().unwrap_or_default();
                            s.push(ch);
                            i += ch.len_utf8();
                        }
                    }
                }
                i += 1;
                out.push(Lexeme::Str(s));
            }
            b'#' => {
                let start = i + 1;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                    i += 1;
                }
                if i == start {
                    return err(line_no, "empty tag");
                }
                out.push(Lexeme::Tag(&text[start..i]));
            }
            b'0'..=b'9' => {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                match text[start..i].parse() {
                    Ok(n) => out.push(Lexeme::Int(n)),
                    Err(_) => return err(line_no, "number too large"),
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-')
                {
                    i += 1;
                }
                out.push(Lexeme::Ident(&text[start..i]));
            }
            b'=' | b':' | b'?' | b'*' => {
                out.push(Lexeme::Sym(c as char));
                i += 1;
            }
            _ => {
                let ch = text[i..].chars().next().unwrap_or_default();
                if !ch.is_whitespace() {
                    return err(line_no, format!("unexpected character {ch:?}"));
                }
                i += ch.len_utf8();
            }
        }
    }
    Ok(())
}

fn split_decls(source: &str) -> Result<Vec<Decl<'_>>, GrammarError> {
    let mut decls: Vec<Decl<'_>> = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line_no = i + 1;
        let mut lexemes = Vec::new();
        lex_line(line_no, raw, &mut lexemes)?;
        if lexemes.is_empty() {
            continue;
        }
        if raw.starts_with(char::is_whitespace) {
            match decls.last_mut() {
                Some(d) => d.lexemes.append(&mut lexemes),
                None => return err(line_no, "continuation line before any declaration"),
            }
        } else {
            decls.push(Decl {
                line: line_no,
                lexemes,
            });
        }
    }
    Ok(decls)
}

fn content_hash(decls: &[Decl]) -> String {
    let mut h = Sha256::new();
    for d in decls {
        for l in &d.lexemes {
            let (tag, text) = match l {
                Lexeme::Ident(s) => (b'i', s.to_string()),
                Lexeme::Str(s) => (b's', s.clone()),
                Lexeme::Tag(s) => (b't', s.to_string()),
                Lexeme::Int(n) => (b'n', n.to_string()),
                Lexeme::Sym(c) => (b'y', c.to_string()),
            };
            h.update([tag]);
            h.update(text.as_bytes());
            h.update([0]);
        }
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
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
    fn string(&mut self, what: &str) -> Result<String, GrammarError> {
        match self.next() {
            Some(Lexeme::Str(s)) => Ok(s.clone()),
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
    text: Option<String>,
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
    let decls = split_decls(source)?;
    let hash = content_hash(&decls);

    let mut language: Option<(String, LanguageId, String)> = None;
    let mut comments = CommentSyntax::default();
    let mut suppressions = Vec::new();
    let mut requires_roles = false;
    let mut raw: Vec<RawKind<'_>> = Vec::new();

    for d in &decls {
        let mut c = Cursor {
            line: d.line,
            rest: &d.lexemes,
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
                    _ => name.to_string(),
                };
                if language
                    .replace((name.to_string(), LanguageId::new(id), title))
                    .is_some()
                {
                    return err(d.line, "duplicate language declaration");
                }
            }
            "comments" => {
                while let Some(l) = c.next() {
                    match l {
                        Lexeme::Ident("line") => {
                            comments.line = Some(c.string("line comment prefix")?)
                        }
                        Lexeme::Ident("block") => {
                            comments.block = Some((
                                c.string("block comment start")?,
                                c.string("block comment end")?,
                            ))
                        }
                        _ => return err(d.line, "expected `line` or `block`"),
                    }
                }
            }
            "roles" => requires_roles = true,
            "suppress" => {
                let role = c.ident("suppression role")?;
                let marker = c.string("suppression marker")?;
                suppressions.push(Suppression {
                    role: role.to_string(),
                    marker,
                });
            }
            "token" | "node" => {
                let is_token = keyword == "token";
                let name = c.ident("a kind name")?;
                let mut text = None;
                if is_token && let Some(Lexeme::Str(s)) = c.peek() {
                    text = Some(s.clone());
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
    resolve(
        language,
        language_id,
        title,
        comments,
        suppressions,
        requires_roles,
        raw,
        hash,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    language: String,
    language_id: LanguageId,
    title: String,
    comments: CommentSyntax,
    suppressions: Vec<Suppression>,
    requires_roles: bool,
    raw: Vec<RawKind<'_>>,
    hash: String,
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
    let mut tag_class: HashMap<String, Option<bool>> = HashMap::new();
    for r in &raw {
        for t in &r.tags {
            let class = tag_class.entry(t.to_string()).or_insert(Some(r.is_token));
            if *class != Some(r.is_token) {
                *class = None;
            }
        }
    }

    // Pass 2: resolve bodies.
    let mut nodes = Vec::new();
    let mut tokens = Vec::new();
    for (r, kind) in raw.into_iter().zip(kinds) {
        let mut body = Vec::new();
        for it in r.body {
            let target = match it.target {
                RawTarget::Name(n) => match by_name.get(n) {
                    Some(k) => Target::Kind(*k),
                    None => return err(r.line, format!("unknown kind `{n}`")),
                },
                RawTarget::Tag(t) => match tag_class.get(t) {
                    // A tag of tokens or of nodes, or a union of both (found by `Grammar::slot`).
                    Some(_) => Target::Tag(t.to_string()),
                    None => return err(r.line, format!("no kind carries tag #{t}")),
                },
            };
            // Accessor names are unique; several items may fill one role field.
            body.push(Item {
                label: it.label.map(str::to_string),
                field: it.field.map(str::to_string),
                target,
                cardinality: it.cardinality,
            });
        }
        let mut labels = HashSet::new();
        for l in body.iter().filter_map(|i| i.label.as_deref()) {
            if !labels.insert(l) {
                return err(r.line, format!("duplicate label `{l}`"));
            }
        }
        let def = KindDef {
            name: r.name.to_string(),
            kind,
            text: r.text,
            tags: r.tags.into_iter().map(str::to_string).collect(),
            body,
        };
        if r.is_token {
            tokens.push(def)
        } else {
            nodes.push(def)
        }
    }

    let nodes_len = nodes.len();
    let mut grammar = Grammar {
        language,
        language_id,
        title,
        comments,
        suppressions,
        requires_roles,
        token_flags: super::hot_flags(&tokens),
        node_flags: super::hot_flags(&nodes),
        nodes,
        tokens,
        by_name,
        hash,
        bodies: std::iter::repeat_with(Default::default)
            .take(nodes_len)
            .collect(),
        skip: Vec::new(),
        tag_nodes: HashMap::new(),
    };
    grammar.resolve();
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
