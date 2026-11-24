use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use core_syntax::{dump_tree, grammars, print, trivia};
use lang_csharp as csharp;
use provider_api::{LanguageProvider, SourceFile};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum DebugCommand {
    /// Parse a .cs file with the C# worker, check it round-trips, and print the tree.
    Parse {
        file: PathBuf,
        /// C# language version, e.g. 12 or latest.
        #[arg(long)]
        language_version: Option<String>,
    },
    /// Parse a file of any language with its own provider and print the tree (kind, byte range, token text).
    Tree { file: PathBuf },
}

pub fn run(command: DebugCommand) -> Result<()> {
    match command {
        DebugCommand::Parse {
            file,
            language_version,
        } => parse(&file, language_version),
        DebugCommand::Tree { file } => tree(&file),
    }
}

fn tree(file: &Path) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let source = SourceFile::from_bytes(file.to_string_lossy(), 1, bytes)?;
    let providers = languages::providers();
    let provider = providers
        .iter()
        .filter(|p| p.file_matchers().iter().any(|m| m.matches(file)))
        .max_by_key(|p| p.file_matchers().iter().filter(|m| m.matches(file)).map(|m| m.specificity()).max())
        .with_context(|| format!("no language for {}", file.display()))?;
    let result = provider.parse(&source)?;
    // Whitespace and newline tokens are left out: their text is in the gaps between the byte ranges.
    for line in dump_tree(&result.tree, provider.grammar()).lines() {
        let blank = line.split_once(" \"").is_some_and(|(_, q)| {
            let inner = q.trim_end_matches('"');
            !inner.is_empty() && inner.replace("\\n", "").replace("\\r", "").replace("\\t", "").replace(' ', "").is_empty()
        });
        if !blank {
            println!("{line}");
        }
    }
    Ok(())
}

fn parse(file: &Path, language_version: Option<String>) -> Result<()> {
    ensure!(
        file.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cs")),
        "`debug parse` takes a .cs file"
    );
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let mut source = SourceFile::from_bytes(file.to_string_lossy(), 1, bytes)?;
    source.language_version = language_version;

    let worker = csharp::locate_worker().map_err(anyhow::Error::msg)?;
    let provider = csharp::provider(&worker);
    let result = provider.parse(&source)?;

    let grammar = grammars::csharp();
    print!("{}", dump_tree(&result.tree, grammar));
    for e in &result.errors {
        eprintln!(
            "parse error {} at bytes {}..{}: {}",
            e.code,
            e.start,
            e.start + e.len,
            e.message
        );
    }
    if print(&result.tree) != source.text {
        bail!("round-trip failed: the printed tree differs from the source");
    }
    trivia::check_attachment(&result.tree, |k| grammar.is_trivia(k)).map_err(anyhow::Error::msg)?;
    eprintln!(
        "round-trip ok ({} bytes), trivia attachment ok",
        source.text.len()
    );
    Ok(())
}
