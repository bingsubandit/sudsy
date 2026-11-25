//! What the site and the package registries show, generated from the data the README table is made
//! from: `docs/ai/parity/site.json` (one entry per compared tool with its catalog, one per language
//! row of `support.yaml`; read by `site/scripts/generate.mjs`) and `packaging/meta.json` (the
//! description and keywords of the packages; read by `scripts/package-channels.py`). A language of
//! the providers, or a row of `bench/speed.json`, that no row of `support.yaml` names fails the build:
//! it would have no page.

use crate::support::{self, Row};
use anyhow::{Result, bail};
use languages::catalog::Catalog;
use provider_api::LanguageProvider;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The files of this module and the spliced `packaging/README.md`.
pub(crate) fn generate(
    providers: &[Box<dyn LanguageProvider>],
    catalogs: &[Catalog],
    support_yaml: &str,
    support: &support::Support,
    rule_count: &dyn Fn(&[String]) -> usize,
) -> Result<Vec<(PathBuf, String)>> {
    let root = xtask::repo_root();
    let rows = support::rows(support_yaml)?;
    check(providers, &rows)?;
    let measured = support::ordered(&root, &rows)?;
    let shares = crate::docs::coverage_shares()?;
    let presets = presets()?;
    let catalog_of = catalog_names()?;
    let mut languages = Vec::new();
    for m in &measured {
        let row = m.row;
        let tool = |ts: &[support::Tool], shares: &[Option<f64>]| -> Vec<Value> {
            ts.iter()
                .zip(shares)
                .map(|(t, s)| {
                    let catalog = catalog_of
                        .get(&t.tool)
                        .filter(|c| catalogs.iter().any(|k| &k.tool == *c));
                    json!({ "name": t.name, "share": s, "catalog": catalog, "note": t.note })
                })
                .collect()
        };
        languages.push(json!({
            "slug": row.ids[0],
            "name": row.language,
            "ids": row.ids,
            "status": m.status,
            "format": tool(&row.format, &m.format),
            "lint": tool(&row.lint, &m.lint),
            "requires": row.requires,
            "section": row.section(),
            "popularity_of": row.most_popular().0,
            "popularity_basis": row.popularity.iter().find_map(|m| m.estimate.as_ref()).map(|e| e.basis.as_str()),
            "popularity": (m.popularity.v * 1000.0).round() / 1000.0,
            "popularity_estimate": m.popularity.est,
            "rules": rule_count(&row.ids),
            "presets": presets.iter().filter(|p| own_preset(providers, &row.ids, p)).collect::<Vec<_>>(),
            "speed": crate::speed::rows_of(&row.speed)?,
        }));
    }
    // One line per language, per tool and per catalog entry: a diff shows what moved.
    let mut tools = Vec::new();
    // The languages of a tool in the order of the language lists (an id of no row last).
    let rank = |id: &String| {
        measured
            .iter()
            .position(|m| m.row.ids.contains(id))
            .unwrap_or(usize::MAX)
    };
    for c in catalogs {
        let target = shares.get(&c.tool).copied();
        let mut languages = c.languages.clone();
        languages.sort_by_key(rank);
        let head = json!({
            "tool": c.tool, "version": catalog_version(&c.tool)?, "languages": languages,
            "target": target, "rule_url": c.rule_url != "none",
        })
        .to_string();
        let entries: Vec<String> = c
            .entries
            .iter()
            .map(|e| json!([e.id, e.coverage, e.sudsy, e.gap, e.url]).to_string())
            .collect();
        tools.push(format!(
            "{},\"entries\":[\n{}\n]}}",
            &head[..head.len() - 1],
            entries.join(",\n")
        ));
    }
    let lines = |v: Vec<String>| v.join(",\n");
    let site = format!(
        "{{\"languages\":[\n{}\n],\n\"tools\":[\n{}\n]}}\n",
        lines(languages.iter().map(Value::to_string).collect()),
        lines(tools)
    );

    // The most-searched terms first: the generic words, the tools a preset reproduces (in the order
    // of POPULAR), then language names, up to MAX_KEYWORDS.
    let tool_words = reproduced_tools(&rows, &presets);
    let mut keywords: Vec<String> = ["formatter", "linter", "code-quality"]
        .map(String::from)
        .into();
    keywords.extend(
        POPULAR
            .iter()
            .filter(|t| tool_words.iter().any(|w| w == *t))
            .map(|t| t.to_string()),
    );
    for row in measured.iter().map(|m| m.row) {
        for part in row.language.split(['/', ',', '(', ')']) {
            keywords.push(part.trim().to_lowercase());
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    keywords.retain(|k| !k.is_empty() && seen.insert(k.clone()));
    keywords.truncate(MAX_KEYWORDS);
    let meta = json!({
        "description": format!("Formatter and linter for {} languages in one binary and one config file", rows.len()),
        "headline": support.headline,
        "ready": support.ready,
        "keywords": keywords,
    });
    let mut out = vec![
        ("docs/ai/parity/site.json".into(), site),
        (
            "packaging/meta.json".into(),
            serde_json::to_string_pretty(&meta)? + "\n",
        ),
    ];
    out.push(crate::docs::splice(
        "packaging/README.md",
        &[
            ("support-headline", support.headline.clone()),
            ("support-ready", support.ready.clone()),
        ],
    )?);
    Ok(out)
}

/// npm and PyPI show about this many keywords; more look like spam.
const MAX_KEYWORDS: usize = 20;
/// Tools people search for, most searched first. Only those a preset reproduces become keywords.
const POPULAR: [&str; 16] = [
    "prettier",
    "eslint",
    "ruff",
    "biome",
    "stylelint",
    "shellcheck",
    "hadolint",
    "markdownlint",
    "rubocop",
    "yamllint",
    "shfmt",
    "ktlint",
    "phpstan",
    "csharpier",
    "stylecop",
    "tflint",
];

fn catalog_version(tool: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Head {
        version: Option<String>,
    }
    let text =
        std::fs::read_to_string(xtask::repo_root().join(format!("docs/ai/parity/{tool}.yaml")))?;
    let head: Head = serde_saphyr::from_str(&text)?;
    Ok(head.version.unwrap_or_default())
}

/// Every provider language is in a row, no language is in two rows, and every language name of
/// `bench/speed.json` belongs to a row.
fn check(providers: &[Box<dyn LanguageProvider>], rows: &[Row]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for row in rows {
        if row.ids.is_empty() {
            bail!("support.yaml: row `{}` has no `ids`", row.language);
        }
        for id in &row.ids {
            if !seen.insert(id.as_str()) {
                bail!("support.yaml: language id `{id}` is in two rows");
            }
        }
    }
    for p in providers {
        if !seen.contains(p.language()) {
            bail!(
                "support.yaml: language `{}` is in no row, so it has no page on the site",
                p.language()
            );
        }
    }
    for name in crate::speed::language_names()? {
        if !rows.iter().any(|r| r.speed.contains(&name)) {
            bail!("support.yaml: `{name}` of bench/speed.json is in no row's `speed`");
        }
    }
    Ok(())
}

/// A preset is a row's own when its folder is one of the row's language ids or the crate of one of the
/// row's languages (`html/` presets belong to HTML, Vue / Svelte and Angular templates). A preset of
/// another language that only enables a shared rule does not count.
fn own_preset(providers: &[Box<dyn LanguageProvider>], ids: &[String], preset: &str) -> bool {
    let folder = preset.split('/').next().unwrap_or_default();
    ids.iter().any(|i| i == folder)
        || providers.iter().any(|p| {
            ids.iter().any(|i| i == p.language()) && p.crate_name() == format!("lang-{folder}")
        })
}

/// `<language>/<name>` of every preset file.
fn presets() -> Result<Vec<String>> {
    let mut out = Vec::new();
    for krate in std::fs::read_dir(xtask::repo_root().join("crates"))? {
        let krate = krate?;
        let name = krate.file_name().to_string_lossy().into_owned();
        let Some(language) = name.strip_prefix("lang-") else {
            continue;
        };
        let Ok(dir) = std::fs::read_dir(krate.path().join("presets")) else {
            continue;
        };
        for f in dir {
            let f = f?.file_name().to_string_lossy().into_owned();
            out.extend(f.strip_suffix(".yaml").map(|s| format!("{language}/{s}")));
        }
    }
    out.sort();
    Ok(out)
}

/// The catalog of each tool folder of `tests/parity` (`scripts/parity_tools.json`).
fn catalog_names() -> Result<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(xtask::repo_root().join("scripts/parity_tools.json"))?;
    let modes: BTreeMap<String, BTreeMap<String, Value>> = serde_json::from_str(&text)?;
    let mut out = BTreeMap::new();
    for entry in modes.values().flatten() {
        if let Some(catalog) = entry.1["catalog"].as_str() {
            out.entry(entry.0.clone())
                .or_insert_with(|| catalog.to_string());
        }
    }
    Ok(out)
}

/// The lowercase first word of each reference tool that a preset is named after (a preset
/// reproduces it): a tool name is a keyword only then.
fn reproduced_tools(rows: &[Row], presets: &[String]) -> Vec<String> {
    let stems: Vec<&str> = presets
        .iter()
        .filter_map(|p| p.split_once('/'))
        .map(|p| p.1)
        .collect();
    let mut out = Vec::new();
    for row in rows {
        for t in row.format.iter().chain(&row.lint) {
            let word = t
                .name
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_lowercase();
            if !word.is_empty() && stems.iter().any(|s| s.starts_with(&word)) {
                out.push(word);
            }
        }
    }
    out
}
