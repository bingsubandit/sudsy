//! The reads written inside a token's text (`$name`, `${name:-x}`, the bare names of `$(( ))`): the
//! binder sees nodes and name tokens, so a language that writes reads in words lists the token
//! kinds (`ScopeLang::expansions`, `ScopeLang::arithmetic`) and they are added to the model.
//!
//! concept: scope-rules
//! what: Unused, undefined, shadowed, redeclared and use-before-define checks from a language scope table.
//! rules: unused-variable, unused-parameter, unused-import, undefined-variable, shadowed-variable, redeclared-variable, use-before-define
//! reads: family:scope

use super::ScopeLang;
use crate::FileContext;
use core_semantics::scopes::{Access, Model, ScopeId};
use core_syntax::{SyntaxKind, TextRange, TextSize};

fn word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The identifier that starts at `at`, if one does.
fn identifier(text: &str, at: usize) -> Option<&str> {
    let b = text.as_bytes();
    if !b
        .get(at)
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
    {
        return None;
    }
    let len = b[at..].iter().take_while(|c| word(**c)).count();
    Some(&text[at..at + len])
}

/// Does the text after a name in `${name...}` give it a default or assign it (`:-`, `=`, `+`, `?`)?
fn guarded(rest: &str) -> bool {
    let rest = rest.strip_prefix(':').unwrap_or(rest);
    rest.starts_with(['-', '=', '+', '?'])
}

/// The innermost scope that holds the range.
fn scope_of(model: &Model, range: TextRange) -> ScopeId {
    (0..model.scopes.len())
        .rev()
        .find(|i| model.scopes[*i].node.text_range().contains_range(range))
        .map_or(ScopeId(0), |i| ScopeId(i as u32))
}

pub(super) fn add_reads(model: &mut Model, file: &FileContext<'_>, lang: &ScopeLang) {
    let kinds = |names: &[&str]| -> Vec<SyntaxKind> {
        (names.iter())
            .filter_map(|n| file.grammar.kind_by_name(n))
            .collect()
    };
    let (words, arith) = (kinds(lang.expansions), kinds(lang.arithmetic));
    let mut last = None;
    for (at, _) in file.text.match_indices('$') {
        let Some(t) = file
            .root
            .token_at_offset(TextSize::new(at as u32))
            .right_biased()
        else {
            continue;
        };
        let start = t.text_range().start();
        let is_arith = arith.contains(&t.kind());
        if last.replace(start) == Some(start) || !(is_arith || words.contains(&t.kind())) {
            continue;
        }
        let Some(parent) = t.parent() else { continue };
        let text = t.text();
        // Single quotes keep the text from expanding.
        if text.len() > 1 && text.starts_with('\'') && text.ends_with('\'') {
            continue;
        }
        let mut found: Vec<(usize, &str, bool)> = Vec::new();
        for (i, _) in text.match_indices('$') {
            if text[..i].ends_with('\\') {
                continue;
            }
            let (mut j, braced) = match text.as_bytes().get(i + 1) {
                Some(b'{') => (i + 2, true),
                _ => (i + 1, false),
            };
            if braced && text[j..].starts_with(['#', '!']) {
                j += 1;
            }
            if let Some(name) = identifier(text, j) {
                let probe = braced && guarded(&text[j + name.len()..]);
                found.push((j, name, probe));
            }
        }
        if is_arith {
            for (i, _) in text.match_indices(|c: char| c.is_ascii_alphabetic() || c == '_') {
                let before = text[..i].bytes().next_back();
                if before.is_some_and(|c| word(c) || c == b'$') {
                    continue;
                }
                if let Some(name) = identifier(text, i) {
                    found.push((i, name, false));
                }
            }
        }
        for (i, name, probe) in found {
            let range = TextRange::at(
                start + TextSize::new(i as u32),
                TextSize::new(name.len() as u32),
            );
            let access = if probe {
                Access::READ.union(Access::PROBE)
            } else {
                Access::READ
            };
            let scope = scope_of(model, range);
            model.add_reference(
                name,
                range,
                (parent.text_range(), parent.kind()),
                access,
                scope,
            );
        }
    }
}
