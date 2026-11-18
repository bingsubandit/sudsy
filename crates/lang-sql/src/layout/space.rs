//! Spacing between blocks on one line (sqlfluff's LT01, LT06): the `[space]` rows of
//! `format.rules` say for each pair of neighbouring tokens whether one blank, none or the source's
//! stands between them, the `[joined]` rows whether a line break between them goes. The rows are
//! read once per file; a round of the layout only applies them to the current gaps.
//!
//! concept: token-spacing-rows

use super::seq::{Seq, is_comment};
use crate::generated::kind as k;

/// What the rows say about the gap before a block.
#[derive(Clone, Copy, Default)]
pub struct Gap {
    /// One blank (`Some(true)`), none (`Some(false)`) or as written (`None`).
    pub want: Option<bool>,
    /// A line break in the gap goes.
    pub join: bool,
}

/// The gap before each block; the first has none.
pub fn gaps(seq: &Seq) -> Vec<Gap> {
    let mut gaps = vec![Gap::default()];
    gaps.extend((1..seq.blocks.len()).map(|i| {
        let (a, b) = (&seq.blocks[i - 1].token, &seq.blocks[i].token);
        Gap {
            want: seq.rules.want(a, b),
            join: seq.rules.joined(a, b),
        }
    }));
    gaps
}

/// Would the text of the two blocks fuse into other tokens with nothing between them?
fn fuses(prev: &str, next: &str, prev_is_sign: bool) -> bool {
    let (a, b) = (prev.chars().next_back(), next.chars().next());
    (a == Some('-') && b == Some('-'))
        || (prev_is_sign && b.is_some_and(|c| "+-*/<>=~!@#%^&|`?".contains(c)))
}

/// Sets the gap of every point that sits inside a line; returns whether anything changed.
pub fn respace(seq: &mut Seq) -> bool {
    (1..seq.blocks.len()).fold(false, |changed, i| respace_point(seq, i, false) | changed)
}

/// Sets the gap before block `i`; `strip` removes a line break whatever the rows say.
pub fn respace_point(seq: &mut Seq, i: usize, strip: bool) -> bool {
    let Gap { want, join } = seq.gaps[i];
    // A `:` that starts a line is a sqlcmd command (`:r file`): the blank after it stays.
    let want = if seq.blocks[i - 1].kind == k::COLON && (i == 1 || seq.points[i - 1].newlines > 0)
    {
        None
    } else {
        want
    };
    let comment = is_comment(seq.blocks[i - 1].kind) || is_comment(seq.blocks[i].kind);
    let touch = want == Some(false);
    let any = want.is_none();
    let fuse = (touch || any)
        && fuses(
            &seq.blocks[i - 1].text,
            &seq.blocks[i].text,
            seq.blocks[i - 1].kind == k::SIGN,
        );
    let strip = (strip || join) && !comment && !fuse;
    // The characters of one operator (`+=`, `||`) the parser lexed apart stay together.
    let op = |j: usize| {
        let b = &seq.blocks[j];
        (b.kind == k::OPERATOR || b.kind == k::BINARY_OPERATOR)
            && !b.text.chars().any(|c| c.is_alphanumeric() || c == '_')
    };
    let joined = op(i - 1) && op(i);
    let p = &mut seq.points[i];
    if joined && p.newlines == 0 && p.ws.is_empty() {
        return false;
    }
    if seq.blocks[i].kind == k::EOF {
        let changed = p.newlines == 0 && !p.ws.is_empty();
        if changed {
            p.set_ws("");
        }
        return changed;
    }
    if p.newlines > 0 && !strip {
        return false;
    }
    let had_break = p.newlines > 0;
    p.newlines = 0;
    if had_break {
        p.set_ws("");
    }
    let want = if any || (touch && fuse) {
        None
    } else {
        Some(if touch { "" } else { " " })
    };
    match want {
        Some(w) if p.ws != w => {
            p.set_ws(w);
            true
        }
        _ => had_break,
    }
}

#[cfg(test)]
mod diff {
    use super::*;
    use crate::layout::space_old::old_gap;

    #[test]
    #[ignore]
    fn rows_agree_with_the_old_wishes() {
        let rows = super::super::ROWS.get().space();
        let dir = std::path::Path::new("/scratch/slots/sudsy/5/parity/sqlfluff-work");
        let mut seen: std::collections::BTreeMap<String, (usize, String)> = Default::default();
        let mut total = 0;
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        names.sort();
        for path in names {
            let Some(name) = path.file_name().and_then(|n| n.to_str()).map(String::from) else { continue };
            if !name.ends_with(".sql") { continue }
            let dialect = if name.starts_with("mysql") { "mysql" } else if name.starts_with("tsql") { "tsql" } else if name.starts_with("postgres") { "postgres" } else { "ansi" };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let parsed = crate::parse(&text, dialect);
            if !parsed.errors.is_empty() { continue }
            let grammar = provider_api::LanguageProvider::grammar(&crate::provider()).clone();
            let seq = Seq::build(&parsed.tree, &grammar, rows);
            let g = gaps(&seq);
            for i in 1..seq.blocks.len() {
                total += 1;
                let (o, ostrip) = old_gap(&seq, i);
                let nw = match g[i].want { Some(true) => 1, Some(false) => 0, None => 2 };
                let nstrip = g[i].join && !is_comment(seq.blocks[i - 1].kind) && !is_comment(seq.blocks[i].kind);
                if o != nw || ostrip != nstrip {
                    let a = &seq.blocks[i - 1];
                    let b = &seq.blocks[i];
                    let pk = |n: u32| if n == super::super::seq::NONE { "-".to_string() } else { grammar.name(seq.nodes[n as usize].kind).to_string() };
                    let key = format!(
                        "old {o}/{ostrip} new {nw}/{nstrip}  {}({}) / {}({})  common {}",
                        grammar.name(a.kind), pk(a.node), grammar.name(b.kind), pk(b.node),
                        pk(seq.common_node(i - 1, i))
                    );
                    let name = format!("{name} `{}` `{}`", a.text, b.text);
                    let e = seen.entry(key).or_insert((0, name.clone()));
                    e.0 += 1;
                }
            }
        }
        let mut v: Vec<_> = seen.into_iter().collect();
        v.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        for (k, (n, f)) in v.iter().take(150) {
            println!("{n:5} {k}   e.g. {f}");
        }
        println!("pairs {total}, distinct mismatches {}", v.len());
    }
}
