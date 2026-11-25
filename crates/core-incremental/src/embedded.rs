//! The code of other languages inside a file (see `provider_api::embedded`): linted with the
//! rules and config of its own language, and formatted by its own formatter when the host asks.
//!
//! concept: none (infrastructure: embedded regions as queries)

use crate::db::Db;
use crate::failure::{Failure, Stage};
use crate::inputs::{ExtensionSet, File};
use crate::queries::{config, source_file};
use core_config::{FileInput, OptionValue, ResolvedOptions};
use core_format::{DirectiveTable, EmbedMode, Embedder, Forward};
use core_rules::engine::ActiveRule;
use core_rules::{Diagnostic, Edit, FileContext, LineIndex, activate, run_file};
use core_syntax::{SyntaxNode, TextRange};
use provider_api::embedded::{EmbeddedParse, Resolved, parse_regions, resolve};
use provider_api::{LanguageProvider, SourceFile, format_file, resolve_dialect};
use std::cell::RefCell;
use std::rc::Rc;

/// Names the host's option that turns the formatting of embedded code off (Prettier's
/// `embeddedLanguageFormatting`: `auto` formats what it knows, `off` keeps all as written).
const FORMATTING: core_config::OptId = core_config::opt::EMBEDDED_LANGUAGE_FORMATTING;

fn lookup<'a>(db: &'a dyn Db, name: &str) -> Resolved<'a> {
    resolve(db.providers().iter().map(|p| p.as_ref()), name)
}

/// The rules that are on for each embedded language of a file, activated once per language and
/// only when a region of that language is looked at.
#[derive(Default)]
pub(crate) struct EmbeddedRules {
    by_language: RefCell<Vec<(String, Rc<Vec<ActiveRule<'static>>>)>>,
}

impl EmbeddedRules {
    fn get(
        &self,
        db: &dyn Db,
        file: File,
        provider: &dyn LanguageProvider,
    ) -> Result<Rc<Vec<ActiveRule<'static>>>, Failure> {
        let language = provider.language();
        if let Some((_, rules)) = self
            .by_language
            .borrow()
            .iter()
            .find(|(l, _)| l == language)
        {
            return Ok(rules.clone());
        }
        // Read only now: a file with no embedded code does not depend on the config here.
        let config = config(db).as_ref().map_err(Clone::clone)?;
        let extensions = ExtensionSet::get(db).extensions(db);
        let input = FileInput::language(language)
            .with_base(provider.config_base())
            .with_path(file.path(db));
        let rules = (config.rules(&input, provider.option_schema()))
            .map_err(|e| Failure::new(Stage::Config, &e))?;
        let mut rules = activate(
            extensions.with_own(provider.rules(), language),
            &rules,
            |_| false,
        )
        .map_err(|e| Failure::new(Stage::Config, &e))?;
        rules.retain(|a| a.rule.layout_role().reports());
        let rules = Rc::new(rules);
        self.by_language
            .borrow_mut()
            .push((language.to_string(), rules.clone()));
        Ok(rules)
    }

    fn any(&self, db: &dyn Db, file: File, provider: &dyn LanguageProvider) -> bool {
        !matches!(self.get(db, file, provider), Ok(r) if r.is_empty())
    }
}

/// The parses of the code of other languages in the file. A region is parsed when the host has a
/// rule that reads it (`host_reads`) or a rule of its own language is on; otherwise nothing could
/// look at the tree.
pub(crate) fn parse_embedded<'a>(
    db: &'a dyn Db,
    file: File,
    host: &dyn LanguageProvider,
    host_reads: bool,
    rules: &EmbeddedRules,
    root: &SyntaxNode,
    text: &str,
) -> Vec<EmbeddedParse<'a>> {
    parse_regions(host, root, text, &|name| {
        lookup(db, name).filter(|(p, _)| host_reads || rules.any(db, file, *p))
    })
}

/// What the rules of each embedded language report, in host offsets. Suppression comments inside
/// the code apply here; the host's apply to the result.
pub(crate) fn lint_embedded(
    db: &dyn Db,
    file: File,
    rules: &EmbeddedRules,
    parses: &[EmbeddedParse<'_>],
) -> Result<Vec<Diagnostic>, Failure> {
    let mut out = Vec::new();
    for e in parses.iter().filter(|e| e.region.lint) {
        let rules = rules.get(db, file, e.provider)?;
        if rules.is_empty() {
            continue;
        }
        let rules = &rules[..];
        let lines = LineIndex::new(&e.code);
        let context = FileContext {
            path: file.path(db),
            text: &e.code,
            root: &e.root,
            grammar: e.provider.grammar(),
            lines: &lines,
            body_kinds: e.provider.body_kinds(),
            hooks: e.provider.rule_hooks(),
            embedded: &[],
            semantics: None,
        };
        let host = |r: TextRange| {
            TextRange::new(
                e.region.to_host(r.start(), false),
                e.region.to_host(r.end(), true),
            )
        };
        for mut d in run_file(&context, rules, false).diagnostics {
            d.range = host(d.range);
            d.related.iter_mut().for_each(|r| r.range = host(r.range));
            for fix in &mut d.fixes {
                fix.edits.iter_mut().for_each(|x| x.range = host(x.range));
            }
            d.fixes
                .retain(|f| f.edits.iter().all(|x| keeps_host(&e.region, x)));
            out.push(d);
        }
    }
    Ok(out)
}

/// Does the edit leave the host's parts of the region alone? A line break could take the
/// container prefix of the next line with it.
fn keeps_host(region: &core_syntax::Embedded, edit: &Edit) -> bool {
    region.carved.is_empty()
        || (!edit.text.contains('\n')
            && region
                .carved
                .iter()
                .all(|c| !(c.start() < edit.range.end() && edit.range.start() < c.end())))
}

/// `formatted`, the host's output, with the code of its embedded regions formatted by their own
/// languages and indented as the region is. Code that does not parse or whose language nobody
/// handles stays as written; so does everything when the result would not read back the same.
pub(crate) fn format_embedded(
    db: &dyn Db,
    file: File,
    host: &dyn LanguageProvider,
    options: &ResolvedOptions,
    input_root: &SyntaxNode,
    formatted: String,
) -> String {
    let off = matches!(options.get(FORMATTING), Some(OptionValue::String(s)) if s == "off");
    let regions = |root: &SyntaxNode, text: &str| {
        let mut all = host.embedded(root, text);
        all.retain(|r| r.format);
        all
    };
    if off || !host.formats_embedded() || regions(input_root, file.text(db)).is_empty() {
        return formatted;
    }
    let source = SourceFile {
        text: formatted.clone(),
        ..source_file(db, file, None)
    };
    let Ok(parsed) = provider_api::guard::parse(host, &source) else {
        return formatted;
    };
    let regions = regions(&parsed.tree, &formatted);
    let crlf = formatted.contains("\r\n");
    let mut out = formatted.clone();
    // From the last region on, so that the offsets of the others stay valid.
    for r in regions.iter().rev() {
        let Some(code) = format_code(db, file, &r.language, &r.stripped(&formatted)) else {
            continue;
        };
        let prefix = prefix_of(r, &formatted);
        let lines: Vec<String> = (code.lines())
            .map(|l| {
                if l.is_empty() {
                    prefix.trim_end().into()
                } else {
                    format!("{prefix}{l}")
                }
            })
            .collect();
        out.replace_range(
            std::ops::Range::<usize>::from(r.range),
            &lines.join(if crlf { "\r\n" } else { "\n" }),
        );
    }
    // The fences must still be there, with the code just written.
    let again = SourceFile {
        text: out.clone(),
        ..source
    };
    match provider_api::guard::parse(host, &again) {
        Ok(p) if p.errors.is_empty() && regions_agree(host, &p.tree, &out, &regions) => out,
        _ => formatted,
    }
}

/// The host's part of the first line of the region that has code on it: what every line of
/// rewritten code starts with.
fn prefix_of<'a>(region: &core_syntax::Embedded, text: &'a str) -> &'a str {
    let mut at = usize::from(region.range.start());
    for line in text[region.range].split_inclusive('\n') {
        let carved = region.carved.iter().find(|c| usize::from(c.start()) == at);
        let len = carved.map_or(0, |c| usize::from(c.len()));
        if !line[len..].trim().is_empty() {
            return &text[at..at + len];
        }
        at += line.len();
    }
    ""
}

fn regions_agree(
    host: &dyn LanguageProvider,
    root: &SyntaxNode,
    text: &str,
    before: &[core_syntax::Embedded],
) -> bool {
    let after = host.embedded(root, text);
    let after: Vec<_> = after.iter().filter(|r| r.format).collect();
    after.len() == before.len()
        && after
            .iter()
            .zip(before)
            .all(|(a, b)| a.language == b.language)
}

/// The formatted code, if its language is known and it parses; `None` also when nothing changes.
fn format_code(db: &dyn Db, file: File, language: &str, code: &str) -> Option<String> {
    let code = code.replace("\r\n", "\n");
    let prepared = prepare(db, file, language, EmbedMode::Program, None, false);
    let out = format_with(db, language, &prepared, &code)?;
    (!out.is_empty() && out != code.trim_end_matches('\n')).then_some(out)
}

/// The options an embedded `language` is formatted with and its language version.
type Prepared = Option<Rc<(ResolvedOptions, Option<String>)>>;

fn prepare(
    db: &dyn Db,
    file: File,
    language: &str,
    mode: EmbedMode,
    width: Option<usize>,
    single_quotes: bool,
) -> Prepared {
    let (provider, dialect) = lookup(db, language)?;
    let input = FileInput::language(provider.language())
        .with_base(provider.config_base())
        .with_path(file.path(db));
    let mut options = (config(db).as_ref().ok()?)
        .format_options(&input, provider.option_schema())
        .ok()?
        .options;
    let language_version = resolve_dialect(&mut options, dialect);
    if let Some(w) = width {
        options.set(
            core_config::opt::LINE_WIDTH,
            OptionValue::Int(w.max(20) as i64),
        );
    }
    if single_quotes {
        if options.get(core_config::opt::QUOTE_STYLE).is_some() {
            options.set(
                core_config::opt::QUOTE_STYLE,
                OptionValue::String("single".into()),
            );
        }
        if options.get(core_config::opt::SINGLE_QUOTE).is_some() {
            options.set(core_config::opt::SINGLE_QUOTE, OptionValue::Bool(true));
        }
    }
    let version = if mode == EmbedMode::Program {
        language_version
    } else {
        // The provider's parse and layout read the mode: the code is not a whole file.
        options.set(
            core_config::opt::EMBED_MODE,
            OptionValue::String(mode.name().into()),
        );
        Some(mode.name().into())
    };
    Some(Rc::new((options, version)))
}

/// `code` formatted by the provider of `language` with prepared options, without the final
/// line break.
fn format_with(db: &dyn Db, language: &str, prepared: &Prepared, code: &str) -> Option<String> {
    let (provider, _) = lookup(db, language)?;
    let (options, version) = &**prepared.as_ref()?;
    let mut source = SourceFile::new(format!("embedded.{language}"), 1, code.to_string());
    source.language_version = version.clone();
    let out = format_file(provider, &source, options, &DirectiveTable::default()).ok()?;
    Some(out.trim_end_matches('\n').to_string())
}

/// The options of one embedded language, prepared for a mode and size and the host options
/// forwarded into them.
struct PreparedFor {
    language: String,
    mode: EmbedMode,
    size: (usize, bool),
    forwarded: Vec<(core_config::OptId, OptionValue)>,
    options: Prepared,
}

/// The embedder a host formatter runs under: other languages' code, formatted by their
/// providers with the config of the file.
pub(crate) struct Engine<'a> {
    db: &'a dyn Db,
    file: File,
    /// Options per language, mode, size and forwarded host options: resolved once per file, not
    /// per snippet.
    prepared: RefCell<Vec<PreparedFor>>,
}

impl<'a> Engine<'a> {
    pub(crate) fn new(db: &'a dyn Db, file: File) -> Self {
        Engine {
            db,
            file,
            prepared: RefCell::default(),
        }
    }

    /// The prepared options of `language`: found by comparing, so that a lookup allocates nothing.
    fn prepared_for(
        &self,
        language: &str,
        mode: EmbedMode,
        size: (usize, bool),
        forward: Forward<'_>,
    ) -> Prepared {
        let values = || {
            forward
                .ids
                .iter()
                .filter_map(|id| Some((*id, forward.host.get(*id)?)))
        };
        let mut all = self.prepared.borrow_mut();
        let found = all.iter().find(|p| {
            p.language == language
                && p.mode == mode
                && p.size == size
                && p.forwarded.len() == values().count()
                && p.forwarded
                    .iter()
                    .zip(values())
                    .all(|((id, v), (id2, v2))| *id == id2 && v == v2)
        });
        if let Some(p) = found {
            return p.options.clone();
        }
        let forwarded: Vec<_> = values().map(|(id, v)| (id, v.clone())).collect();
        let options = prepare(self.db, self.file, language, mode, Some(size.0), size.1).map(|p| {
            if forwarded.is_empty() {
                return p;
            }
            let (mut options, version) = (*p).clone();
            for (id, value) in &forwarded {
                options.set(*id, value.clone());
            }
            Rc::new((options, version))
        });
        all.push(PreparedFor {
            language: language.to_string(),
            mode,
            size,
            forwarded,
            options: options.clone(),
        });
        options
    }
}

impl Embedder for Engine<'_> {
    fn format(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        size: (usize, bool),
        forward: Forward<'_>,
    ) -> Option<String> {
        let prepared = self.prepared_for(language, mode, size, forward);
        format_with(self.db, language, &prepared, code)
    }

    fn same_code(
        &self,
        language: &str,
        mode: EmbedMode,
        (before, after): (&str, &str),
        size: (usize, bool),
        forward: Forward<'_>,
    ) -> Result<(), String> {
        let prepared = self.prepared_for(language, mode, size, forward);
        let (provider, _) = lookup(self.db, language).ok_or("the language is unknown")?;
        let (options, version) = &**prepared.as_ref().ok_or("the language has no options")?;
        let parse = |text: &str| {
            let mut source = SourceFile::new(format!("embedded.{language}"), 1, text.to_string());
            source.language_version = version.clone();
            let parsed = provider.parse(&source).map_err(|e| e.to_string())?;
            match parsed.errors.first() {
                Some(e) => Err(format!("`{language}` text does not parse: {}", e.message)),
                None => Ok(parsed.tree),
            }
        };
        let tree = parse(before)?;
        core_format::safety::verify(
            provider.grammar(),
            provider.formatter(),
            options,
            &tree,
            after,
            parse,
        )
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::TextSize;

    #[test]
    fn host_parts_are_kept() {
        let region = core_syntax::Embedded {
            language: "x".into(),
            range: core_syntax::TextRange::new(TextSize::from(0), TextSize::from(6)),
            carved: vec![core_syntax::TextRange::new(
                TextSize::from(0),
                TextSize::from(2),
            )],
            format: true,
            strip: false,
            lint: true,
        };
        let edit = |a: u32, b: u32, t: &str| {
            Edit::replace(core_syntax::TextRange::new(a.into(), b.into()), t)
        };
        assert!(keeps_host(&region, &edit(2, 4, "z")));
        assert!(!keeps_host(&region, &edit(1, 4, "z")));
        assert!(!keeps_host(&region, &edit(2, 4, "\n")));
    }
}
