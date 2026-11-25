//! The reference tables of the docs, generated from the registered rules, the option schemas and
//! the parity catalog. A generated block sits between `<!-- generated:NAME -->` and
//! `<!-- /generated -->`; the prose around it is hand-written.

use anyhow::{Context, Result, bail};
use core_config::{OptionKind, OptionSchema, OptionValue};
use languages::catalog::Catalog;
use provider_api::LanguageProvider;
use std::collections::BTreeMap;
use std::path::PathBuf;

const END: &str = "<!-- /generated -->";

/// Every doc with generated blocks as (path relative to the repo root, contents).
pub fn generate(providers: &[Box<dyn LanguageProvider>]) -> Result<Vec<(PathBuf, String)>> {
    let catalogs = catalogs()?;
    let rule_docs = crate::rules::collect(providers, &catalogs);
    let schemas = languages::schemas(providers);
    let mut out = vec![
        splice(
            "docs/ai/rules/builtin.md",
            &[("rules", crate::rules::table(&rule_docs))],
        )?,
        (
            "docs/ai/rules/rules.json".into(),
            crate::rules::json(&rule_docs),
        ),
    ];
    let baseline =
        std::fs::read_to_string(xtask::repo_root().join("docs/ai/rules/undescribed.txt"));
    out.push((
        "docs/ai/rules/undescribed.txt".into(),
        crate::rules::undescribed(&rule_docs, baseline.ok().as_deref())?,
    ));
    for p in providers {
        let rel = format!("docs/ai/formatting/{}.md", p.language());
        // A dialect without a doc of its own (scss, less) is documented with its config base.
        if p.config_base().is_some() && !xtask::repo_root().join(&rel).exists() {
            continue;
        }
        out.push(splice(
            &rel,
            &[("options", options_table(p.option_schema()))],
        )?);
    }
    out.push(splice(
        "docs/ai/formatting/engine.md",
        &[("shared-options", shared_options_table(&schemas))],
    )?);
    out.push(splice(
        "docs/ai/architecture/syntax-tree.md",
        &[("roles", roles_table()), ("operators", operators_table())],
    )?);
    let root = xtask::repo_root();
    let support_rows = crate::support::rows(include_str!("../support.yaml"))?;
    let in_order = crate::support::ordered(&root, &support_rows)?;
    // The language lists name the languages in the order of the support table.
    let rank = |id: &str| {
        in_order
            .iter()
            .position(|m| m.row.ids.iter().any(|i| i == id))
            .unwrap_or(usize::MAX)
    };
    let mut titles: Vec<(usize, &str, &str)> = providers
        .iter()
        .map(|p| (rank(p.language()), p.grammar().title.as_str(), p.language()))
        .collect();
    titles.sort();
    // A dialect with its own provider (GitHub Actions) shares its grammar, and so its title, with its base.
    let mut shown: Vec<&str> = titles.iter().map(|t| t.1).collect();
    let mut seen_title = std::collections::BTreeSet::new();
    shown.retain(|t| seen_title.insert(*t));
    let (last, rest) = shown.split_last().expect("a language");
    let names = format!("{} and {last}", rest.join(", "));
    // A crate with several languages (lang-css) is listed once.
    let mut crates: Vec<String> = providers
        .iter()
        .map(|p| (p.grammar().title.as_str(), p.language(), p.crate_name()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|(_, language, krate)| match krate {
            "" => format!("`lang-{language}`"),
            _ => format!("`{krate}`"),
        })
        .collect();
    crates.dedup();
    let crates = crates.join(", ");
    // The rules a language has: its own and the shared ones.
    let rule_count = |ids: &[String]| {
        rule_docs
            .iter()
            .filter(|d| d.languages.iter().any(|l| l == "all" || ids.contains(l)))
            .count()
    };
    crate::support::check_measured(&root, include_str!("../support.yaml"))?;
    // The language names of bench/speed.json in the order of the language lists.
    let speed_order: Vec<String> = in_order.iter().flat_map(|m| m.row.speed.clone()).collect();
    crate::numbers::check(&root, &crate::numbers::tool_names(&root, &support_rows)?)?;
    out.push(crate::languages_doc::generate(
        &std::fs::read_to_string(root.join("docs/languages.md"))?,
        &in_order,
        &rule_count,
    )?);
    let support = crate::support::render(
        &xtask::repo_root(),
        include_str!("../support.yaml"),
        &rule_count,
    )?;
    out.extend(crate::discover::generate(
        providers,
        &catalogs,
        include_str!("../support.yaml"),
        &support,
        &rule_count,
    )?);
    out.push(splice(
        "README.md",
        &[
            ("languages", names.clone()),
            ("speed", crate::speed::table(&speed_order)?),
            ("support", support.table),
            ("support-headline", support.headline),
            ("support-ready", support.ready),
            (
                "beyond",
                crate::beyond::render(
                    providers,
                    include_str!("../tool-options.yaml"),
                    include_str!("../support.yaml"),
                )?,
            ),
        ],
    )?);
    out.push(splice(
        "docs/performance.md",
        &[("speed-method", crate::speed::method(&speed_order)?)],
    )?);
    for rel in ["editors/vscode/README.md", "docs/ai/editor/vscode.md"] {
        out.push(splice(rel, &[("languages", names.clone())])?);
    }
    out.push(splice(
        "docs/ai/architecture/overview.md",
        &[("crates", crates)],
    )?);
    out.push(splice(
        "docs/ai/parity/README.md",
        &[("counts", catalog_counts(catalogs)?)],
    )?);
    out.push(splice(
        "docs/ai/development/parity.md",
        &[("parity-tools", parity_tools()?)],
    )?);
    // The differences blocks go into the text the other generators made of the same file.
    for (path, text) in crate::differences::generate(&out)? {
        out.retain(|(p, _)| *p != path);
        out.push((path, text));
    }
    out.push(crate::queue::generate()?);
    Ok(out)
}

/// Every catalog (`docs/ai/parity/*.yaml`) by tool name.
pub(crate) fn catalogs() -> Result<Vec<Catalog>> {
    let mut catalogs = Vec::new();
    for file in std::fs::read_dir(xtask::repo_root().join("docs/ai/parity"))? {
        let path = file?.path();
        if path.extension().is_some_and(|e| e == "yaml") {
            let text = std::fs::read_to_string(&path)?;
            catalogs.push(
                languages::catalog::parse(&text)
                    .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?,
            );
        }
    }
    catalogs.sort_by(|a, b| a.tool.cmp(&b.tool));
    Ok(catalogs)
}

/// One row per catalog: the entries by coverage.
fn catalog_counts(catalogs: Vec<Catalog>) -> Result<String> {
    let ratchets = lint_ratchets()?;
    let mut out = String::from(
        "| Tool | Target | Measured on | Entries | Covered | Partial | Missing | SDK | Wontfix | Deferred | Planned |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
",
    );
    for c in catalogs {
        let n = |coverage: &str| c.entries.iter().filter(|e| e.coverage == coverage).count();
        let planned = c.entries.iter().filter(|e| e.sudsy == "planned").count();
        out += &format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {planned} |
",
            c.tool,
            ratchets
                .get(&c.tool)
                .and_then(|r| r.target)
                .map_or("-".to_string(), |t| format!("{t:.1}%")),
            ratchets
                .get(&c.tool)
                .and_then(|r| r.measured.as_ref()?.main.as_deref())
                .map_or("-".to_string(), |m| format!("`{m}`")),
            c.entries.len(),
            n("covered"),
            n("partial"),
            n("missing"),
            n("sdk"),
            n("wontfix"),
            n("deferred"),
        );
    }
    Ok(out)
}

/// The `target` ratchet (the share of the tool's findings on its corpus that Sudsy also reports,
/// leaving out the reviewed exclusions of docs/ai/design/parity-target.md, `scripts/parity.sh
/// coverage <tool>`) of each lint tool, by catalog name.
pub(crate) fn coverage_shares() -> Result<BTreeMap<String, f64>> {
    Ok(lint_ratchets()?
        .into_iter()
        .filter_map(|(catalog, r)| Some((catalog, r.target?)))
        .collect())
}

/// What a lint tool's `differences.yaml` records of its coverage run.
#[derive(serde::Deserialize)]
struct Ratchets {
    target: Option<f64>,
    measured: Option<Measured>,
}

/// The `measured` key: the origin/main commit the measured tree is based on.
#[derive(serde::Deserialize)]
struct Measured {
    main: Option<String>,
}

/// The ratchets of each lint tool that has a `differences.yaml`, by catalog name.
fn lint_ratchets() -> Result<BTreeMap<String, Ratchets>> {
    let root = xtask::repo_root();
    let text = std::fs::read_to_string(root.join("scripts/parity_tools.json"))?;
    let modes: BTreeMap<String, BTreeMap<String, serde_json::Value>> = serde_json::from_str(&text)?;
    let mut out = BTreeMap::new();
    for (tool, entry) in &modes["lint"] {
        // A tool entry with `"readme": false` is a ratchet of its own that the tables leave out.
        if entry["readme"].as_bool() == Some(false) {
            continue;
        }
        let path = root
            .join("tests/parity")
            .join(tool)
            .join("differences.yaml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let r: Ratchets = serde_saphyr::from_str(&text)
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        if let Some(catalog) = entry["catalog"].as_str() {
            out.insert(catalog.to_string(), r);
        }
    }
    Ok(out)
}

/// The tools of `scripts/parity_tools.json`, per mode.
fn parity_tools() -> Result<String> {
    let text = std::fs::read_to_string(xtask::repo_root().join("scripts/parity_tools.json"))?;
    let modes: BTreeMap<String, BTreeMap<String, serde_json::Value>> = serde_json::from_str(&text)?;
    let list = |mode: &str| {
        modes[mode]
            .keys()
            .map(|t| format!("`{t}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Ok(format!(
        "{} (lint mode); {} (format mode, below).",
        list("lint"),
        list("format")
    ))
}

/// The file with the body of each named block replaced.
pub(crate) fn splice(rel: &str, blocks: &[(&str, String)]) -> Result<(PathBuf, String)> {
    let path = xtask::repo_root().join(rel);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
    splice_text(rel, text, blocks)
}

/// [`splice`] into `text`, the file's content so far.
pub(crate) fn splice_text(
    rel: &str,
    mut text: String,
    blocks: &[(&str, String)],
) -> Result<(PathBuf, String)> {
    for (name, body) in blocks {
        let start = format!("<!-- generated:{name} -->");
        // Only the first block would be rewritten: a merge that adds a second, stale one must fail.
        if text.matches(&start).count() > 1 {
            bail!("{rel}: `{start}` occurs more than once: delete the stale block");
        }
        let from = text
            .find(&start)
            .with_context(|| format!("{rel}: no `{start}` marker"))?
            + start.len();
        let Some(len) = text[from..].find(END) else {
            bail!("{rel}: `{start}` has no `{END}`");
        };
        // A marker followed by text on its line is an inline block: the body replaces the text.
        let newline = if text[from..].starts_with('\n') {
            "\n"
        } else {
            ""
        };
        text.replace_range(from..from + len, &format!("{newline}{body}"));
    }
    Ok((rel.into(), text))
}

/// The role vocabulary of `core_syntax::roles`.
fn roles_table() -> String {
    let mut out = String::from("| Role | Fields | Meaning |\n|---|---|---|\n");
    for r in core_syntax::roles::ROLES {
        let fields = if r.fields.is_empty() {
            "-".to_string()
        } else {
            r.fields
                .split(' ')
                .map(|f| format!("`{f}`"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        out += &format!("| `#{}` | {fields} | {} |\n", r.name, cell(r.doc));
    }
    out
}

/// The operator kinds of `core_syntax::roles`.
fn operators_table() -> String {
    let mut out = String::from("| Kind | Common texts |\n|---|---|\n");
    for (kind, texts) in core_syntax::roles::OPERATOR_KINDS {
        out += &format!("| `{kind}` | {} |\n", cell(texts));
    }
    out
}

/// A table cell: one line, no pipes.
fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

fn show(value: &OptionValue) -> String {
    match value {
        OptionValue::Bool(b) => b.to_string(),
        OptionValue::Int(n) => n.to_string(),
        OptionValue::Float(f) => f.to_string(),
        OptionValue::String(s) => s.clone(),
        OptionValue::List(items) => {
            format!(
                "[{}]",
                items.iter().map(show).collect::<Vec<_>>().join(", ")
            )
        }
        OptionValue::Map(_) => "{}".into(),
    }
}

fn values(kind: &OptionKind) -> String {
    match kind {
        OptionKind::Bool => "bool".into(),
        OptionKind::Int { min, max } => format!("int {min}-{max}"),
        OptionKind::Enum(values) => values
            .iter()
            .map(|v| format!("`{v}`"))
            .collect::<Vec<_>>()
            .join(", "),
        OptionKind::Words => "list of words".into(),
        OptionKind::WordSets => "list of lists of words".into(),
    }
}

fn options_table(schema: &OptionSchema) -> String {
    let mut out = String::from("| Option | Values | Default | Description |\n|---|---|---|---|\n");
    for d in schema.defs() {
        out += &format!(
            "| `{}` | {} | `{}` | {} |\n",
            d.name,
            values(&d.kind),
            show(&d.default),
            cell(d.description)
        );
    }
    out
}

/// The options several languages declare, with each language's default.
fn shared_options_table(schemas: &[(&str, &OptionSchema)]) -> String {
    let mut by_name: BTreeMap<&str, Vec<(&str, &core_config::OptionDef)>> = BTreeMap::new();
    for (language, schema) in schemas {
        for d in schema.defs() {
            by_name.entry(d.name).or_default().push((language, d));
        }
    }
    let mut out = String::from("| Option | Values | Defaults |\n|---|---|---|\n");
    for (name, declared) in by_name.iter().filter(|(_, d)| d.len() > 1) {
        let defaults: Vec<String> = declared
            .iter()
            .map(|(language, d)| format!("{language} `{}`", show(&d.default)))
            .collect();
        let kinds: Vec<String> = declared.iter().map(|(_, d)| values(&d.kind)).collect();
        let kind = if kinds.iter().all(|k| *k == kinds[0]) {
            kinds[0].clone()
        } else {
            declared
                .iter()
                .zip(&kinds)
                .map(|((language, _), k)| format!("{language}: {k}"))
                .collect::<Vec<_>>()
                .join("; ")
        };
        out += &format!("| `{name}` | {kind} | {} |\n", defaults.join(", "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::splice_text;

    #[test]
    fn a_block_that_occurs_twice_fails() {
        let one = "a <!-- generated:x -->old<!-- /generated --> b".to_string();
        let blocks = [("x", "new".to_string())];
        assert_eq!(
            splice_text("f.md", one.clone(), &blocks).unwrap().1,
            "a <!-- generated:x -->new<!-- /generated --> b"
        );
        let err = splice_text("f.md", format!("{one} {one}"), &blocks).unwrap_err();
        assert!(err.to_string().contains("more than once"), "{err}");
    }
}
