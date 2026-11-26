//! tree-sitter-rust (grammar crate on tree-sitter 0.27). Editions: one grammar for all, the edition is not an input.
use evidence_rust_parsers::{best_of, load, mutant, probes, report, Item};
use std::hint::black_box;
use std::path::Path;

/// Byte spans of the outermost error or missing nodes of a tree.
fn error_spans(node: tree_sitter::Node, out: &mut Vec<usize>) {
    if node.is_error() || node.is_missing() {
        out.push(node.end_byte() - node.start_byte());
        return;
    }
    let mut c = node.walk();
    for child in node.children(&mut c) {
        if child.has_error() {
            error_spans(child, out);
        }
    }
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let list = std::env::args().nth(1).expect("usage: ts <list.tsv>");
    let items: Vec<Item> = load(Path::new(&list));
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).expect("language");
    println!("tree-sitter ABI: {}", lang.abi_version());

    let errors: Vec<bool> = items.iter().map(|i| parser.parse(&i.text, None).expect("tree").root_node().has_error()).collect();
    let (secs, _) = best_of(5, || {
        items.iter().map(|i| black_box(parser.parse(&i.text, None).expect("tree").root_node().end_byte())).sum()
    });

    let mut mutants = Vec::new();
    for i in &items {
        let Some(m) = mutant(&i.text) else { continue };
        let tree = parser.parse(&m, None).expect("tree");
        let root = tree.root_node();
        let mut spans = Vec::new();
        if root.has_error() {
            error_spans(root, &mut spans);
        }
        let lost: usize = spans.iter().sum();
        mutants.push((root.has_error(), lost as f64 / m.len().max(1) as f64));
    }

    let probe_rows: Vec<(String, String, bool)> = probes(&dir.join("probes"))
        .into_iter()
        .map(|(n, e, t)| {
            let tree = parser.parse(&t, None).expect("tree");
            (n, e, !tree.root_node().has_error())
        })
        .collect();
    report("tree-sitter-rust 0.24.2 (tree-sitter 0.27)", &items, &errors, secs, &mutants, &probe_rows);
}
