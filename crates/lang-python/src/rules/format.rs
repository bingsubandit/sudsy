//! Checks of string contents: `%` and `str.format` strings that do not fit their arguments
//! (F50x, F52x), escape sequences that do not exist (W605).
//!
//! A format string is read as CPython reads it; which right-hand sides are one value, a sequence
//! or a mapping follows ruff. Each check of a family reports the findings of one analysis that
//! carry its rule's id.

use super::Check;
use super::tree::{callee, inner, keyword, span, tokens};
use crate::format::syntax;
use crate::generated::kind as k;
use crate::text::string_value;
use core_rules::RuleContext;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, TextRange, TextSize};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

type Finding = (&'static str, String);

/// The result of the analysis of the node the checks of a family ran on last: each check of the
/// family asks for it, the first one works it out.
struct Memo<T>(RefCell<Option<(SyntaxNode, Rc<T>)>>);

impl<T> Memo<T> {
    const fn new() -> Self {
        Memo(RefCell::new(None))
    }

    fn get(&self, n: &SyntaxNode, f: impl FnOnce() -> T) -> Rc<T> {
        let mut slot = self.0.borrow_mut();
        if let Some((node, value)) = &*slot
            && node == n
        {
            return value.clone();
        }
        let value = Rc::new(f());
        *slot = Some((n.clone(), value.clone()));
        value
    }
}

thread_local! {
    static PERCENT: Memo<Vec<Finding>> = const { Memo::new() };
    static DOT: Memo<Vec<Finding>> = const { Memo::new() };
}

fn report(c: &Check, ctx: &mut RuleContext<'_>, range: TextRange, found: &[Finding]) {
    for (_, name) in found.iter().filter(|(id, _)| *id == c.meta.id) {
        c.hit(ctx, range, name);
    }
}

fn join<T: ToString>(items: impl IntoIterator<Item = T>) -> String {
    items
        .into_iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// What a `%` format string asks for.
#[derive(Default)]
struct Spec {
    /// The conversions and `*` that take the next positional value.
    positional: usize,
    /// The `%(name)s` names, once each, in order.
    keys: Vec<String>,
    star: bool,
}

enum Failure {
    Invalid(&'static str),
    Char(char),
}

fn parse(text: &str) -> Result<Spec, Failure> {
    let mut spec = Spec::default();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' || chars.next_if_eq(&'%').is_some() {
            continue;
        }
        let mut named = false;
        if chars.next_if_eq(&'(').is_some() {
            let (mut depth, mut key) = (1, String::new());
            loop {
                let Some(c) = chars.next() else {
                    return Err(Failure::Invalid("incomplete format key"));
                };
                depth += i32::from(c == '(') - i32::from(c == ')');
                if depth == 0 {
                    break;
                }
                key.push(c);
            }
            named = true;
            if !spec.keys.contains(&key) {
                spec.keys.push(key);
            }
        }
        while chars.next_if(|c| "#0- +".contains(*c)).is_some() {}
        let mut stars = 0;
        for part in 0..2 {
            if part == 1 && chars.next_if_eq(&'.').is_none() {
                break;
            }
            if chars.next_if_eq(&'*').is_some() {
                stars += 1;
            } else {
                while chars.next_if(char::is_ascii_digit).is_some() {}
            }
        }
        while chars.next_if(|c| "hlL".contains(*c)).is_some() {}
        match chars.next() {
            None => return Err(Failure::Invalid("incomplete format")),
            Some(c) if !"diouxXeEfFgGcrsa".contains(c) => return Err(Failure::Char(c)),
            Some(_) => {}
        }
        spec.star |= stars > 0;
        spec.positional += stars + usize::from(!named);
    }
    Ok(spec)
}

fn is_collection(e: &SyntaxNode) -> bool {
    matches!(
        e.kind(),
        k::EXPR_LIST
            | k::EXPR_SET
            | k::EXPR_DICT
            | k::EXPR_LIST_COMP
            | k::EXPR_SET_COMP
            | k::EXPR_DICT_COMP
    )
}

fn operator(e: &SyntaxNode) -> Option<SyntaxKind> {
    tokens(e)
        .map(|t| t.kind())
        .find(|x| !matches!(*x, k::LPAR | k::RPAR))
}

/// An expression that is one value, never a tuple (ruff's reading).
fn single(e: &SyntaxNode) -> bool {
    let e = inner(e);
    let parts: Vec<SyntaxNode> = e.children().collect();
    match e.kind() {
        k::EXPR_STRING_LITERAL
        | k::EXPR_BYTES_LITERAL
        | k::EXPR_NUMBER_LITERAL
        | k::EXPR_BOOLEAN_LITERAL
        | k::EXPR_NONE_LITERAL
        | k::EXPR_ELLIPSIS_LITERAL
        | k::EXPR_F_STRING
        | k::EXPR_GENERATOR => true,
        _ if is_collection(&e) => true,
        k::EXPR_UNARY_OP => operator(&e) == Some(k::NOT) || parts.first().is_some_and(single),
        k::EXPR_BIN_OP => match (operator(&e), &parts[..]) {
            (Some(k::AT), _) => false,
            (Some(k::PERCENT), [l, _]) if inner(l).kind() == k::EXPR_STRING_LITERAL => true,
            (Some(k::STAR | k::VBAR), [l, r])
                if is_collection(&inner(l)) || is_collection(&inner(r)) =>
            {
                false
            }
            _ => parts.iter().all(single),
        },
        k::EXPR_BOOL_OP => parts.iter().all(single),
        k::EXPR_IF => parts.first().is_some_and(single) && parts.last().is_some_and(single),
        k::EXPR_NAMED => parts.last().is_some_and(single),
        _ => false,
    }
}

enum Rhs {
    Tuple(Vec<SyntaxNode>),
    /// The string keys, when every key is one.
    Dict(Option<Vec<String>>),
    Sequence,
    Single,
    Unknown,
}

/// The `(key, value)` entries of a dict display; the key of a `**mapping` is `None`.
fn dict_items(d: &SyntaxNode) -> Vec<(Option<SyntaxNode>, SyntaxNode)> {
    let (mut items, mut key, mut colon, mut unpack) = (Vec::new(), None, false, false);
    for e in d.children_with_tokens() {
        match e {
            SyntaxElement::Node(x) if colon => {
                items.push((key.take(), x));
                colon = false;
            }
            SyntaxElement::Node(x) if unpack => {
                items.push((None, x));
                unpack = false;
            }
            SyntaxElement::Node(x) => key = Some(x),
            SyntaxElement::Token(t) if t.kind() == k::COLON => colon = true,
            SyntaxElement::Token(t) if t.kind() == k::DOUBLE_STAR => unpack = true,
            SyntaxElement::Token(_) => {}
        }
    }
    items
}

fn classify(e: &SyntaxNode) -> Rhs {
    let mut e = inner(e);
    while e.kind() == k::EXPR_NAMED {
        match e.children().last() {
            Some(v) => e = inner(&v),
            None => break,
        }
    }
    match e.kind() {
        k::EXPR_TUPLE => Rhs::Tuple(e.children().collect()),
        k::EXPR_DICT => Rhs::Dict(
            dict_items(&e)
                .into_iter()
                .map(|(key, _)| {
                    let key = inner(&key?);
                    (key.kind() == k::EXPR_STRING_LITERAL).then(|| string_value(&key))
                })
                .collect(),
        ),
        k::EXPR_DICT_COMP => Rhs::Dict(None),
        k::EXPR_LIST | k::EXPR_SET | k::EXPR_LIST_COMP | k::EXPR_SET_COMP | k::EXPR_GENERATOR => {
            Rhs::Sequence
        }
        _ if single(&e) => Rhs::Single,
        _ => Rhs::Unknown,
    }
}

fn percent_findings(b: &SyntaxNode) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let parts: Vec<SyntaxNode> = b.children().collect();
    let [left, right] = &parts[..] else {
        return out;
    };
    let left = &inner(left);
    if operator(b) != Some(k::PERCENT) || left.kind() != k::EXPR_STRING_LITERAL {
        return out;
    }
    let spec = match parse(&string_value(left)) {
        Ok(spec) => spec,
        Err(Failure::Invalid(why)) => {
            return vec![("percent-format-invalid-format", why.to_owned())];
        }
        Err(Failure::Char(c)) => {
            return vec![("percent-format-unsupported-format-character", c.to_string())];
        }
    };
    let (named, pos) = (!spec.keys.is_empty(), spec.positional);
    let count = |out: &mut Vec<Finding>, got: usize| {
        if pos != got {
            out.push((
                "percent-format-positional-count-mismatch",
                format!("{pos} placeholder(s) but {got} substitution(s)"),
            ));
        }
    };
    if named && pos > 0 {
        out.push(("percent-format-mixed-positional-and-named", String::new()));
    }
    match classify(right) {
        Rhs::Tuple(_) if named => out.push(("percent-format-expected-mapping", String::new())),
        Rhs::Tuple(t) => {
            if !t.iter().any(|e| inner(e).kind() == k::EXPR_STARRED) {
                count(&mut out, t.len());
            }
        }
        Rhs::Sequence if named => out.push(("percent-format-expected-mapping", String::new())),
        Rhs::Sequence => count(&mut out, 1),
        Rhs::Dict(keys) => {
            if named || pos == 0 {
                if let Some(keys) = keys {
                    let unused: Vec<_> = keys.iter().filter(|k| !spec.keys.contains(k)).collect();
                    let mut missing: Vec<_> =
                        spec.keys.iter().filter(|k| !keys.contains(k)).collect();
                    missing.sort();
                    if !unused.is_empty() {
                        out.push(("percent-format-extra-named-arguments", join(unused)));
                    }
                    if !missing.is_empty() {
                        out.push(("percent-format-missing-argument", join(missing)));
                    }
                }
                if !named {
                    count(&mut out, 1);
                }
            } else {
                if pos > 1 {
                    out.push(("percent-format-expected-sequence", String::new()));
                }
                count(&mut out, 1);
            }
            if spec.star {
                out.push(("percent-format-star-requires-sequence", String::new()));
            }
        }
        Rhs::Single if !named => count(&mut out, 1),
        Rhs::Unknown if !named && pos == 0 => count(&mut out, 1),
        Rhs::Single | Rhs::Unknown => {}
    }
    if out.is_empty() {
        let convertible = match classify(right) {
            Rhs::Tuple(t) => !named && !t.iter().any(|e| inner(e).kind() == k::EXPR_STARRED),
            Rhs::Dict(keys) => named && keys.is_some(),
            Rhs::Unknown => matches!(
                inner(right).kind(),
                k::EXPR_NAME | k::EXPR_ATTRIBUTE | k::EXPR_SUBSCRIPT | k::EXPR_CALL
            ),
            Rhs::Single => {
                !named && pos == 1 && !matches!(inner(right).kind(), k::EXPR_BOOL_OP | k::EXPR_IF)
            }
            Rhs::Sequence => false,
        };
        if convertible {
            out.push(("printf-string-formatting", String::new()));
        }
    }
    out
}

/// F501 to F509.
pub(super) fn percent(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let found = PERCENT.with(|m| m.get(n, || percent_findings(n)));
    report(c, ctx, span(ctx, n), &found);
}

#[derive(Default)]
struct Fields {
    /// Fields that take the next positional argument.
    auto: usize,
    /// Positional arguments named by number.
    index: BTreeSet<usize>,
    /// Keyword arguments named, once each, in order.
    keywords: Vec<String>,
}

enum Arg {
    Auto,
    Index(usize),
    Keyword(String),
}

/// The text of every field of `s` (nested ones in a spec included), or why `s` is invalid.
fn field_texts(s: &str) -> Result<Vec<&str>, &'static str> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < s.len() {
        let rest = &s[at..];
        let Some(i) = rest.find(['{', '}']) else {
            break;
        };
        let (c, after) = (rest.as_bytes()[i], &rest[i + 1..]);
        if after.as_bytes().first() == Some(&c) {
            at += i + 2;
            continue;
        }
        if c == b'}' {
            return Err("Single '}' encountered in format string");
        }
        // A field ends at the first `}` outside a nested `{...}`.
        let (mut nested, mut end) = (false, None);
        for (j, ch) in after.char_indices() {
            match ch {
                '{' if nested => return Err("Max format placeholder recursion exceeded"),
                '{' => nested = true,
                '}' if nested => nested = false,
                '}' => {
                    end = Some(j);
                    break;
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            return Err("Single '{' encountered in format string");
        };
        out.push(&after[..end]);
        at += i + 1 + end + 1;
    }
    Ok(out)
}

/// The argument a field refers to; `name` is what is inside the braces, up to any `:`.
fn argument(name: &str) -> Result<Arg, &'static str> {
    let (head, mut rest) = name.split_at(name.find(['.', '[']).unwrap_or(name.len()));
    while !rest.is_empty() {
        if let Some(attr) = rest.strip_prefix('.') {
            let end = attr.find(['.', '[']).unwrap_or(attr.len());
            if end == 0 {
                return Err("Empty attribute in format string");
            }
            rest = &attr[end..];
        } else if let Some(index) = rest.strip_prefix('[') {
            let Some(end) = index.find(']') else {
                return Err("Expected '}' before end of string");
            };
            if end == 0 {
                return Err("Empty attribute in format string");
            }
            rest = &index[end + 1..];
            if !rest.is_empty() && !rest.starts_with(['.', '[']) {
                return Err("Only '.' or '[' may follow ']' in format field specifier");
            }
        }
    }
    Ok(if head.is_empty() {
        Arg::Auto
    } else if let Ok(i) = head.parse() {
        Arg::Index(i)
    } else {
        Arg::Keyword(head.to_string())
    })
}

fn fields(s: &str) -> Result<Fields, &'static str> {
    let mut out = Fields::default();
    for text in field_texts(s)? {
        let (name, spec) = text.split_once(':').unwrap_or((text, ""));
        let (name, conversion) = name
            .split_once('!')
            .map_or((name, None), |(n, c)| (n, Some(c)));
        if conversion.is_some_and(|c| c.chars().count() != 1) {
            return Err("Unexpected error parsing format string");
        }
        for name in std::iter::once(name).chain(
            field_texts(spec)?
                .into_iter()
                .map(|t| t.split([':', '!']).next().unwrap_or("")),
        ) {
            match argument(name)? {
                Arg::Auto => out.auto += 1,
                Arg::Index(i) => {
                    out.index.insert(i);
                }
                Arg::Keyword(k) => {
                    if !out.keywords.contains(&k) {
                        out.keywords.push(k);
                    }
                }
            }
        }
    }
    Ok(out)
}

fn dot_findings(call: &SyntaxNode) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let Some(f) = callee(call).filter(|f| f.kind() == k::EXPR_ATTRIBUTE) else {
        return out;
    };
    let mut parts = f.children();
    let (Some(string), Some(member)) = (parts.next(), parts.next()) else {
        return out;
    };
    let string = inner(&string);
    if string.kind() != k::EXPR_STRING_LITERAL || crate::text::code_text(&member) != "format" {
        return out;
    }
    let f = match fields(&string_value(&string)) {
        Ok(f) => f,
        Err(why) => return vec![("string-dot-format-invalid-format", why.to_owned())],
    };
    if f.auto > 0 && !f.index.is_empty() {
        out.push(("string-dot-format-mixing-automatic", String::new()));
    }
    let (args, keywords) = match super::tree::call_arguments(call) {
        Some(a) => super::tree::arguments(&a),
        None => (Vec::new(), Vec::new()),
    };
    let starred = |a: &SyntaxNode| a.kind() == k::EXPR_STARRED;
    let used = |i: usize| i < f.auto || f.index.contains(&i);
    let unused: Vec<usize> = (0..args.len())
        .filter(|&i| !starred(&args[i]) && !used(i))
        .collect();
    if !unused.is_empty() {
        out.push(("string-dot-format-extra-positional-arguments", join(unused)));
    }
    let named: Vec<String> = keywords.iter().filter_map(|kw| keyword(kw).0).collect();
    let splat = named.len() < keywords.len();
    if !splat {
        let extra: Vec<_> = named.iter().filter(|n| !f.keywords.contains(n)).collect();
        if !extra.is_empty() {
            out.push(("string-dot-format-extra-named-arguments", join(extra)));
        }
    }
    if !splat && !args.iter().any(starred) {
        let positions = (0..f.auto)
            .chain(f.index.iter().copied())
            .filter(|&i| i >= args.len())
            .collect::<BTreeSet<_>>();
        let keywords = f.keywords.iter().filter(|k| !named.contains(k));
        let missing = join(
            positions
                .into_iter()
                .map(|i| i.to_string())
                .chain(keywords.cloned()),
        );
        if !missing.is_empty() {
            out.push(("string-dot-format-missing-arguments", missing));
        }
    }
    // The arguments move into the string: they must not hold its quote character.
    let quote = string
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::STRING)
        .and_then(|t| t.text().chars().find(|c| matches!(c, '\'' | '"')));
    let clash = |a: &SyntaxNode| quote.is_none_or(|q| crate::text::code_text(a).contains(q));
    if out.is_empty()
        && !splat
        && !args.iter().any(starred)
        && !args.iter().any(clash)
        && !keywords.iter().any(clash)
    {
        out.push(("f-string", String::new()));
    }
    out
}

/// F521 to F525.
pub(super) fn dot_format(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let found = DOT.with(|m| m.get(n, || dot_findings(n)));
    report(c, ctx, span(ctx, n), &found);
}

/// The text before the first quote: the prefix letters of a string.
fn is_raw(text: &str) -> bool {
    text.chars()
        .take_while(|c| !matches!(c, '\'' | '"'))
        .any(|c| matches!(c, 'r' | 'R'))
}

/// W605: a backslash in a string that is not raw and starts no escape sequence.
pub(super) fn invalid_escape(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let interpolated = matches!(n.kind(), k::F_STRING | k::T_STRING);
    let raw = |t: &core_syntax::SyntaxToken| {
        if interpolated {
            n.first_token().is_none_or(|s| is_raw(s.text()))
        } else {
            is_raw(t.text())
        }
    };
    let pieces = n
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| {
            matches!(
                t.kind(),
                k::STRING | k::F_STRING_MIDDLE | k::T_STRING_MIDDLE
            ) && t
                .parent_ancestors()
                .find(|a| matches!(a.kind(), k::F_STRING | k::T_STRING))
                .is_none_or(|a| a == *n)
        });
    for t in pieces.filter(|t| !raw(t)) {
        // The backslash ending the piece of an f-string escapes the replacement field's brace.
        let text = if t.kind() == k::STRING {
            t.text().to_owned()
        } else {
            format!("{}{{", t.text())
        };
        let start = t.text_range().start();
        syntax().unknown_escapes(&text, '"', |i, next| {
            let at = start + TextSize::try_from(i).unwrap_or_default();
            let len = TextSize::try_from(1 + next.len_utf8()).unwrap_or_default();
            c.hit(ctx, TextRange::at(at, len), &format!("\\{next}"));
        });
    }
}
