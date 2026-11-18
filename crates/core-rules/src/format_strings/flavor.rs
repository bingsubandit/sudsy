//! The placeholder syntaxes of format strings, read as the language's own library reads them:
//! what a string asks of its arguments, or why it is invalid.

use std::collections::BTreeSet;

/// What a format string asks of its arguments.
#[derive(Default)]
pub(super) struct Spec {
    /// The placeholders (and `*` widths) that take the next positional argument.
    pub auto: usize,
    /// Positional arguments named by number.
    pub index: BTreeSet<usize>,
    /// The names of the arguments, once each, in order.
    pub names: Vec<String>,
    /// A `*` width or precision.
    pub star: bool,
}

pub(super) enum Failure {
    Invalid(&'static str),
    Char(char),
}

/// A `format` row's flavor.
#[derive(Clone, Copy)]
pub(super) enum Flavor {
    /// `%(name)s`, `%5.2f`, `%*d`.
    Percent,
    /// `{}`, `{0}`, `{name.attr[0]!r:>{width}}`.
    Brace,
}

impl Flavor {
    pub fn named(name: &str) -> Option<Flavor> {
        match name {
            "percent" => Some(Flavor::Percent),
            "brace" => Some(Flavor::Brace),
            _ => None,
        }
    }

    pub fn parse(self, text: &str) -> Result<Spec, Failure> {
        match self {
            Flavor::Percent => percent(text),
            Flavor::Brace => brace(text).map_err(Failure::Invalid),
        }
    }
}

fn percent(text: &str) -> Result<Spec, Failure> {
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
            if !spec.names.contains(&key) {
                spec.names.push(key);
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
        spec.auto += stars + usize::from(!named);
    }
    Ok(spec)
}

enum Arg {
    Auto,
    Index(usize),
    Name(String),
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
        Arg::Name(head.to_string())
    })
}

fn brace(s: &str) -> Result<Spec, &'static str> {
    let mut out = Spec::default();
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
                Arg::Name(k) => {
                    if !out.names.contains(&k) {
                        out.names.push(k);
                    }
                }
            }
        }
    }
    Ok(out)
}
