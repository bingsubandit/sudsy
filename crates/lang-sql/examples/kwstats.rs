//! Temporary: words sqruff classes as keywords in the fixtures. `kwstats <outfile>`
use core_syntax::SyntaxElement;
use provider_api::LanguageProvider;
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = lang_sql::provider().grammar().clone();
    let root = std::path::PathBuf::from("tests/conformance/corpus/sql/sqlfluff/test/fixtures/dialects");
    let mut stats: BTreeMap<String, (u32, u32, u32)> = BTreeMap::new(); // keyword, identifier-ish, other
    for d in ["ansi", "postgres", "mysql", "tsql"] {
        for e in std::fs::read_dir(root.join(d)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_none_or(|e| e != "sql") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let r = lang_sql::parse(&text, d);
            if !r.errors.is_empty() {
                continue;
            }
            for el in r.tree.descendants_with_tokens() {
                if let SyntaxElement::Token(t) = el {
                    let name = g.name(t.kind());
                    let w = t.text();
                    if !w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || w.chars().next().is_none_or(|c| c.is_ascii_digit()) {
                        continue;
                    }
                    let e = stats.entry(w.to_ascii_uppercase()).or_default();
                    match name {
                        "Keyword" | "BinaryKeyword" => e.0 += 1,
                        "Identifier" | "QuotedIdentifier" | "Word" | "ObjectReference" => e.1 += 1,
                        _ => e.2 += 1,
                    }
                }
            }
        }
    }
    let mut out = String::new();
    for (w, (k, i, o)) in stats {
        {
            out.push_str(&format!("{w} {k} {i} {o}\n"));
        }
    }
    std::fs::write(&a[1], out).unwrap();
}
