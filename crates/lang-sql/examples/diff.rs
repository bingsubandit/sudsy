//! Temporary: differential run of the hand-written parser against sqruff's tree.
//! `diff <dialect> <dir-or-file>... [-v N] [-k kind]`
use core_syntax::{Grammar, SyntaxElement};
use provider_api::LanguageProvider;
use std::collections::BTreeMap;

#[derive(PartialEq, Clone, Debug)]
enum Item {
    Tok(String, String, usize, usize),
    Meta(&'static str),
}

fn walk(
    e: SyntaxElement,
    g: &Grammar,
    items: &mut Vec<Item>,
    nodes: &mut Vec<(String, usize, usize)>,
) {
    let toks = |items: &Vec<Item>| items.iter().filter(|i| matches!(i, Item::Tok(..))).count();
    match e {
        SyntaxElement::Node(n) => {
            let start = toks(items);
            for c in n.children_with_tokens() {
                walk(c, g, items, nodes);
            }
            nodes.push((g.name(n.kind()).to_string(), start, toks(items)));
        }
        SyntaxElement::Token(t) => {
            if g.is_trivia(t.kind()) {
                return;
            }
            let name = g.name(t.kind());
            let item = match name {
                "Indent" => Item::Meta("I"),
                "ImplicitIndent" => Item::Meta("i"),
                "Dedent" => Item::Meta("D"),
                "Eof" => return,
                _ => { let r = t.text_range(); Item::Tok(name.to_string(), t.text().to_string(), u32::from(r.start()) as usize, u32::from(r.end()) as usize) }
            };
            items.push(item);
        }
    }
}

fn merge_ops(items: Vec<Item>) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    for it in items {
        if let (Item::Tok(k, t, s, e), Some(Item::Tok(pk, pt, ps, pe))) = (&it, out.last_mut()) {
            if k == "Operator" && pk == "Operator" && pe == s {
                *pt = format!("{pt}{t}");
                *pe = *e;
                let _ = ps;
                continue;
            }
        }
        out.push(it);
    }
    out
}

fn show(items: &[Item], from: usize, to: usize) -> String {
    items[from.min(items.len())..to.min(items.len())]
        .iter()
        .map(|i| match i {
            Item::Tok(k, t, ..) => format!("{t}/{k}"),
            Item::Meta(m) => format!("<{m}>"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn files(path: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
        let mut v: Vec<_> = std::fs::read_dir(path).unwrap().map(|e| e.unwrap().path()).collect();
        v.sort();
        for p in v {
            files(&p, out);
        }
    } else if path.extension().is_some_and(|e| e == "sql") {
        out.push(path.to_path_buf());
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dialect = a[1].clone();
    let mut verbose = 5usize;
    let mut only_node: Option<String> = None;
    let mut paths = Vec::new();
    let mut i = 2;
    while i < a.len() {
        match a[i].as_str() {
            "-v" => {
                verbose = a[i + 1].parse().unwrap();
                i += 1;
            }
            "-k" => {
                only_node = Some(a[i + 1].clone());
                i += 1;
            }
            p => files(std::path::Path::new(p), &mut paths),
        }
        i += 1;
    }
    let g = lang_sql::provider().grammar().clone();
    let (mut total, mut old_ok, mut new_ok, mut tok_eq, mut node_eq) = (0, 0, 0, 0, 0);
    let mut kind_diff: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut shown = 0;
    let mut firsts: BTreeMap<String, usize> = BTreeMap::new();
    for p in paths {
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        total += 1;
        let old = lang_sql::parse(&text, &dialect);
        let new = lang_sql::parse_hand(&text, &dialect);
        assert_eq!(core_syntax::print(&new.tree), text, "round trip {}", p.display());
        if new.errors.is_empty() {
            new_ok += 1;
        }
        if !old.errors.is_empty() {
            continue;
        }
        old_ok += 1;
        let (mut oi, mut on, mut ni, mut nn) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        walk(SyntaxElement::Node(old.tree.clone()), &g, &mut oi, &mut on);
        walk(SyntaxElement::Node(new.tree.clone()), &g, &mut ni, &mut nn);
        let (oi, ni) = (merge_ops(oi), merge_ops(ni));
        let same_tokens = oi == ni;
        let mut counts: BTreeMap<&str, (isize, isize)> = BTreeMap::new();
        let key = |n: &(String, usize, usize)| (n.0.clone(), n.1, n.2);
        let oset: std::collections::HashMap<_, usize> = on.iter().fold(Default::default(), |mut m, n| {
            *m.entry(key(n)).or_default() += 1;
            m
        });
        let nset: std::collections::HashMap<_, usize> = nn.iter().fold(Default::default(), |mut m, n| {
            *m.entry(key(n)).or_default() += 1;
            m
        });
        if same_tokens {
            tok_eq += 1;
            if oset == nset {
                node_eq += 1;
            } else {
                for (kk, v) in &oset {
                    if nset.get(kk) != Some(v) {
                        kind_diff.entry(kk.0.clone()).or_default().0 += 1;
                    }
                }
                for (kk, v) in &nset {
                    if oset.get(kk) != Some(v) {
                        kind_diff.entry(kk.0.clone()).or_default().1 += 1;
                    }
                }
                if let Some(want) = &only_node {
                    if shown < verbose {
                        let miss: Vec<_> = oset.keys().filter(|k| !nset.contains_key(*k) && &k.0 == want).take(1).collect();
                        let extra: Vec<_> = nset.keys().filter(|k| !oset.contains_key(*k) && &k.0 == want).take(1).collect();
                        for k in miss.iter().chain(extra.iter()) {
                            shown += 1;
                            let side = if oset.contains_key(*k) { "OLD only" } else { "NEW only" };
                            println!("{} {side} {} [{}]: {}", p.display(), k.0, k.1, show_tok(&oi, k.1, k.2));
                        }
                    }
                }
            }
        } else if only_node.is_none() {
            let at = oi.iter().zip(ni.iter()).position(|(x, y)| x != y).unwrap_or(oi.len().min(ni.len()));
            let f = |v: &Vec<Item>| v.get(at).map_or("END".to_string(), |i| show(std::slice::from_ref(i), 0, 1));
            let prev = if at > 0 { show(&oi, at - 1, at) } else { String::new() };
            *firsts.entry(format!("{prev} | OLD {} | NEW {}", f(&oi), f(&ni))).or_default() += 1;
        }
        if !same_tokens && only_node.is_none() && shown < verbose {
            shown += 1;
            let at = oi.iter().zip(ni.iter()).position(|(x, y)| x != y).unwrap_or(oi.len().min(ni.len()));
            println!(
                "{}\n  at {at}\n  OLD: {}\n  NEW: {}",
                p.display(),
                show(&oi, at.saturating_sub(8), at + 8),
                show(&ni, at.saturating_sub(8), at + 8)
            );
        }
        let _ = &mut counts;
    }
    let mut fv: Vec<_> = firsts.into_iter().collect();
    fv.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (kk, n) in fv.iter().take(40) {
        println!("{n:4} {kk}");
    }
    println!("{dialect}: files={total} old_ok={old_ok} new_ok={new_ok} tokens_equal={tok_eq} trees_equal={node_eq}");
    for (k, (o, n)) in kind_diff {
        println!("  node {k}: old-only {o}, new-only {n}");
    }
}

/// The items of token indices `from..to`, with a little context.
fn show_tok(items: &[Item], from: usize, to: usize) -> String {
    let mut idx = Vec::new();
    let mut t = 0;
    for (i, it) in items.iter().enumerate() {
        if matches!(it, Item::Tok(..)) {
            if t == from.saturating_sub(3) {
                idx.push(i);
            }
            if t == to + 3 {
                idx.push(i);
            }
            t += 1;
        }
    }
    let a = idx.first().copied().unwrap_or(0);
    let b = idx.get(1).copied().unwrap_or(items.len());
    show(items, a, b)
}
