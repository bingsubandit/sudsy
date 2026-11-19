//! The README "Performance" table, generated from `bench/speed.json` (written by `scripts/speed.sh`, run on a
//! fixed runner by `.github/workflows/speed.yml`; format: docs/ai/design/speed-comparison.md). One row per
//! language, task and tool; Sudsy is always the subject of the speed column.

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Speed {
    runner: String,
    runs: u32,
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    language: String,
    task: String,
    tool: String,
    #[serde(default)]
    corpus: String,
    not_measured: Option<String>,
    sudsy: Option<Run>,
    other: Option<Run>,
}

/// The best run: wall seconds and peak resident memory in MB.
#[derive(Deserialize)]
struct Run {
    s: f64,
    mb: f64,
}

/// The table and the line under it.
pub(crate) fn table() -> Result<String> {
    let path = xtask::repo_root().join("bench/speed.json");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let speed: Speed =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let mut languages: Vec<&str> = Vec::new();
    for row in &speed.rows {
        if !languages.contains(&row.language.as_str()) {
            languages.push(&row.language);
        }
    }
    let mut out = String::from(
        "| Language | Task | Compared with | Sudsy speed | Time (Sudsy / tool) | Peak memory (Sudsy / tool) |\n|---|---|---|---|---|---|\n",
    );
    let mut corpora = Vec::new();
    for language in &languages {
        let mut rows: Vec<&Row> = speed
            .rows
            .iter()
            .filter(|r| r.language == *language)
            .collect();
        rows.sort_by(|a, b| a.task.cmp(&b.task)); // stable: Format before Lint, tools in file order
        let mut texts: Vec<&str> = Vec::new();
        for row in rows.iter().filter(|r| !r.corpus.is_empty()) {
            if !texts.contains(&row.corpus.as_str()) {
                texts.push(&row.corpus);
            }
        }
        if !texts.is_empty() {
            corpora.push(format!("{language}: {}", texts.join(" and ")));
        }
        let mut task = "";
        for (i, row) in rows.iter().enumerate() {
            let (language_cell, task_cell) = (
                if i == 0 { language } else { "" },
                if row.task == task { "" } else { &row.task },
            );
            task = &row.task;
            let (speed_cell, time, memory) = match (&row.sudsy, &row.other) {
                (Some(s), Some(o)) => (
                    verdict(s.s, o.s),
                    format!("{} / {}", seconds(s.s), seconds(o.s)),
                    format!("{:.0} / {:.0} MB", s.mb, o.mb),
                ),
                _ => (
                    format!(
                        "not measured: {}",
                        row.not_measured.as_deref().unwrap_or("no result")
                    ),
                    "-".into(),
                    "-".into(),
                ),
            };
            out += &format!(
                "| {language_cell} | {task_cell} | {} | {speed_cell} | {time} | {memory} |\n",
                row.tool
            );
        }
    }
    out += &format!(
        "\nRunner: {}. Best of {} runs, a new process each, caches off, `format --check` and `lint` with the Sudsy preset that mirrors the tool. Corpus ([docs/languages.md](docs/languages.md)): {}. Rerun one row: `scripts/speed.sh <tool>`.\n",
        speed.runner,
        speed.runs,
        corpora.join("; ")
    );
    Ok(out)
}

/// What Sudsy is against the tool: `same` within 10%, else `N× faster` or `N× slower`.
fn verdict(sudsy: f64, tool: f64) -> String {
    let ratio = tool / sudsy;
    let times = |x: f64| {
        if x >= 10.0 {
            format!("{x:.0}×")
        } else {
            format!("{x:.1}×")
        }
    };
    if (1.0 / 1.1..=1.1).contains(&ratio) {
        "same".into()
    } else if ratio > 1.0 {
        format!("{} faster", times(ratio))
    } else {
        format!("{} slower", times(1.0 / ratio))
    }
}

fn seconds(s: f64) -> String {
    if s < 0.1 {
        format!("{s:.3} s")
    } else {
        format!("{s:.2} s")
    }
}
