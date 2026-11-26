//! syn (`parse_file`, full syntax). Editions: syn has no edition input. An error gives no tree: a damaged file is detected,
//! and no share of it is recovered (the share is NaN: not measured).
use evidence_rust_parsers::{best_of, load, mutant, probes, report, Item};
use std::hint::black_box;
use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let list = std::env::args().nth(1).expect("usage: syn <list.tsv>");
    let items: Vec<Item> = load(Path::new(&list));

    let errors: Vec<bool> = items.iter().map(|i| syn::parse_file(&i.text).is_err()).collect();
    let (secs, _) = best_of(5, || items.iter().map(|i| black_box(syn::parse_file(&i.text).is_ok()) as usize).sum());
    let mutants: Vec<(bool, f64)> = items
        .iter()
        .filter_map(|i| mutant(&i.text))
        .map(|m| (syn::parse_file(&m).is_err(), f64::NAN))
        .collect();
    let probe_rows: Vec<(String, String, bool)> = probes(&dir.join("probes"))
        .into_iter()
        .map(|(n, e, t)| (n, e, syn::parse_file(&t).is_ok()))
        .collect();
    report("syn 3 (parse_file)", &items, &errors, secs, &mutants, &probe_rows);
}
