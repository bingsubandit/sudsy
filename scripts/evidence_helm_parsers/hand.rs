//! Hand-written scan of the template actions (docs/ai/design/helm.md): the rules of crates/core-syntax/src/template_actions.rs
//! for Go templates, with no tree. An action runs from `{{` to the first `}}` outside a quoted string; a comment (`{{/* */}}`,
//! trims allowed) runs to its `*/` and quotes in it do not count; a block word (`if`, `range`, `with`, `define`, `block`)
//! opens, `else`/`elif` continues and `end` closes. Error: an action that does not close, an `end` or `else` with no open
//! block, or a block open at the end. No tree, so no share.
use evidence_helm_parsers::{Check, load, probes, run};
use std::path::PathBuf;

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The index of the `}}` that closes the action whose body starts at `s`.
fn action_end(b: &[u8], s: usize) -> Option<usize> {
    let body = b[s..].iter().position(|c| !matches!(c, b'-' | b' ' | b'\t' | b'\r' | b'\n'))?;
    if b[s + body..].starts_with(b"/*") {
        let close = find(&b[s..], b"*/")? + s;
        return find(&b[close + 2..], b"}}").map(|e| close + 2 + e);
    }
    let mut j = s;
    let mut quote = 0u8;
    while j < b.len() {
        let c = b[j];
        if quote != 0 {
            if c == b'\\' && quote == b'"' {
                j += 2;
                continue;
            }
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' || c == b'`' {
            quote = c;
        } else if c == b'}' && b.get(j + 1) == Some(&b'}') {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn has_error(text: &str) -> bool {
    let b = text.as_bytes();
    let mut i = 0;
    let mut depth = 0usize;
    while let Some(off) = find(&b[i..], b"{{") {
        let s = i + off + 2;
        let Some(e) = action_end(b, s) else { return true };
        let word: String = text[s..e].trim().trim_start_matches('-').trim_start().chars().take_while(|c| c.is_alphanumeric()).collect();
        match word.as_str() {
            "if" | "range" | "with" | "define" | "block" => depth += 1,
            "end" => {
                if depth == 0 {
                    return true;
                }
                depth -= 1;
            }
            "else" | "elif" if depth == 0 => return true,
            _ => {}
        }
        i = e + 2;
    }
    depth != 0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let items = load(&PathBuf::from(&args[1]));
    let probe_list = probes(&PathBuf::from(&args[2]));
    run("hand-written action scan (template_actions rules, no tree)", &items, &probe_list, |text| Check {
        error: has_error(text),
        share: f64::NAN,
    });
}
