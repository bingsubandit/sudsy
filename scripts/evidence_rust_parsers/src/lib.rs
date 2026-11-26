//! Shared part of the parser measurement (docs/ai/design/rust.md): the corpus list written by
//! `scripts/evidence_rust_tools.py` (`list.tsv`: path, tab, edition), the edition probes (`probes/NN-name.EDITION.rs`), the
//! mutants (one closing bracket removed from each file, the middle one) and the timing (best of 5, warm, from memory).
//! Each binary (`base`, `ts`, `syn`, `ra`) gives the same report lines for its parser; `base` gives none (binary size).
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct Item {
    pub path: PathBuf,
    pub edition: String,
    pub text: String,
}

/// The corpus files of `list.tsv` (path and edition), read into memory before any timing.
pub fn load(list: &Path) -> Vec<Item> {
    let body = fs::read_to_string(list).expect("list.tsv");
    body.lines()
        .filter_map(|l| {
            let (p, e) = l.split_once('\t')?;
            let text = fs::read_to_string(p).ok()?;
            Some(Item { path: PathBuf::from(p), edition: e.to_string(), text })
        })
        .collect()
}

/// The probes of `probes/` (name, edition from the file name, text), in name order.
pub fn probes(dir: &Path) -> Vec<(String, String, String)> {
    let mut names: Vec<PathBuf> = fs::read_dir(dir).expect("probes").map(|e| e.expect("entry").path()).collect();
    names.sort();
    names
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .map(|p| {
            let stem = p.file_stem().expect("stem").to_string_lossy().to_string();
            let (name, edition) = stem.rsplit_once('.').expect("NAME.EDITION.rs");
            (name.to_string(), edition.to_string(), fs::read_to_string(&p).expect("probe"))
        })
        .collect()
}

/// The mutant of a file: its middle `)`, `]` or `}` removed (none when the file has no closing bracket).
pub fn mutant(text: &str) -> Option<String> {
    let idx: Vec<usize> = text.char_indices().filter(|(_, c)| matches!(c, ')' | ']' | '}')).map(|(i, _)| i).collect();
    let at = *idx.get(idx.len() / 2)?;
    let mut s = String::with_capacity(text.len());
    s.push_str(&text[..at]);
    s.push_str(&text[at + 1..]);
    Some(s)
}

/// Best of `runs` wall times of `f` (seconds), and the value of the last run.
pub fn best_of<F: FnMut() -> usize>(runs: usize, mut f: F) -> (f64, usize) {
    let mut best = f64::MAX;
    let mut last = 0;
    for _ in 0..runs {
        let t = Instant::now();
        last = f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    (best, last)
}

pub fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    v[v.len() / 2]
}

pub fn pct(part: usize, whole: usize) -> f64 {
    100.0 * part as f64 / whole.max(1) as f64
}

/// The shared report: corpus size, the timing, the mutants and the probes. `errors` is the parse-error count per file.
pub fn report(name: &str, items: &[Item], errors: &[bool], secs: f64, mutants: &[(bool, f64)], probe_rows: &[(String, String, bool)]) {
    let bytes: usize = items.iter().map(|i| i.text.len()).sum();
    let failed = errors.iter().filter(|e| **e).count();
    println!("parser: {name}");
    println!("corpus files: {}, bytes: {bytes}", items.len());
    println!("files with a parse error: {failed} ({:.1}%)", pct(failed, items.len()));
    for (i, e) in items.iter().zip(errors) {
        if *e {
            println!("error file: {}", i.path.display());
        }
    }
    println!("parse time, best of 5 (whole corpus, from memory): {:.4} s, {:.1} MB/s", secs, bytes as f64 / 1e6 / secs);
    let detected = mutants.iter().filter(|(d, _)| *d).count();
    // NaN: the parser gives no tree of a damaged file, so no share is measured
    let lost: Vec<f64> = mutants.iter().filter(|(d, l)| *d && !l.is_nan()).map(|(_, l)| *l).collect();
    println!("mutants (one closing bracket removed): {}, reported as an error: {detected} ({:.1}%)", mutants.len(), pct(detected, mutants.len()));
    if lost.is_empty() {
        println!("mutants, share of the file in error nodes: not measured for this parser");
    } else {
        let p75 = { let mut s = lost.clone(); s.sort_by(|a, b| a.partial_cmp(b).expect("finite")); s[(s.len() * 3 / 4).min(s.len() - 1)] };
        println!("mutants, share of the file in error nodes: median {:.1}%, p75 {:.1}%", 100.0 * median(lost.clone()), 100.0 * p75);
    }
    let ok = probe_rows.iter().filter(|(_, _, ok)| *ok).count();
    println!("probes parsed: {ok} of {}", probe_rows.len());
    for (n, e, ok) in probe_rows {
        println!("probe {n} {e}: {}", if *ok { "ok" } else { "error" });
    }
}
