//! Formatting a file through a provider: parse, format (and, in debug builds, the safety check).
//!
//! concept: none (infrastructure: formatting a file through a provider)

use crate::provider::{LanguageProvider, ParseError, ParseResult, ProviderError, SourceFile};
use core_config::ResolvedOptions;
use core_format::{
    DirectiveTable, Embedder, FormatCtx, FormatError, Variants, format_tree, safety,
};
use core_rules::options::RuleOptions;
use core_rules::rule::FormatInput;
use core_rules::{ActiveRule, FileContext, Severity};
use core_syntax::SyntaxNode;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum FileFormatError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// Files with syntax errors are not formatted.
    #[error("{line}:{column}: {message}{}", more(*.count))]
    Syntax {
        line: usize,
        column: usize,
        message: String,
        count: usize,
    },
    #[error(transparent)]
    Format(#[from] FormatError),
}

fn more(count: usize) -> String {
    match count {
        0 | 1 => String::new(),
        n => format!(" (and {} more)", n - 1),
    }
}

/// 1-based line and column (in characters) of a byte offset.
fn position(text: &str, offset: u32) -> (usize, usize) {
    core_rules::LineIndex::new(text).line_col(text, offset.into())
}

fn syntax_error(text: &str, errors: &[ParseError]) -> FileFormatError {
    let first = &errors[0];
    let (line, column) = position(text, first.start);
    FileFormatError::Syntax {
        line,
        column,
        message: first.message.clone(),
        count: errors.len(),
    }
}

/// Formats `file` with the provider. A file with syntax errors is an error and is left alone;
/// debug builds (every test) also run the safety check on the output.
pub fn format_file(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
) -> Result<String, FileFormatError> {
    let parsed = crate::guard::parse(provider, file)?;
    format_parsed(provider, file, &parsed, options, directives)
}

/// [`format_file`] for a tree that is already parsed (the incremental engine memoizes parsing).
/// `parsed` must be the result of parsing `file`.
pub fn format_parsed(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    parsed: &ParseResult,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
) -> Result<String, FileFormatError> {
    format_parsed_with(provider, file, parsed, options, directives, None)
}

/// [`format_parsed`] for a host whose formatter embeds code of other languages through
/// `embedder`.
pub fn format_parsed_with(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    parsed: &ParseResult,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
    embedder: Option<&dyn Embedder>,
) -> Result<String, FileFormatError> {
    if !parsed.errors.is_empty() {
        return Err(syntax_error(&file.text, &parsed.errors));
    }
    let rewritten = rewrite_loop(provider, file, parsed, options)?;
    let out = match &rewritten {
        Some((again, reparsed)) => {
            format_checked(provider, again, reparsed, options, directives, embedder)?
        }
        None => format_checked(provider, file, parsed, options, directives, embedder)?,
    };
    // A rewrite decided on the source (a width) can look different after the layout moved the
    // code: when the output has a wrapped statement of the `settle_keywords` (a parse and a scan
    // for these files only), it gets one more rewrite round, and a layout when that rewrote
    // something.
    let done = rewritten.as_ref().map_or(file, |(done, _)| done);
    let keywords = options.strings("settle_keywords");
    if !keywords.is_empty() && core_format::rewrite::has_wrapped_statement(&out, &keywords) {
        let settled = SourceFile {
            version: done.version + 1,
            text: out.clone(),
            ..done.clone()
        };
        if let Ok(reparsed) = provider.parse(&settled)
            && reparsed.errors.is_empty()
            && let Some((again, reparsed)) = rewrite_loop(provider, &settled, &reparsed, options)?
        {
            return format_checked(provider, &again, &reparsed, options, directives, embedder);
        }
    }
    Ok(out)
}

/// The rewrite rounds on a parsed file: the file and its tree after the rewrites, `None` when
/// nothing was rewritten.
fn rewrite_loop(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    parsed: &ParseResult,
    options: &ResolvedOptions,
) -> Result<Option<(SourceFile, ParseResult)>, FileFormatError> {
    let grammar = provider.grammar();
    let listed = options.strings("format_rules");
    let mut current: Option<(SourceFile, ParseResult)> = None;
    // The rules that rewrite the source take a round of their own: the language's rewrite follows.
    let rounds = provider.formatter().rewrite_rounds()
        + usize::from(!format_rules(provider.formatter().rows(), &listed).is_empty());
    for _ in 0..rounds {
        let (text, tree) = match &current {
            Some((f, p)) => (&f.text, &p.tree),
            None => (&file.text, &parsed.tree),
        };
        // One walk of the nodes serves the rules' kinds and the rows' scan; a rule whose cheap
        // look at the text finds nothing to do takes no part in it.
        let rules = format_rules(provider.formatter().rows(), &listed);
        let rules: Vec<&ActiveRule<'static>> = rules
            .iter()
            .filter(|r| r.rule.format_applies(grammar, text, &r.options, options))
            .collect();
        let wanted: Vec<_> = rules
            .iter()
            .flat_map(|r| r.rule.format_kinds(grammar))
            .collect();
        let mut scan = provider.formatter().rewrite_rows().map(|r| r.scan());
        let mut index: Vec<Vec<SyntaxNode>> = Vec::new();
        if scan.is_some() || !wanted.is_empty() {
            let mut w = core_syntax::tree::Wanted::default();
            for k in &wanted {
                let at = k.index() as usize;
                if w.nodes.len() <= at {
                    w.nodes.resize(at + 1, false);
                }
                w.nodes[at] = true;
            }
            let rule_nodes = w.nodes.clone();
            if let Some(r) = provider.formatter().rewrite_rows() {
                r.wanted(&mut w);
            }
            let (nodes, tokens) = core_syntax::tree::find(tree, &w);
            if let Some(s) = &mut scan {
                tokens.into_iter().for_each(|t| s.add_token(t));
            }
            for n in nodes {
                if let Some(s) = &mut scan {
                    s.visit(&n);
                }
                let at = n.kind().index() as usize;
                if rule_nodes.get(at).copied().unwrap_or(false) {
                    if index.len() <= at {
                        index.resize_with(at + 1, Vec::new);
                    }
                    index[at].push(n);
                }
            }
        }
        let Some(next) = rule_rewrite(provider, &file.id, tree, text, options, &rules, index)
            .or_else(|| {
                provider
                    .formatter()
                    .rewrite_scanned(tree, text, grammar, options, scan)
            })
            .filter(|t| t != text)
        else {
            break;
        };
        let prev_version = current.as_ref().map_or(file.version, |(f, _)| f.version);
        let again = SourceFile {
            id: file.id.clone(),
            version: prev_version + 1,
            text: next,
            had_bom: file.had_bom,
            language_version: file.language_version.clone(),
            preprocessor_symbols: file.preprocessor_symbols.clone(),
        };
        let reparsed = provider.parse(&again)?;
        if let Some(e) = reparsed.errors.first() {
            return Err(FormatError::Safety(format!(
                "the rewritten source does not parse: {}",
                e.message
            ))
            .into());
        }
        current = Some((again, reparsed));
    }
    Ok(current)
}

/// The shared rules that rewrite the source that the language has rows for, found once per
/// language (the rows' text is static).
fn format_rules(rows: &'static str, listed: &[String]) -> std::sync::Arc<Vec<ActiveRule<'static>>> {
    type Store = std::sync::Mutex<
        std::collections::HashMap<(usize, usize, String), std::sync::Arc<Vec<ActiveRule<'static>>>>,
    >;
    static STORE: std::sync::OnceLock<Store> = std::sync::OnceLock::new();
    let mut store = STORE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    store
        .entry((rows.as_ptr() as usize, rows.len(), listed.join(",")))
        .or_insert_with(|| {
            let active = core_rules::shared::RULES
                .iter()
                .copied()
                .filter(|r| {
                    r.formats()
                        && (rows.contains(&format!("[{}]", r.meta().id))
                            || listed.iter().any(|l| l == r.meta().id))
                })
                .filter_map(|rule| {
                    let options =
                        RuleOptions::resolve(rule.meta().options, &Default::default()).ok()?;
                    Some(ActiveRule {
                        rule,
                        severity: Severity::Warning,
                        options,
                    })
                })
                .collect();
            std::sync::Arc::new(active)
        })
        .clone()
}

/// The source with the fixes of the shared rules that rewrite it (`core_rules::Rule::formats`)
/// applied, for the ones the language has rows for or lists in `format_rules`; `None` when none of them has anything to do.
fn rule_rewrite(
    provider: &dyn LanguageProvider,
    id: &str,
    tree: &SyntaxNode,
    text: &str,
    options: &ResolvedOptions,
    active: &[&ActiveRule<'static>],
    index: Vec<Vec<SyntaxNode>>,
) -> Option<String> {
    if active.is_empty() {
        return None;
    }
    let lines = core_rules::LineIndex::new(text);
    let context = FileContext {
        path: Path::new(id),
        text,
        root: tree,
        grammar: provider.grammar(),
        lines: &lines,
        body_kinds: provider.body_kinds(),
        hooks: provider.rule_hooks(),
        embedded: &[],
        semantics: None,
    };
    let diagnostics =
        core_rules::run_file_rules(&context, active, FormatInput { options }, Some(index));
    let chosen = core_rules::fixes::select(&diagnostics, |_| true);
    if chosen.is_empty() {
        return None;
    }
    Some(core_rules::fixes::apply(text, &chosen))
}

/// Formats a tree that has no syntax errors and checks the output against it. Code that the
/// parse leaves out (inactive `#if` branches) is formatted from other parses when the provider
/// asks for that (see [`core_format::NodeFormatter::merge_variants`]); every parse it used checks the result.
fn format_checked(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    parsed: &ParseResult,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
    embedder: Option<&dyn Embedder>,
) -> Result<String, FileFormatError> {
    let grammar = provider.grammar();
    let cx =
        FormatCtx::new(grammar, options, directives, provider.formatter()).with_embedder(embedder);
    let output = format_tree(&parsed.tree, &cx)?;
    let check = |symbols: &[String], tree: &SyntaxNode, text: &str| {
        let reparse = |text: &str| {
            // Not `..file.clone()`: that would copy the whole source text just to replace it.
            let again = SourceFile {
                id: file.id.clone(),
                version: file.version + 1,
                text: text.to_string(),
                had_bom: file.had_bom,
                language_version: file.language_version.clone(),
                preprocessor_symbols: symbols.to_vec(),
            };
            let result = provider.parse(&again).map_err(|e| e.to_string())?;
            match result.errors.first() {
                Some(e) => Err(format!("{}: {}", position_str(text, e), e.message)),
                None => Ok(result.tree),
            }
        };
        safety::verify(grammar, provider.formatter(), options, tree, text, reparse)
    };
    let mut variants = Reparses {
        provider,
        file,
        options,
        directives,
        parses: vec![(file.preprocessor_symbols.clone(), Some(parsed.tree.clone()))],
    };
    let merged = provider
        .formatter()
        .merge_variants(&parsed.tree, &output, options, &mut variants)
        .filter(|(text, _)| *text != output);
    if let Some((text, sets)) = merged {
        let trees: Option<Vec<_>> = sets.iter().map(|s| variants.parse(s)).collect();
        let sound = trees.is_some_and(|trees| {
            check(&file.preprocessor_symbols, &parsed.tree, &text).is_ok()
                && sets
                    .iter()
                    .zip(&trees)
                    .all(|(s, t)| check(s, t, &text).is_ok())
        });
        // Any doubt: the inactive branches stay as they were.
        if sound {
            return Ok(text);
        }
    }
    // Release builds trust the formatter; tests and the corpus runs keep checking it.
    if cfg!(debug_assertions) {
        check(&file.preprocessor_symbols, &parsed.tree, &output)?;
    }
    Ok(output)
}

/// The parses of one file under other symbol sets, memoized (see [`Variants`]).
struct Reparses<'a> {
    provider: &'a dyn LanguageProvider,
    file: &'a SourceFile,
    options: &'a ResolvedOptions,
    directives: &'a DirectiveTable,
    parses: Vec<(Vec<String>, Option<SyntaxNode>)>,
}

impl Variants for Reparses<'_> {
    fn parse(&mut self, symbols: &[String]) -> Option<SyntaxNode> {
        if let Some((_, tree)) = self.parses.iter().find(|(s, _)| s == symbols) {
            return tree.clone();
        }
        let again = SourceFile {
            id: self.file.id.clone(),
            version: self.file.version + 1,
            text: self.file.text.clone(),
            had_bom: self.file.had_bom,
            language_version: self.file.language_version.clone(),
            preprocessor_symbols: symbols.to_vec(),
        };
        let tree = self
            .provider
            .parse(&again)
            .ok()
            .filter(|r| r.errors.is_empty())
            .map(|r| r.tree);
        self.parses.push((symbols.to_vec(), tree.clone()));
        tree
    }

    fn format(&mut self, tree: &SyntaxNode) -> Option<String> {
        let cx = FormatCtx::new(
            self.provider.grammar(),
            self.options,
            self.directives,
            self.provider.formatter(),
        );
        format_tree(tree, &cx).ok()
    }
}

fn position_str(text: &str, e: &ParseError) -> String {
    let (line, column) = position(text, e.start);
    format!("{line}:{column}")
}

/// The provider responsible for `path` and the dialect its matcher selects. The most specific
/// matcher wins (file name, then directory, then extension).
pub fn select_provider<'a>(
    providers: &'a [Box<dyn LanguageProvider>],
    path: &Path,
) -> Option<(&'a dyn LanguageProvider, Option<String>)> {
    select_provider_index(providers, path)
        .map(|(index, dialect)| (providers[index].as_ref(), dialect))
}

/// [`select_provider`], naming the provider by its position in `providers`.
pub fn select_provider_index(
    providers: &[Box<dyn LanguageProvider>],
    path: &Path,
) -> Option<(usize, Option<String>)> {
    providers
        .iter()
        .enumerate()
        .flat_map(|(i, p)| p.file_matchers().iter().map(move |m| (i, m)))
        .filter(|(_, m)| m.matches(path))
        .max_by_key(|(_, m)| m.specificity())
        .map(|(i, m)| (i, m.dialect.clone()))
}

/// Fixes the dialect of a file for parsing and formatting. A provider whose schema has a
/// `dialect` option gets the option's value, or the matcher's dialect when it is `auto`; the
/// effective dialect is written back to `options` so formatting code can read it. Returns the
/// value for [`SourceFile::language_version`]; a `target_version` option (Python) is that value.
pub fn resolve_dialect(
    options: &mut ResolvedOptions,
    matcher_dialect: Option<String>,
) -> Option<String> {
    if let Some(core_config::OptionValue::String(v)) = options.get("target_version") {
        return Some(v.clone());
    }
    let chosen = match options.get("dialect")? {
        core_config::OptionValue::String(s) if s != "auto" => Some(s.clone()),
        _ => matcher_dialect,
    }?;
    options.set("dialect", core_config::OptionValue::String(chosen.clone()));
    Some(chosen)
}
