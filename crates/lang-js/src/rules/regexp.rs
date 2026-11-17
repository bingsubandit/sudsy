//! Rules about what a regular expression matches: control characters and combined characters in
//! classes. The pattern is a regex literal or the constant string given to `RegExp`.

use super::scope::ref_at;
use super::{Check, code_text, code_token, span};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use core_rules::RuleContext;
use core_semantics::scopes::Flags;
use core_syntax::SyntaxNode;
use std::sync::OnceLock;

/// The value of a string literal's source (`'a\\u00e9'`).
fn cook(source: &str) -> String {
    let inner = &source[1..source.len().saturating_sub(1).max(1)];
    let chars: Vec<char> = inner.chars().collect();
    let (mut out, mut i) = (String::new(), 0);
    let hex = |from: usize, len: usize| -> Option<(char, usize)> {
        let digits: String = chars.get(from..from + len)?.iter().collect();
        Some((char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?, len))
    };
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(&e) = chars.get(i) else { break };
        i += 1;
        match e {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '0' if !chars.get(i).is_some_and(char::is_ascii_digit) => out.push('\0'),
            'x' => match hex(i, 2) {
                Some((ch, n)) => (out.push(ch), i += n).1,
                None => out.push('x'),
            },
            'u' if chars.get(i) == Some(&'{') => {
                let end = chars[i..].iter().position(|c| *c == '}').map_or(i, |p| i + p);
                let digits: String = chars[i + 1..end].iter().collect();
                if let Some(ch) = u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
                i = end + 1;
            }
            'u' => match hex(i, 4) {
                Some((ch, n)) => (out.push(ch), i += n).1,
                None => out.push('u'),
            },
            '\n' => {}
            other => out.push(other),
        }
    }
    out
}

/// The string an expression always evaluates to: literals, `+`, and the names of variables that
/// are written once, with an initializer that is one itself.
fn constant(cx: Option<&Cx>, n: &SyntaxNode, g: &core_syntax::Grammar, depth: u8) -> Option<String> {
    if depth > 16 {
        return None;
    }
    match n.kind() {
        k::STRING_LITERAL => Some(cook(&code_text(n, g))),
        k::TEMPLATE_LITERAL => {
            let text = code_text(n, g);
            (!text.contains("${")).then(|| text[1..text.len() - 1].to_string())
        }
        k::PARENTHESIZED_EXPRESSION => constant(cx, &n.children().next()?, g, depth + 1),
        k::BINARY_EXPRESSION => {
            let mut parts = n.children();
            let (left, right) = (parts.next()?, parts.next()?);
            let plus = n.children_with_tokens().filter_map(|e| e.into_token()).any(|t| t.text() == "+");
            plus.then_some(())?;
            Some(constant(cx, &left, g, depth + 1)? + &constant(cx, &right, g, depth + 1)?)
        }
        k::IDENTIFIER_REFERENCE => {
            let cx = cx?;
            let r = ref_at(cx, cx.range(n).start())?;
            let sym = cx.s.symbol(r.symbol?);
            let [decl] = sym.decls.as_slice() else { return None };
            let once = decl.flags.contains(Flags::CONST)
                || (decl.flags.contains(Flags::VAR) && !cx.s.refs_of(r.symbol?).any(|w| w.is_write()));
            if !once || decl.node.kind() != k::VARIABLE_DECLARATOR {
                return None;
            }
            constant(Some(cx), &cx.slot(&decl.node, "init")?, g, depth + 1)
        }
        _ => None,
    }
}

/// The pattern of a regex literal, or of `new RegExp(pattern)` / `RegExp(pattern)`.
fn pattern(ctx: &RuleContext<'_>, n: &SyntaxNode) -> Option<String> {
    let g = ctx.file.grammar;
    if n.kind() == k::REG_EXP_LITERAL {
        let text = code_token(n, g)?.text().to_string();
        return Some(text[1..text.rfind('/')?.max(1)].to_string());
    }
    let mut parts = n.children();
    let callee = parts.next().filter(|f| f.kind() == k::IDENTIFIER_REFERENCE && code_text(f, g) == "RegExp")?;
    let _ = callee;
    let arg = parts.next()?;
    let cx = model(ctx.file);
    constant(cx.as_deref(), &arg, g, 0)
}

fn hex(chars: &[char], from: usize, len: usize) -> Option<u32> {
    let digits: String = chars.get(from..from + len)?.iter().collect();
    u32::from_str_radix(&digits, 16).ok()
}

/// `\x1f`, `\u001f`, `\u{1f}` at `i` (just after the backslash): the code point and the length
/// of the escape after the backslash.
fn code_escape(chars: &[char], i: usize) -> Option<(u32, usize)> {
    match chars.get(i)? {
        'x' => Some((hex(chars, i + 1, 2)?, 3)),
        'u' if chars.get(i + 1) == Some(&'{') => {
            let end = chars[i..].iter().position(|c| *c == '}')? + i;
            let digits: String = chars[i + 2..end].iter().collect();
            Some((u32::from_str_radix(&digits, 16).ok()?, end - i + 1))
        }
        'u' => Some((hex(chars, i + 1, 4)?, 5)),
        _ => None,
    }
}

/// The control characters (below 0x20) a pattern matches by their code, as `\x1f`.
fn controls(pattern: &str) -> Vec<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let (mut found, mut i) = (Vec::new(), 0);
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        let cp = if c == '\\' {
            match code_escape(&chars, i) {
                Some((cp, len)) => {
                    i += len;
                    cp
                }
                None => {
                    i += 1;
                    continue;
                }
            }
        } else {
            c as u32
        };
        if cp < 0x20 {
            found.push(format!("\\x{cp:02x}"));
        }
    }
    found
}

/// `no-control-regex`
pub(super) fn control_regex(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(pattern) = pattern(ctx, n) else { return };
    let found = controls(&pattern);
    if !found.is_empty() {
        c.hit(ctx, span(ctx, n), &found.join(", "));
    }
}

/// The code points of `\p{M}`.
fn marks() -> &'static [(u32, u32)] {
    static MARKS: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    MARKS.get_or_init(|| {
        use regex_syntax::hir::{Class, HirKind};
        match regex_syntax::parse(r"\p{M}").map(|h| h.into_kind()) {
            Ok(HirKind::Class(Class::Unicode(u))) => {
                u.iter().map(|r| (r.start() as u32, r.end() as u32)).collect()
            }
            _ => Vec::new(),
        }
    })
}

fn combining(cp: u32) -> bool {
    let table = marks();
    table
        .binary_search_by(|&(lo, hi)| {
            if cp < lo {
                std::cmp::Ordering::Greater
            } else if cp > hi {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Does a character class of the pattern have a combining mark after a character that is not one
/// (the sequences ESLint reads: a range ends the one before it and its last character starts the
/// next)?
fn combined_in_class(pattern: &str) -> bool {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '[' => {
                i += 1;
                if chars.get(i) == Some(&'^') {
                    i += 1;
                }
                let mut seq: Vec<u32> = Vec::new();
                let atom = |i: &mut usize| -> Option<Option<u32>> {
                    let c = *chars.get(*i)?;
                    *i += 1;
                    if c != '\\' {
                        return Some(Some(c as u32));
                    }
                    let e = *chars.get(*i)?;
                    if let Some((cp, len)) = code_escape(&chars, *i) {
                        *i += len;
                        return Some(Some(cp));
                    }
                    *i += 1;
                    Some(match e {
                        'd' | 'D' | 'w' | 'W' | 's' | 'S' => None,
                        'p' | 'P' => {
                            if chars.get(*i) == Some(&'{') {
                                while chars.get(*i).is_some_and(|c| *c != '}') {
                                    *i += 1;
                                }
                                *i += 1;
                            }
                            None
                        }
                        'b' => Some(8),
                        other => Some(other as u32),
                    })
                };
                while i < chars.len() && chars[i] != ']' {
                    let Some(first) = atom(&mut i) else { break };
                    let range = chars.get(i) == Some(&'-') && chars.get(i + 1).is_some_and(|c| *c != ']');
                    match first {
                        None => seq.clear(),
                        Some(cp) if range => {
                            i += 1;
                            seq.push(cp);
                            if sequence_combined(&seq) {
                                return true;
                            }
                            seq = atom(&mut i).flatten().into_iter().collect();
                        }
                        Some(cp) => seq.push(cp),
                    }
                }
                if sequence_combined(&seq) {
                    return true;
                }
            }
            _ => i += 1,
        }
    }
    false
}

fn sequence_combined(seq: &[u32]) -> bool {
    seq.windows(2).any(|w| combining(w[1]) && !combining(w[0]))
}

/// `no-misleading-character-class`: a combining mark after another character in a class.
pub(super) fn misleading_class(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if pattern(ctx, n).is_some_and(|p| combined_in_class(&p)) {
        c.hit(ctx, span(ctx, n), "");
    }
}
