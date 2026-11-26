//! ra_ap_syntax (rust-analyzer's parser, published crate; lossless rowan tree with ERROR nodes). Editions: the parse takes
//! the file's edition, as in the corpus (Cargo.toml of its project; 2021 for a single file).
use evidence_rust_parsers::{best_of, load, mutant, probes, report, Item};
use ra_ap_syntax::{Edition, SourceFile, SyntaxKind, SyntaxNode};
use std::hint::black_box;
use std::path::Path;

fn edition(s: &str) -> Edition {
    match s {
        "2015" => Edition::Edition2015,
        "2018" => Edition::Edition2018,
        "2021" => Edition::Edition2021,
        "2024" => Edition::Edition2024,
        other => panic!("edition {other}"),
    }
}

/// Bytes of the outermost ERROR nodes of a tree.
fn error_bytes(root: &SyntaxNode) -> usize {
    root.descendants()
        .filter(|n| n.kind() == SyntaxKind::ERROR && !n.ancestors().skip(1).any(|a| a.kind() == SyntaxKind::ERROR))
        .map(|n| usize::from(n.text_range().len()))
        .sum()
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let list = std::env::args().nth(1).expect("usage: ra <list.tsv>");
    let items: Vec<Item> = load(Path::new(&list));

    let errors: Vec<bool> = items
        .iter()
        .map(|i| !SourceFile::parse(&i.text, edition(&i.edition)).errors().is_empty())
        .collect();
    let (secs, _) = best_of(5, || {
        items
            .iter()
            .map(|i| black_box(usize::from(SourceFile::parse(&i.text, edition(&i.edition)).syntax_node().text_range().len())))
            .sum::<usize>()
    });
    let mutants: Vec<(bool, f64)> = items
        .iter()
        .filter_map(|i| mutant(&i.text).map(|m| (m, edition(&i.edition))))
        .map(|(m, e)| {
            let p = SourceFile::parse(&m, e);
            let root = p.syntax_node();
            let lost = error_bytes(&root) as f64 / m.len().max(1) as f64;
            (!p.errors().is_empty(), lost)
        })
        .collect();
    let probe_rows: Vec<(String, String, bool)> = probes(&dir.join("probes"))
        .into_iter()
        .map(|(n, e, t)| {
            let p = SourceFile::parse(&t, edition(&e));
            (n, e, p.errors().is_empty())
        })
        .collect();
    report("ra_ap_syntax 0.0.364", &items, &errors, secs, &mutants, &probe_rows);
}
