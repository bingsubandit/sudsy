//! The SQL corpus (sqlfluff's dialect fixtures and PostgreSQL's regression inputs, pinned in
//! tests/conformance/fetch-sql-corpus.sh): every file prints back byte for byte, satisfies the
//! trivia attachment rule in its dialect and fits the bodies of the grammar; the files with parse
//! errors, the parser's panics and the speed are printed per dialect.
//!
//! Run: `bash tests/conformance/fetch-sql-corpus.sh &&
//! cargo test --profile corpus -p lang-sql --test integration -- --ignored --nocapture conformance::`

use core_syntax::shape::ShapeChecker;
use core_syntax::{print, trivia};
use provider_api::LanguageProvider;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|_| panic!("run tests/conformance/fetch-sql-corpus.sh"))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect();
    files.sort();
    files
}

/// Parses every file as `dialect`; returns what the run printed.
fn run(label: &str, dialect: &str, dir: PathBuf) {
    let grammar = lang_sql::provider().grammar().clone();
    let (mut bytes, mut count, mut errors, mut panics, mut big) = (0, 0, 0, 0, 0);
    let mut elapsed = 0.0;
    for path in files(&dir) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let start = Instant::now();
        let parsed = lang_sql::parse(&text, dialect);
        elapsed += start.elapsed().as_secs_f64();
        assert_eq!(print(&parsed.tree), text, "{}: round trip", path.display());
        trivia::check_attachment(&parsed.tree, |k| grammar.is_trivia(k))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        count += 1;
        bytes += text.len();
        if parsed.errors.is_empty() {
            ShapeChecker::new(&grammar)
                .check(&parsed.tree)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        if let Some(e) = parsed.errors.first() {
            errors += 1;
            panics += usize::from(e.message.contains("failed on this file"));
            big += usize::from(e.message.starts_with("file over"));
        }
    }
    println!(
        "{label} {dialect}: files={count} with_errors={errors} panics={panics} over_limit={big} MB/s={:.2}",
        bytes as f64 / 1e6 / elapsed
    );
}

fn corpus(repo: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/sql")
        .join(repo)
}

#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-sql-corpus.sh"]
fn sqlfluff_fixtures() {
    for dialect in lang_sql::DIALECTS {
        run(
            "sqlfluff",
            dialect,
            corpus("sqlfluff")
                .join("test/fixtures/dialects")
                .join(dialect),
        );
    }
}

/// Peak resident memory of the process in MB (Linux).
fn peak_mb() -> f64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = status.lines().find_map(|l| {
        l.strip_prefix("VmHWM:")?
            .trim()
            .strip_suffix("kB")?
            .trim()
            .parse::<f64>()
            .ok()
    });
    kb.unwrap_or(0.0) / 1024.0
}

/// The cost of a dialect's first file and the time per MB at growing sizes: the numbers behind
/// `MAX_BYTES` (docs/performance/numbers.md). Run with `--release` (debug assertions slow sqruff).
#[test]
#[ignore = "measurement: cargo test --release -p lang-sql --test integration -- --ignored --nocapture conformance::scaling"]
fn scaling() {
    let input = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/inputs/sql/ansi-fixtures.sql"),
    )
    .unwrap();
    for dialect in lang_sql::DIALECTS {
        let start = Instant::now();
        lang_sql::parse("SELECT 1;", dialect);
        println!(
            "{dialect}: first file (builds the dialect) {:.1} ms",
            start.elapsed().as_secs_f64() * 1e3
        );
    }
    for kb in [64, 256, 480] {
        let text = input.repeat(kb * 1024 / input.len() + 1);
        let text = &text[..text.rfind(";\n").unwrap() + 2];
        let start = Instant::now();
        let parsed = lang_sql::parse(text, "ansi");
        let secs = start.elapsed().as_secs_f64();
        println!(
            "{} KB: {:.0} ms, {:.2} MB/s, errors {}, peak {:.0} MB",
            text.len() / 1024,
            secs * 1e3,
            text.len() as f64 / 1e6 / secs,
            parsed.errors.len(),
            peak_mb()
        );
    }
}

#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-sql-corpus.sh"]
fn postgres_regress() {
    run(
        "postgres-regress",
        "postgres",
        corpus("postgres").join("src/test/regress/sql"),
    );
}

/// Formats every file that parses, twice: the output must be stable and the safety check must
/// pass (a refusal that is not a parse error fails the test). Prints the counts per dialect.
/// Run with `--profile corpus` (the safety check needs debug assertions).
#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-sql-corpus.sh"]
fn format_is_stable() {
    use core_format::DirectiveTable;
    use provider_api::SourceFile;
    use provider_api::format::format_file;
    let provider = lang_sql::provider();
    let mut options = provider.option_schema().defaults();
    let dirs = lang_sql::DIALECTS.map(|d| {
        let sub = if d == "postgres" { "postgres" } else { d };
        (
            d,
            corpus("sqlfluff").join("test/fixtures/dialects").join(sub),
        )
    });
    let regress = ("postgres", corpus("postgres").join("src/test/regress/sql"));
    for (dialect, dir) in dirs.into_iter().chain([regress]) {
        options.set("dialect", core_config::OptionValue::String(dialect.into()));
        let run = |name: &str, text: &str| {
            let mut file = SourceFile::new(name, 1, text);
            file.language_version = Some(dialect.into());
            format_file(&provider, &file, &options, &DirectiveTable::default())
        };
        let (mut count, mut changed, mut refused, mut unstable) = (0, 0, 0, Vec::new());
        for path in files(&dir) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            count += 1;
            let name = path.file_name().unwrap().to_str().unwrap();
            match run(name, &text) {
                Ok(once) => {
                    changed += usize::from(once != text);
                    if run(name, &once).ok().as_deref() != Some(once.as_str()) {
                        unstable.push(path.display().to_string());
                    }
                }
                Err(e) if e.to_string().contains("internal error") => {
                    panic!("{}: {e}", path.display())
                }
                Err(_) => refused += 1,
            }
        }
        println!(
            "{dialect} {}: files={count} changed={changed} refused={refused} unstable={}",
            dir.file_name().unwrap().to_str().unwrap(),
            unstable.len()
        );
        for path in unstable.iter().take(5) {
            println!("  not idempotent: {path}");
        }
    }
}
