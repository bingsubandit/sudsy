//! The README "Language support" table, generated from the target numbers of the parity runs
//! (docs/ai/design/parity-target.md, Phase 6). The rows are `support.yaml` (language to reference tools);
//! the numbers are the ratchets of `tests/parity/<tool>/differences.yaml`: format = `ratchet` (identical
//! files) over `target-files` (files of the target) summed over the tool's groups, lint = `target`. A
//! language Sudsy formats with no reference formatter has `stability` groups instead: the share of its
//! corpus files that `scripts/parity.sh stability <group>` finds stable, in `tests/parity/stability`.

use crate::newlang::{V, demand};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// The CLAUDE.md usability bar, in percent.
const BAR: f64 = 85.0;

/// The folder of `tests/parity` that holds the stability ratchets.
const STABILITY: &str = "stability";

/// The ending of a cell with no reference tool.
const NO_TOOL: &str = "no reference tool";

/// `Ready` bars of a new language's leg with no reference tool (docs/ai/development/no-reference-tool.md), percent:
/// format stable and agreement, lint checklist (the usability bar) and precision.
const STABLE_BAR: f64 = 100.0;
const AGREEMENT_BAR: f64 = 80.0;
const PRECISION_BAR: f64 = 90.0;

#[derive(Deserialize)]
pub(crate) struct Row {
    pub language: String,
    #[serde(default)]
    pub format: Vec<Tool>,
    #[serde(default)]
    pub lint: Vec<Tool>,
    /// The language ids of the row (`provider.language()`); the first is the page slug. Their rules
    /// count for a row with no lint reference tool.
    #[serde(default)]
    pub ids: Vec<String>,
    /// The `language` names of the rows of `bench/speed.json` that belong to this row.
    #[serde(default)]
    pub speed: Vec<String>,
    /// Sudsy formats the language although no reference formatter was found: the ratchet groups of
    /// `tests/parity/stability` (the corpus files that are stable) that stand for the format leg.
    #[serde(default)]
    pub stability: Vec<String>,
    /// The leg with no reference tool predates docs/ai/development/no-reference-tool.md: it needs no agreement,
    /// checklist or precision number (the five legs INI, EditorConfig, .gitignore, Dockerfile and GitHub
    /// Actions format; set by name here, never for a new language).
    #[serde(default)]
    pub exempt: bool,
    /// A new language's leg with no reference tool: the folder of `tests/parity` whose `differences.yaml` holds
    /// `agreement` (format, needed beside `stability`) and `checklist` with `precision` (lint, both or none) in percent.
    pub evidence: Option<String>,
    /// What a user needs before the language works, in one sentence (shown on the site's language page).
    pub requires: Option<String>,
    /// The raw popularity values of the languages of the row (the sources and the formula of `languages.yaml`);
    /// the row takes its most popular member. Every row has one (`check_measured`).
    #[serde(default)]
    pub popularity: Vec<Member>,
    /// The heading of the section of docs/languages.md that holds the row, when it is not the row's name.
    pub section: Option<String>,
}

/// A language (or file format) of a row with the raw values of the two popularity sources; a missing value means
/// the source does not list it (`newlang::demand` estimates it).
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct Member {
    pub name: String,
    pub gh: Option<f64>,
    pub so: Option<f64>,
    /// A popularity set by hand where no source lists the language: the value and its basis. Always an estimate.
    pub estimate: Option<Estimate>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct Estimate {
    pub value: f64,
    pub basis: String,
}

impl Member {
    fn demand(&self) -> V {
        match &self.estimate {
            Some(e) => V {
                v: e.value,
                est: true,
            },
            None => demand(self.gh, self.so),
        }
    }
}

impl Row {
    /// The heading of the row's section in docs/languages.md.
    pub fn section(&self) -> &str {
        self.section.as_deref().unwrap_or(&self.language)
    }

    /// The most popular member of the row and its popularity 0.1-1 (an estimate when a source lacks it).
    pub fn most_popular(&self) -> (&str, V) {
        self.popularity
            .iter()
            .map(|m| (m.name.as_str(), m.demand()))
            .reduce(|a, b| if b.1.v > a.1.v { b } else { a })
            .unwrap_or(("", demand(None, None)))
    }

    /// The popularity by the sources and the formula alone, never an estimate set by hand (the new-language scores).
    pub fn source_popularity(&self) -> V {
        self.popularity
            .iter()
            .map(|m| demand(m.gh, m.so))
            .reduce(|a, b| if b.v > a.v { b } else { a })
            .unwrap_or_else(|| demand(None, None))
    }

    /// Popularity 0.1-1 of the row.
    pub fn popularity(&self) -> V {
        self.most_popular().1
    }
}

/// A reference tool: its `tests/parity` folder, its name in the table and, for a folder that serves
/// several languages, the ratchet groups of this language.
#[derive(Deserialize, Default)]
pub(crate) struct Tool {
    pub tool: String,
    pub name: String,
    #[serde(default)]
    pub groups: Vec<String>,
    /// The tool that decides `Ready` and is listed first; without one, the first of the list.
    #[serde(default)]
    pub primary: bool,
    /// A widely used secondary tool: weight 1 in the priority table (docs/ai/parity/queue.md).
    #[serde(default)]
    pub popular: bool,
    /// Why the leg waits on a user decision; the priority table lists it apart.
    #[serde(default)]
    pub blocked: Option<String>,
    /// A short caveat of the number, printed under the table (what the comparison leaves out).
    #[serde(default)]
    pub note: Option<String>,
}

/// The rows of `support.yaml`.
pub(crate) fn rows(support_yaml: &str) -> Result<Vec<Row>> {
    let mut rows: Vec<Row> =
        serde_saphyr::from_str(support_yaml).context("parsing support.yaml")?;
    for row in &mut rows {
        row.format.sort_by_key(|t| !t.primary); // stable: the primary first, the others as listed
        row.lint.sort_by_key(|t| !t.primary);
    }
    Ok(rows)
}

/// The keys of a `differences.yaml` the table reads.
#[derive(Deserialize, Default)]
struct Ratchets {
    #[serde(default)]
    ratchet: BTreeMap<String, u64>,
    #[serde(default, rename = "target-files")]
    target_files: BTreeMap<String, u64>,
    target: Option<f64>,
    /// No reference tool, format: the share of decided constructs that already have the chosen form on held-out code.
    agreement: Option<f64>,
    /// No reference tool, lint: the share of checklist entries a rule reports on the bad example and not the good one.
    checklist: Option<f64>,
    /// No reference tool, lint: the share of judged findings that are true.
    precision: Option<f64>,
    /// The findings of the lint corpus.
    measured: Option<Findings>,
}

#[derive(Deserialize, Default, Clone, Copy)]
struct Findings {
    covered: u64,
    total: u64,
}

/// The generated texts of the README: the table with the line under it, the headline count and the
/// note naming the ready languages.
pub struct Support {
    pub table: String,
    pub headline: String,
    pub ready: String,
}

/// The texts of `support_yaml` with the numbers below `root`.
pub fn render(
    root: &Path,
    support_yaml: &str,
    rule_count: &dyn Fn(&[String]) -> usize,
) -> Result<Support> {
    let rows = rows(support_yaml)?;
    let mut out = String::from("| Language | Format | Lint | Status |\n|---|---|---|---|\n");
    let mut measured = Vec::new();
    let in_order = ordered(root, &rows)?;
    for m in &in_order {
        let (row, status) = (m.row, m.status);
        let line = format!(
            "| {} | {} | {} | {status} |\n",
            row.language,
            cell(&row.format, &m.format).unwrap_or_else(|| match m.format.first() {
                Some(p) if !row.stability.is_empty() => match m.no_tool.0 {
                    Some(a) => format!(
                        "{} · {} · {NO_TOOL}",
                        share(*p, "stable"),
                        share(Some(a), "agreement")
                    ),
                    None => format!("{} · {NO_TOOL}", share(*p, "stable")),
                },
                _ => "none".into(),
            }),
            cell(&row.lint, &m.lint).unwrap_or_else(|| match rule_count(&row.ids) {
                n if n > 0 && m.no_tool.1.is_some() => format!(
                    "{n} rules · {} · {} · {NO_TOOL}",
                    share(m.no_tool.1, "checklist"),
                    share(m.no_tool.2, "precision")
                ),
                n if n > 0 => format!("{n} rules · {NO_TOOL}"),
                _ => "none".into(),
            }),
        );
        measured.push((status, row.language.as_str(), line));
    }
    let ready: Vec<&str> = measured
        .iter()
        .filter(|(status, ..)| status.ends_with("Ready"))
        .map(|(_, language, _)| *language)
        .collect();
    out += &measured.iter().map(|m| m.2.as_str()).collect::<String>();
    let notes: Vec<String> = in_order
        .iter()
        .map(|m| m.row)
        .flat_map(|r| r.format.iter().chain(&r.lint).map(move |t| (r, t)))
        .filter_map(|(r, t)| {
            Some(format!(
                "{} ({}): {}.",
                t.name,
                r.language,
                t.note.as_ref()?
            ))
        })
        .collect();
    if !notes.is_empty() {
        out += &format!("\nNotes: {}\n", notes.join(" "));
    }
    out += "\nThe rows are ordered by how widely the language is used. Every number is measured against each reference tool's default rule set; a row measured with another set says so in the notes above. How each language is measured, with the tool versions and corpora: [docs/languages.md](docs/languages.md#how-to-read-the-numbers). Presets: [configuration](docs/configuration.md#presets).\n";
    let (n, rest) = (ready.len(), rows.len() - ready.len());
    let headline = match n {
        0 => format!("Early support for {rest} languages."),
        _ => format!("Ready for {n} languages, early support for {rest} more."),
    };
    let ready = match ready.split_last() {
        None => "No language is ready yet.".to_string(),
        Some((last, [])) => format!("Ready: {last}. The rest are in progress."),
        Some((last, rest)) => {
            format!(
                "Ready: {} and {last}. The rest are in progress.",
                rest.join(", ")
            )
        }
    };
    Ok(Support {
        table: out,
        headline,
        ready,
    })
}

/// Every leg of `support.yaml` that is not `blocked` must have a number: a `target:` (lint) or a
/// `ratchet:` of its groups (format) in its `differences.yaml`; a leg that lost it fails with the tool's name.
pub fn check_measured(root: &Path, support_yaml: &str) -> Result<()> {
    for row in rows(support_yaml)? {
        if row.popularity.is_empty() {
            bail!(
                "support.yaml: row `{}` has no `popularity` (one member per language with the `gh` and `so` values its sources list; none for a source that does not list it)",
                row.language
            );
        }
        let no_format_tool = row.format.is_empty() && !row.stability.is_empty();
        if no_format_tool && !row.exempt {
            let agreement = match &row.evidence {
                Some(f) => ratchets(root, f)?.agreement,
                None => None,
            };
            if agreement.is_none() {
                bail!(
                    "support.yaml: format of `{}` has no reference tool: it needs `evidence:` (a folder of tests/parity) with an `agreement:` number (docs/ai/development/no-reference-tool.md), or `exempt: true` for the six legs that predate it",
                    row.language
                );
            }
        }
        if let Some(f) = &row.evidence {
            let r = ratchets(root, f)?;
            if r.checklist.is_some() != r.precision.is_some() {
                bail!(
                    "support.yaml: tests/parity/{f}/differences.yaml ({}) needs `checklist:` and `precision:` together",
                    row.language
                );
            }
        }
        for t in row.lint.iter().filter(|t| t.blocked.is_none()) {
            if ratchets(root, &t.tool)?.target.is_none() {
                bail!(
                    "support.yaml: lint tool `{}` ({}) has no `target:` in tests/parity/{}/differences.yaml (measure it, or mark it `blocked`)",
                    t.name,
                    row.language,
                    t.tool
                );
            }
        }
        for t in row.format.iter().filter(|t| t.blocked.is_none()) {
            let r = ratchets(root, &t.tool)?;
            if !r
                .ratchet
                .keys()
                .any(|g| t.groups.is_empty() || t.groups.contains(g))
            {
                bail!(
                    "support.yaml: format tool `{}` ({}) has no `ratchet:` in tests/parity/{}/differences.yaml (measure it, or mark it `blocked`)",
                    t.name,
                    row.language,
                    t.tool
                );
            }
        }
    }
    Ok(())
}

/// A row with its measured shares: format and lint per tool (the share and what it counts: identical files of
/// the files compared, covered findings of all), the status and the popularity.
pub(crate) struct Measured<'a> {
    pub row: &'a Row,
    pub format: Vec<Option<f64>>,
    pub lint: Vec<Option<f64>>,
    pub format_n: Vec<Option<(u64, u64)>>,
    pub lint_n: Vec<Option<(u64, u64)>>,
    pub status: &'static str,
    pub popularity: V,
    /// The numbers of a leg with no reference tool (`evidence`): agreement, checklist, precision.
    pub no_tool: (Option<f64>, Option<f64>, Option<f64>),
}

impl Measured<'_> {
    /// `Ready` rows come before the others (`In progress`, `not measured`).
    fn ready(&self) -> bool {
        self.status.ends_with("Ready")
    }
}

/// The percentage of a count pair.
fn percent(n: Option<(u64, u64)>) -> Option<f64> {
    n.filter(|n| n.1 > 0)
        .map(|(a, b)| a as f64 * 100.0 / b as f64)
}

/// The shares of every row, in the order of `rows`.
pub(crate) fn measure<'a>(root: &Path, rows: &'a [Row]) -> Result<Vec<Measured<'a>>> {
    rows.iter()
        .map(|row| {
            let mut format_n: Vec<Option<(u64, u64)>> = row
                .format
                .iter()
                .map(|t| format_counts(root, t))
                .collect::<Result<_>>()?;
            if !row.stability.is_empty() {
                let stability = Tool {
                    tool: STABILITY.into(),
                    name: String::new(),
                    groups: row.stability.clone(),
                    ..Tool::default()
                };
                format_n.push(format_counts(root, &stability)?);
            }
            let lint_r: Vec<Ratchets> = row
                .lint
                .iter()
                .map(|t| ratchets(root, &t.tool))
                .collect::<Result<_>>()?;
            let lint: Vec<Option<f64>> = lint_r.iter().map(|r| r.target).collect();
            let lint_n = lint_r
                .iter()
                .map(|r| r.measured.map(|f| (f.covered, f.total)))
                .collect();
            let format: Vec<Option<f64>> = format_n.iter().map(|n| percent(*n)).collect();
            let no_formatter = row.format.is_empty() && row.stability.is_empty();
            let no_tool = match &row.evidence {
                Some(f) => {
                    let r = ratchets(root, f)?;
                    (r.agreement, r.checklist, r.precision)
                }
                None => (None, None, None),
            };
            let ok = no_tool_ready(row, &format, no_tool);
            let status = status(no_formatter, &format, &lint, ok);
            Ok(Measured {
                row,
                format,
                lint,
                format_n,
                lint_n,
                status,
                popularity: row.popularity(),
                no_tool,
            })
        })
        .collect()
}

/// The rows in the order of every language list: the ready ones first, then the others, each by popularity
/// (most popular first), equal popularity by name.
pub(crate) fn ordered<'a>(root: &Path, rows: &'a [Row]) -> Result<Vec<Measured<'a>>> {
    let mut all = measure(root, rows)?;
    all.sort_by(|a, b| {
        (!a.ready())
            .cmp(&!b.ready())
            .then(b.popularity.v.total_cmp(&a.popularity.v))
            .then(a.row.language.cmp(&b.row.language))
    });
    Ok(all)
}

fn ratchets(root: &Path, tool: &str) -> Result<Ratchets> {
    let path = root
        .join("tests/parity")
        .join(tool)
        .join("differences.yaml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Ratchets::default());
    };
    serde_saphyr::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}

/// The ratchet group of the local fixture files of a tool folder: a few files, never the README number.
const FIXTURES: &str = "fixtures";

/// Identical files and files compared across the groups of the tool, never the local `fixtures` group; `None`
/// (`not measured`) until another group has `target-files`.
fn format_counts(root: &Path, tool: &Tool) -> Result<Option<(u64, u64)>> {
    let r = ratchets(root, &tool.tool)?;
    let wanted =
        |g: &&String| g.as_str() != FIXTURES && (tool.groups.is_empty() || tool.groups.contains(g));
    let files: u64 = r
        .target_files
        .keys()
        .filter(wanted)
        .map(|g| r.target_files[g])
        .sum();
    let same: u64 = r
        .target_files
        .keys()
        .filter(wanted)
        .map(|g| r.ratchet.get(g).copied().unwrap_or(0))
        .sum();
    Ok((files > 0).then_some((same, files)))
}

/// Bar, percent (rounded down, so the text never reads as passing the bar when it does not) and
/// name; `not measured` and the name when there is no number.
fn share(share: Option<f64>, name: &str) -> String {
    match share {
        Some(p) => {
            let full = ((p / 10.0).floor() as usize).min(10);
            format!(
                "{}{} {:.0}% {name}",
                "█".repeat(full),
                "░".repeat(10 - full),
                p.floor()
            )
        }
        None => format!("not measured {name}"),
    }
}

/// One line per tool; `None` when the language has no reference tool.
fn cell(tools: &[Tool], shares: &[Option<f64>]) -> Option<String> {
    if tools.is_empty() {
        return None;
    }
    Some(
        tools
            .iter()
            .zip(shares)
            .map(|(t, s)| share(*s, &t.name))
            .collect::<Vec<_>>()
            .join("<br>"),
    )
}

/// A new language's leg with no reference tool passes its own bars: format `stable` 100% and agreement 80%; lint
/// (when the evidence has a checklist) checklist 85% and precision 90%. The exempt legs and the legs with a tool pass.
fn no_tool_ready(
    row: &Row,
    format: &[Option<f64>],
    (agreement, checklist, precision): (Option<f64>, Option<f64>, Option<f64>),
) -> bool {
    let at = |v: Option<f64>, bar: f64| v.is_some_and(|v| v >= bar);
    let format_ok = row.exempt
        || !row.format.is_empty()
        || row.stability.is_empty()
        || (at(format.last().copied().flatten(), STABLE_BAR) && at(agreement, AGREEMENT_BAR));
    let lint_ok = (checklist.is_none() && precision.is_none())
        || (at(checklist, BAR) && at(precision, PRECISION_BAR));
    format_ok && lint_ok
}

/// `Ready` when the primary (`primary: true`, else the first listed) format and lint tool are both at the bar; further tools
/// are shown but do not decide the status.
fn status(
    no_formatter: bool,
    format: &[Option<f64>],
    lint: &[Option<f64>],
    no_tool_ok: bool,
) -> &'static str {
    let all: Vec<Option<f64>> = format.iter().chain(lint).copied().collect();
    let primary = format.first().into_iter().chain(lint.first());
    let ready = no_tool_ok && primary.copied().all(|s| s.is_some_and(|p| p >= BAR));
    match (all.iter().all(Option::is_none), no_formatter, ready) {
        (true, ..) => "not measured",
        (_, true, true) => "Lint only: Ready",
        (_, true, false) => "Lint only: In progress",
        (_, false, true) => "Ready",
        (_, false, false) => "In progress",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/support")
    }

    fn row(popularity: &str) -> Row {
        let text = format!("- language: X\n  popularity: {popularity}\n");
        rows(&text).unwrap().remove(0)
    }

    #[test]
    fn a_grouped_row_takes_its_most_popular_member() {
        // A quarter of the top of each source: the square root gives 0.5 for both.
        let quarter = "{name: A, gh: 2082333.75, so: 15.7}";
        let small = "{name: B, gh: 520583.4375, so: 3.925}";
        let p = row(&format!("[{small}, {quarter}]")).popularity();
        assert!((p.v - 0.5).abs() < 1e-9 && !p.est, "{p:?}");
    }

    #[test]
    fn a_missing_source_makes_the_popularity_an_estimate() {
        assert!(row("[{name: A, gh: 2082333.75}]").popularity().est);
        // Neither source lists it: the floor, an estimate.
        let none = row("[{name: A}]").popularity();
        assert!(none.est && (none.v - 0.1).abs() < 1e-9, "{none:?}");
    }

    #[test]
    fn a_hand_set_estimate_orders_the_row_but_not_the_new_language_scores() {
        let r = row("[{name: A, gh: 2082333.75, estimate: {value: 0.9, basis: b}}]");
        let p = r.popularity();
        assert!((p.v - 0.9).abs() < 1e-9 && p.est, "{p:?}");
        // The formula alone (one source: an estimate by 2.27) stays what the scores read.
        assert!(r.source_popularity().v < 0.9);
    }

    #[test]
    fn a_row_without_popularity_fails_the_check() {
        let e = check_measured(&fixtures(), "- language: A\n").unwrap_err();
        assert!(e.to_string().contains("no `popularity`"), "{e}");
    }

    #[test]
    fn equal_popularity_is_ordered_by_name() {
        let list = rows("- language: B\n- language: A\n").unwrap();
        let names: Vec<&str> = ordered(&fixtures(), &list)
            .unwrap()
            .iter()
            .map(|m| m.row.language.as_str())
            .collect();
        assert_eq!(names, ["A", "B"]);
    }
}
