//! Shared part of the Helm parser measurement (docs/ai/design/helm.md): the corpus list written by
//! `python3 -I scripts/evidence.py helm list` (project, half and path, tab-separated; the path is read), the probes
//! (`probes/NN-name.txt`), the mutants (the middle `}}` removed from each file) and the timing (best of 5, warm, from memory).
//! Each binary (`base`, `ts`, `hand`) hands one check to `run`; the report lines are the same for all.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The result of one check of one text: whether the parser reports an error, and the share of the text in error (NaN when the
/// parser gives no tree of a damaged text, or the check has none).
pub struct Check {
    pub error: bool,
    pub share: f64,
}

pub struct Item {
    pub path: PathBuf,
    pub text: String,
}

/// The corpus files of the list (the path is the third tab-separated column), read into memory before any timing.
pub fn load(list: &Path) -> Vec<Item> {
    let body = fs::read_to_string(list).expect("list.tsv");
    body.lines()
        .filter_map(|l| {
            let p = l.split('\t').nth(2)?;
            let text = fs::read_to_string(p).ok()?;
            Some(Item { path: PathBuf::from(p), text })
        })
        .collect()
}

/// The probes of `dir` (name and text, in name order).
pub fn probes(dir: &Path) -> Vec<(String, String)> {
    let mut names: Vec<PathBuf> = fs::read_dir(dir).expect("probes").map(|e| e.expect("entry").path()).collect();
    names.sort();
    names
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .map(|p| {
            let name = p.file_stem().expect("stem").to_string_lossy().to_string();
            (name, fs::read_to_string(&p).expect("probe"))
        })
        .collect()
}

/// The mutant of a text: its middle `}}` removed (None when the text has no `}}`).
pub fn mutant(text: &str) -> Option<String> {
    let idx: Vec<usize> = text.match_indices("}}").map(|(i, _)| i).collect();
    let at = *idx.get(idx.len() / 2)?;
    Some(format!("{}{}", &text[..at], &text[at + 2..]))
}

pub fn pct(part: usize, whole: usize) -> f64 {
    100.0 * part as f64 / whole.max(1) as f64
}

/// Runs `check` over the corpus (timed), the mutants and the probes, and prints the report lines.
pub fn run(name: &str, items: &[Item], probes: &[(String, String)], mut check: impl FnMut(&str) -> Check) {
    let bytes: usize = items.iter().map(|i| i.text.len()).sum();
    let mut best = f64::MAX;
    let mut errors = 0;
    for run in 0..5 {
        let t = Instant::now();
        let n = items.iter().filter(|i| check(&i.text).error).count();
        best = best.min(t.elapsed().as_secs_f64());
        if run == 0 {
            errors = n;
        }
    }
    println!("parser: {name}");
    println!("corpus files: {}, bytes: {bytes}", items.len());
    println!("files with an error: {errors} ({:.1}%)", pct(errors, items.len()));
    for i in items {
        if check(&i.text).error {
            println!("error file: {}", i.path.display());
        }
    }
    println!("parse time, best of 5 (whole corpus, from memory): {best:.4} s, {:.1} MB/s", bytes as f64 / 1e6 / best);
    let mutants: Vec<Check> = items.iter().filter_map(|i| mutant(&i.text)).map(|m| check(&m)).collect();
    let detected = mutants.iter().filter(|c| c.error).count();
    println!("mutants (middle `}}` removed): {}, reported as an error: {detected} ({:.1}%)", mutants.len(), pct(detected, mutants.len()));
    let mut shares: Vec<f64> = mutants.iter().filter(|c| c.error && !c.share.is_nan()).map(|c| c.share).collect();
    if shares.is_empty() {
        println!("mutants, share of the text in error nodes: not measured for this parser");
    } else {
        shares.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        let median = shares[shares.len() / 2];
        println!("mutants, share of the text in error nodes: median {:.1}%", 100.0 * median);
    }
    let ok = probes.iter().filter(|(_, t)| !check(t).error).count();
    println!("probes parsed: {ok} of {}", probes.len());
    for (n, t) in probes {
        let c = check(t);
        println!("probe {n}: {}", if c.error { "error" } else { "ok" });
    }
}
