//! `perf-bench trees DIR LANG...`: the tree, errors and formatted text of every file of those
//! languages, to compare two builds of the parsers.
use anyhow::Result;
use core_format::DirectiveTable;
use perf_bench::file;
use std::path::Path;

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let mut es: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    es.sort();
    for p in es {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

pub fn run(dir: &Path, langs: &[String]) -> Result<()> {
    let providers = languages::providers();
    let mut files = Vec::new();
    walk(dir, &mut files);
    let (mut n, mut skipped) = (0usize, 0usize);
    for path in files {
        let Some((i, dialect)) = provider_api::select_provider_index(&providers, &path) else {
            skipped += 1;
            continue;
        };
        let p = providers[i].as_ref();
        if !langs.iter().any(|l| l == p.language()) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let (mut f, mut opts) = file(p, &path.to_string_lossy(), 1, &text);
        f.language_version = provider_api::resolve_dialect(&mut opts, dialect);
        n += 1;
        println!("== {}", path.display());
        match p.parse(&f) {
            Ok(r) => {
                print!("{}", core_syntax::dump_tree(&r.tree, p.grammar()));
                for e in &r.errors {
                    println!("error {} {} {} {}", e.code, e.start, e.len, e.message);
                }
            }
            Err(e) => println!("parse failed {e}"),
        }
        match provider_api::format_file(p, &f, &opts, &DirectiveTable::default()) {
            Ok(o) => println!("format:\n{:?}", o),
            Err(e) => println!("format failed {e:?}"),
        }
    }
    eprintln!("{n} files, {skipped} unclaimed");
    Ok(())
}
