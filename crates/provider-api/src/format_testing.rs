//! The format tests every language repeats: a provider as a `core_format::testing::Subject`, the
//! corpus run, and the file-name check.

use crate::format::format_file;
use crate::provider::{LanguageProvider, ParseResult, SourceFile};
use core_config::OptionValue;
use core_format::DirectiveTable;
use core_format::testing::{Subject, check};
use core_syntax::shape::ShapeChecker;
use core_syntax::{Grammar, SyntaxNode, print, trivia};
use std::path::{Path, PathBuf};

/// A provider formatting files named `file` with its default options plus `options`.
pub struct Formatting {
    pub provider: Box<dyn LanguageProvider>,
    pub file: &'static str,
    pub options: Vec<(&'static str, OptionValue)>,
}

impl Formatting {
    pub fn new(provider: impl LanguageProvider + 'static, file: &'static str) -> Self {
        Self {
            provider: Box::new(provider),
            file,
            options: Vec::new(),
        }
    }
}

impl Subject for Formatting {
    fn grammar(&self) -> &Grammar {
        self.provider.grammar()
    }

    fn parse(&self, text: &str) -> Result<SyntaxNode, String> {
        let r = self
            .provider
            .parse(&self.source(text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => {
                check_shape(self.grammar(), &r, self.file);
                Ok(r.tree)
            }
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            &*self.provider,
            &self.source(text),
            &self.resolved(),
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }

    fn rewritten(&self, input: &str) -> String {
        let mut text = input.to_string();
        for _ in 0..self.provider.formatter().rewrite_rounds() {
            let Ok(tree) = self.parse(&text) else {
                break;
            };
            match self
                .provider
                .formatter()
                .rewrite_source(&tree, &text, self.grammar(), &self.resolved())
                .filter(|t| *t != text)
            {
                Some(next) => text = next,
                None => break,
            }
        }
        text
    }
}

impl Formatting {
    /// The file for `text`, parsed in the dialect (or target version) the options choose.
    fn source(&self, text: &str) -> SourceFile {
        let mut file = SourceFile::new(self.file, 1, text);
        file.language_version = crate::format::resolve_dialect(&mut self.resolved(), None);
        file
    }

    fn resolved(&self) -> core_config::ResolvedOptions {
        let schema = self.provider.option_schema();
        let mut options = schema.defaults();
        for (k, v) in &self.options {
            schema.apply(&mut options, k, v).unwrap();
        }
        options
    }
}

/// Formats `input` with every check of `core_format::testing` (round trip, meaning, idempotency,
/// comments kept) and returns the output.
#[track_caller]
pub fn assert_formats(
    provider: impl LanguageProvider + 'static,
    file: &'static str,
    input: &str,
) -> String {
    core_format::testing::assert_formats(&Formatting::new(provider, file), input)
}

/// Runs the checks on every file of `corpus` (a folder of `tests/conformance/corpus`, fetched by
/// its `fetch-*-corpus.sh`) whose extension is in `extensions` (any when empty). Panics on a
/// failure that is not a parse error; returns the number of files and the parse errors.
pub fn check_corpus(
    provider: impl LanguageProvider + 'static,
    file: &'static str,
    corpus: &str,
    extensions: &[&str],
) -> (usize, Vec<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/corpus");
    let subject = Formatting::new(provider, file);
    let mut dirs = vec![root.join(corpus)];
    let (mut count, mut failures, mut unparsed) = (0, Vec::new(), Vec::new());
    while let Some(d) = dirs.pop() {
        let entries = std::fs::read_dir(&d)
            .unwrap_or_else(|_| panic!("run tests/conformance/fetch-{corpus}-corpus.sh"));
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let wanted = extensions.is_empty()
                || path
                    .extension()
                    .is_some_and(|e| extensions.iter().any(|x| e == *x));
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !wanted {
                continue;
            }
            count += 1;
            if let Err(e) = check(&subject, text.trim_start_matches('\u{feff}')) {
                let e = e.lines().next().unwrap_or("");
                let list = if e.starts_with("input: ") {
                    &mut unparsed
                } else {
                    &mut failures
                };
                list.push(format!("{}: {e}", path.display()));
            }
        }
    }
    println!(
        "{count} files, {} not parsed, {} failures",
        unparsed.len(),
        failures.len()
    );
    println!("{}", unparsed.join("\n"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    (count, unparsed)
}

/// The provider claims every path of `yes` and none of `no`.
#[track_caller]
pub fn assert_file_names(provider: &dyn LanguageProvider, yes: &[&str], no: &[&str]) {
    let claims = |p: &str| {
        provider
            .file_matchers()
            .iter()
            .any(|m| m.matches(&PathBuf::from(p)))
    };
    for p in yes {
        assert!(claims(p), "{p}");
    }
    for p in no {
        assert!(!claims(p), "{p}");
    }
}

/// Parses `text` as the file `name` and checks the tree: it prints back byte for byte and
/// satisfies the trivia attachment rule. Returns the parse, errors included.
#[track_caller]
pub fn assert_lossless(provider: &dyn LanguageProvider, name: &str, text: &str) -> ParseResult {
    let parsed =
        (provider.parse(&SourceFile::new(name, 1, text))).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(print(&parsed.tree), text, "{name}: round trip");
    let grammar = provider.grammar();
    trivia::check_attachment(&parsed.tree, |k| grammar.is_trivia(k))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    check_shape(grammar, &parsed, name);
    parsed
}

/// A grammar that opted into roles (`roles`) has bodies that describe every child: the tree fits
/// them (nothing to check for the others, whose bodies are partial).
#[track_caller]
fn check_shape(grammar: &Grammar, parsed: &ParseResult, name: &str) {
    if grammar.requires_roles && parsed.errors.is_empty() {
        ShapeChecker::new(grammar)
            .check(&parsed.tree)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

/// [`assert_lossless`] on a file that must parse: no errors, no `#error` token, and every token
/// of fixed text has it.
#[track_caller]
pub fn assert_parses(provider: &dyn LanguageProvider, name: &str, text: &str) -> ParseResult {
    let parsed = assert_lossless(provider, name, text);
    let first = parsed.errors.first();
    assert!(first.is_none(), "{name}: {first:?}");
    let grammar = provider.grammar();
    for t in (parsed.tree.descendants_with_tokens()).filter_map(|e| e.into_token()) {
        assert!(
            !grammar.has_tag(t.kind(), "error"),
            "{name}: unknown token {t:?}"
        );
        if let Some(fixed) = grammar.def(t.kind()).and_then(|d| d.text.as_deref()) {
            assert_eq!(t.text(), fixed, "{name}: {:?}", t.kind());
        }
    }
    parsed
}

/// The files of `bench/inputs/<folder>` as `(name, text)`; at least four.
pub fn bench_input_files(folder: &str) -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/inputs");
    let files: Vec<_> = (std::fs::read_dir(dir.join(folder)).unwrap())
        .map(|e| e.unwrap().path())
        .map(|p| {
            let name = p.file_name().unwrap().to_str().unwrap().to_string();
            (name, std::fs::read_to_string(p).unwrap())
        })
        .collect();
    assert!(files.len() >= 4, "bench inputs of {folder} missing");
    files
}
