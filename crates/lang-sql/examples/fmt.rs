//! Temporary: formats every file with sqruff's tree and with the hand-written parser's, compares.
//! `fmt <dialect> <dir> [-v N]`
use core_format::DirectiveTable;
use provider_api::LanguageProvider;
use provider_api::SourceFile;
use provider_api::format::format_file;
use std::sync::atomic::Ordering;

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
    let mut verbose = 0;
    let mut paths = Vec::new();
    let mut i = 2;
    while i < a.len() {
        if a[i] == "-v" {
            verbose = a[i + 1].parse().unwrap();
            i += 1;
        } else {
            files(std::path::Path::new(&a[i]), &mut paths);
        }
        i += 1;
    }
    let provider = lang_sql::provider();
    let mut options = provider.option_schema().defaults();
    options.set("dialect", core_config::OptionValue::String(dialect.clone()));
    let run = |text: &str, hand: bool| {
        lang_sql::HAND.store(hand, Ordering::Relaxed);
        let mut file = SourceFile::new("a.sql", 1, text);
        file.language_version = Some(dialect.clone());
        format_file(&provider, &file, &options, &DirectiveTable::default()).ok()
    };
    let (mut total, mut old_fmt, mut same, mut new_refused, mut shown) = (0, 0, 0, 0, 0);
    let mut diffs = Vec::new();
    for p in paths {
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        total += 1;
        let Some(old) = run(&text, false) else { continue };
        old_fmt += 1;
        match run(&text, true) {
            None => new_refused += 1,
            Some(new) if new == old => same += 1,
            Some(new) => {
                diffs.push(p.display().to_string());
                if shown < verbose {
                    shown += 1;
                    let (ol, nl): (Vec<_>, Vec<_>) = (old.lines().collect(), new.lines().collect());
                    let at = ol.iter().zip(nl.iter()).position(|(x, y)| x != y).unwrap_or(ol.len().min(nl.len()));
                    println!("{}:{}\n  OLD| {}\n  NEW| {}", p.display(), at + 1, ol.get(at).unwrap_or(&""), nl.get(at).unwrap_or(&""));
                }
            }
        }
    }
    println!("{dialect}: files={total} old_formats={old_fmt} same_output={same} new_refused={new_refused} differ={}", diffs.len());
}
