//! The code of other languages inside a file, parsed with the providers of those languages.
//!
//! concept: embedded-code
//! what: Code of another language inside a file, reported by the host provider and parsed by that language.
//! tools: Prettier embedded languages, shellcheck in Dockerfile RUN
//! reads: grammar:embed

use crate::provider::{FilePattern, LanguageProvider, SourceFile};
use core_rules::EmbeddedTree;
use core_syntax::embedded::size;
use core_syntax::{Embedded, SyntaxNode, TextRange, TextSize};
use std::path::Path;

/// A region of the host file and its parse (offsets from the start of the region).
pub struct EmbeddedParse<'a> {
    pub region: Embedded,
    pub provider: &'a dyn LanguageProvider,
    pub root: SyntaxNode,
    /// The text that was parsed: the region with the host's parts blanked.
    pub code: String,
    /// The cells of a notebook that `code` joins, in order (`region` is the first's); empty for
    /// one region.
    pub cells: Vec<Cell>,
}

/// A cell in the joined code: its code and the line break after it.
pub struct Cell {
    pub start: TextSize,
    pub end: TextSize,
    pub len: TextSize,
    pub region: Embedded,
}

impl EmbeddedParse<'_> {
    /// The ranges of the cells in `code`, the line break after each included.
    pub fn cell_ranges(&self) -> Vec<TextRange> {
        self.cells
            .iter()
            .map(|c| TextRange::new(c.start, c.end))
            .collect()
    }

    /// The host offset of `at`, an offset in `code`; `end` for the end of a range.
    pub fn to_host(&self, at: TextSize, end: bool) -> TextSize {
        let Some(cell) = self.cell_at(at, end) else {
            return self.region.to_host(at, end);
        };
        cell.region.to_host((at - cell.start).min(cell.len), end)
    }

    /// The cell an offset or the end of a range belongs to.
    fn cell_at(&self, at: TextSize, end: bool) -> Option<&Cell> {
        let i = self
            .cells
            .partition_point(|c| if end { c.start < at } else { c.start <= at });
        self.cells.get(i.saturating_sub(1))
    }

    /// Does the range lie in one cell? A fix across cells is not one the host can write.
    pub fn in_one_cell(&self, range: TextRange) -> bool {
        self.cells.is_empty()
            || self
                .cell_at(range.start(), false)
                .zip(self.cell_at(range.end(), true))
                .is_some_and(|(a, b)| std::ptr::eq(a, b))
    }
}

/// A provider and the dialect its matcher selects.
pub type Resolved<'a> = Option<(&'a dyn LanguageProvider, Option<String>)>;

/// The provider `name` stands for (a language name, an extension or a file name).
pub fn resolve<'a>(
    providers: impl IntoIterator<Item = &'a dyn LanguageProvider>,
    name: &str,
) -> Resolved<'a> {
    let file = format!("x.{name}");
    // A plain extension (`ts`) is compared as it is, without parsing a path per matcher.
    let plain = !name.contains(['.', '/', '\\']);
    providers
        .into_iter()
        .filter(|p| p.embeddable())
        .find_map(|p| {
            if p.language() == name {
                return Some((p, None));
            }
            let m = (p.file_matchers().iter()).find(|m| match &m.pattern {
                FilePattern::Extension(e) if plain => e.eq_ignore_ascii_case(name),
                FilePattern::InDirectory { .. } if plain => false,
                FilePattern::Except { pattern, .. }
                    if plain
                        && matches!(&**pattern, FilePattern::Extension(e) if !e.eq_ignore_ascii_case(name)) =>
                {
                    false
                }
                FilePattern::FileName(n) if plain && !n.contains('/') => {
                    match n.strip_suffix('*') {
                        Some(prefix) => name.starts_with(prefix) || file.starts_with(prefix),
                        None => n == name || *n == file,
                    }
                }
                _ => m.matches(Path::new(&file)) || m.matches(Path::new(name)),
            })?;
            Some((p, m.dialect.clone()))
        })
}

/// The parses as the trees rules read (see `core_rules::FileContext::embedded`).
pub fn trees(parses: &[EmbeddedParse<'_>]) -> Vec<EmbeddedTree> {
    parses
        .iter()
        .map(|e| EmbeddedTree {
            range: e.region.range,
            language: e.provider.language().to_string(),
            root: e.root.clone(),
        })
        .collect()
}

/// Parses what `host` finds in `root`. A region whose language nobody handles or whose code has
/// syntax errors is left out: it stays as written. The cells of one language (`Embedded::cell`)
/// are parsed as one program, each followed by a line break; a cell that does not parse leaves
/// the program, as the others stay (the unit of an `EmbeddedParse` with `cells`).
pub fn parse_regions<'a>(
    host: &dyn LanguageProvider,
    root: &SyntaxNode,
    text: &str,
    resolve: &dyn Fn(&str) -> Resolved<'a>,
) -> Vec<EmbeddedParse<'a>> {
    let mut out = Vec::new();
    let mut cells: Vec<(&'a dyn LanguageProvider, Option<String>, Embedded)> = Vec::new();
    for region in host.embedded(root, text) {
        // No language: the host reports the text for its own rules.
        let Some((provider, dialect)) = Some(&region.language)
            .filter(|l| !l.is_empty())
            .and_then(|l| resolve(l))
        else {
            continue;
        };
        if region.cell {
            cells.push((provider, dialect, region));
            continue;
        }
        let code = region.code(text);
        if let Some(root) = parse_code(provider, &region, dialect, &code) {
            out.push(EmbeddedParse {
                region,
                provider,
                root,
                code,
                cells: Vec::new(),
            });
        }
    }
    out.extend(program(cells, text));
    out.sort_by_key(|e| e.region.range.start());
    out
}

/// The parse of `code` as code of `region`'s language, when it has no syntax error.
fn parse_code(
    provider: &dyn LanguageProvider,
    region: &Embedded,
    dialect: Option<String>,
    code: &str,
) -> Option<SyntaxNode> {
    let mut file = SourceFile::new(format!("embedded.{}", region.language), 1, code.to_string());
    // The dialect a host sets for its code (`ipython`) is read from the language version.
    file.language_version = (region.settings.iter())
        .find(|(name, _)| name == "dialect")
        .map(|(_, value)| value.clone())
        .or(dialect);
    let parsed = crate::guard::parse(provider, &file).ok()?;
    parsed.errors.is_empty().then_some(parsed.tree)
}

/// The cells of each language as one parse. All of them are tried first; when that fails, the
/// cells that parse on their own.
fn program<'a>(
    cells: Vec<(&'a dyn LanguageProvider, Option<String>, Embedded)>,
    text: &str,
) -> Vec<EmbeddedParse<'a>> {
    let mut out = Vec::new();
    let mut rest = cells;
    while let Some(first) = rest.first() {
        let language = first.2.language.clone();
        let (same, others): (Vec<_>, Vec<_>) =
            rest.into_iter().partition(|c| c.2.language == language);
        rest = others;
        let provider = same[0].0;
        let texts: Vec<String> = same.iter().map(|c| c.2.code(text)).collect();
        let mut keep: Vec<usize> = (0..same.len()).collect();
        let mut joined = join(&texts, &keep);
        let mut tree = parse_code(provider, &same[0].2, same[0].1.clone(), &joined);
        if tree.is_none() {
            keep.retain(|i| {
                parse_code(provider, &same[*i].2, same[*i].1.clone(), &texts[*i]).is_some()
            });
            joined = join(&texts, &keep);
            tree = parse_code(provider, &same[0].2, same[0].1.clone(), &joined);
        }
        let Some(root) = tree.filter(|_| !keep.is_empty()) else {
            continue;
        };
        let mut start = 0;
        let parts = keep
            .iter()
            .map(|i| {
                let len = texts[*i].len();
                let part = Cell {
                    start: size(start),
                    end: size(start + len + 1),
                    len: size(len),
                    region: same[*i].2.clone(),
                };
                start += len + 1;
                part
            })
            .collect::<Vec<_>>();
        out.push(EmbeddedParse {
            region: parts[0].region.clone(),
            provider,
            root,
            code: joined,
            cells: parts,
        });
    }
    out
}

/// The texts of the cells `keep` names, each followed by a line break.
fn join(texts: &[String], keep: &[usize]) -> String {
    keep.iter().fold(String::new(), |mut all, i| {
        all.push_str(&texts[*i]);
        all.push('\n');
        all
    })
}
