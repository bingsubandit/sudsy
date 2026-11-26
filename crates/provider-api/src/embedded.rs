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
    /// The regions of a joined program (`Embedded::joined`: a notebook's cells), each with its
    /// range in `code`; `region` is then the first with the range of all. Empty for one region.
    pub cells: Vec<(TextRange, Embedded)>,
}

impl EmbeddedParse<'_> {
    /// The host offset of `at`, an offset in `code`; `end` for the end of a range.
    pub fn to_host(&self, at: TextSize, end: bool) -> TextSize {
        match self.cell_at(at, end) {
            Some((range, region)) => region.to_host(at.min(range.end()) - range.start(), end),
            None => self.region.to_host(at, end),
        }
    }

    /// The cell that holds `at` (an end offset belongs to the cell it ends).
    fn cell_at(&self, at: TextSize, end: bool) -> Option<&(TextRange, Embedded)> {
        let i =
            (self.cells).partition_point(|(r, _)| if end { r.end() < at } else { r.end() <= at });
        self.cells.get(i.min(self.cells.len().saturating_sub(1)))
    }

    /// Whether a code range lies in one cell (an edit across cells cannot be written back).
    pub fn in_one_cell(&self, range: TextRange) -> bool {
        self.cells.is_empty()
            || self
                .cell_at(range.start(), false)
                .is_some_and(|(r, _)| range.end() <= r.end())
    }

    /// The ranges of the cells in `code` (empty for one region).
    pub fn cell_ranges(&self) -> Vec<TextRange> {
        self.cells.iter().map(|(r, _)| *r).collect()
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
/// syntax errors is left out: it stays as written. Joined regions of one language are parsed as
/// one program (`join`).
pub fn parse_regions<'a>(
    host: &dyn LanguageProvider,
    root: &SyntaxNode,
    text: &str,
    resolve: &dyn Fn(&str) -> Resolved<'a>,
) -> Vec<EmbeddedParse<'a>> {
    let mut out = Vec::new();
    let mut joined: Vec<(&'a dyn LanguageProvider, Option<String>, Vec<Embedded>)> = Vec::new();
    for region in host.embedded(root, text) {
        // No language: the host reports the text for its own rules.
        let Some((provider, dialect)) = Some(&region.language)
            .filter(|l| !l.is_empty())
            .and_then(|l| resolve(l))
        else {
            continue;
        };
        let dialect = set_dialect(&region).or(dialect);
        if region.joined {
            match joined
                .iter_mut()
                .find(|(p, ..)| p.language() == provider.language())
            {
                Some((.., cells)) => cells.push(region),
                None => joined.push((provider, dialect, vec![region])),
            }
            continue;
        }
        let code = region.code(text);
        if let Some(root) = parse_clean(provider, &region.language, dialect, &code) {
            out.push(EmbeddedParse {
                region,
                provider,
                root,
                code,
                cells: Vec::new(),
            });
        }
    }
    for (provider, dialect, cells) in joined {
        out.extend(join(provider, dialect, cells, text));
    }
    out
}

/// The dialect a host sets for its code (`dialect: ipython` of a notebook's cells).
fn set_dialect(region: &Embedded) -> Option<String> {
    (region.settings.iter())
        .find(|(k, _)| k == "dialect")
        .map(|(_, v)| v.clone())
}

/// The tree of `code` when it parses without errors.
fn parse_clean(
    provider: &dyn LanguageProvider,
    language: &str,
    dialect: Option<String>,
    code: &str,
) -> Option<SyntaxNode> {
    let mut file = SourceFile::new(format!("embedded.{language}"), 1, code.to_string());
    file.language_version = dialect;
    crate::guard::parse(provider, &file)
        .ok()
        .filter(|p| p.errors.is_empty())
        .map(|p| p.tree)
}

/// The cells of one program as one parse: their code in order, each ending with a line break
/// (as a notebook kernel runs them). A cell with a syntax error is left out, the others are
/// parsed again without it, so one broken cell does not hide the rest.
fn join<'a>(
    provider: &'a dyn LanguageProvider,
    dialect: Option<String>,
    mut cells: Vec<Embedded>,
    text: &str,
) -> Option<EmbeddedParse<'a>> {
    let mut codes: Vec<String> = cells.iter().map(|c| c.code(text)).collect();
    loop {
        if cells.is_empty() {
            return None;
        }
        let (mut code, mut ranges) = (String::new(), Vec::with_capacity(cells.len()));
        for c in &codes {
            let start = size(code.len());
            code.push_str(c);
            if !code.ends_with('\n') {
                code.push('\n');
            }
            ranges.push(TextRange::new(start, size(code.len())));
        }
        let mut file = SourceFile::new(format!("embedded.{}", cells[0].language), 1, code.clone());
        file.language_version = dialect.clone();
        let parsed = crate::guard::parse(provider, &file).ok()?;
        if parsed.errors.is_empty() {
            let mut region = cells[0].clone();
            region.range = region.range.cover(cells[cells.len() - 1].range);
            return Some(EmbeddedParse {
                region,
                provider,
                root: parsed.tree,
                code,
                cells: ranges.into_iter().zip(cells).collect(),
            });
        }
        // The cells that do not parse on their own (an unclosed bracket would take the next
        // cells with it); else the cells where an error starts (one at the end belongs to the
        // last cell).
        let parses = |c: &String| {
            let mut file = SourceFile::new(file.id.clone(), 1, c.clone());
            file.language_version = dialect.clone();
            crate::guard::parse(provider, &file).is_ok_and(|p| p.errors.is_empty())
        };
        let mut broken: Vec<usize> = (0..codes.len()).filter(|&i| !parses(&codes[i])).collect();
        if broken.is_empty() {
            broken = (parsed.errors.iter())
                .map(|e| {
                    let at = TextSize::from(e.start);
                    ranges
                        .partition_point(|r| r.end() <= at)
                        .min(ranges.len() - 1)
                })
                .collect();
        }
        let mut i = 0;
        cells.retain(|_| {
            i += 1;
            !broken.contains(&(i - 1))
        });
        let mut i = 0;
        codes.retain(|_| {
            i += 1;
            !broken.contains(&(i - 1))
        });
    }
}
