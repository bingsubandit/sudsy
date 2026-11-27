//! The VS Code extension's language list, from the providers and the `vscode-languages` fact of
//! each `lang.yaml`: the `activationEvents`, `keywords` and `description` of `package.json` and
//! the document selector in `src/selector.generated.ts`.

use anyhow::{Context, Result, bail};
use provider_api::LanguageProvider;
use std::path::PathBuf;

/// Marketplace limit on `keywords`.
const MAX_KEYWORDS: usize = 30;
/// Configuration and markup formats, in the order their keywords are dropped when the languages
/// do not fit next to the two generic words: a programming language is never dropped.
const CONFIG_FORMATS: [&str; 9] = [
    "EditorConfig",
    "Gitignore",
    "INI",
    "Makefile",
    "Dockerfile",
    "TOML",
    "XML",
    "YAML",
    "JSON",
];
/// Always among the keywords: what people search for.
const GENERIC_KEYWORDS: [&str; 3] = ["formatter", "linter", "prettier"];
/// The description: the intro of the README.
const DESCRIPTION: &str = "A true polyglot formatter and linter for every repo: one tool and one config file in place of Prettier, ESLint, ruff and more. Extensible with YAML rules and Wasm plugins.";

/// The providers of languages with an id of their own: one that shares another's grammar (GitHub Actions on YAML) has none.
fn own_language(
    providers: &[Box<dyn LanguageProvider>],
) -> impl Iterator<Item = &dyn LanguageProvider> {
    providers
        .iter()
        .map(|p| p.as_ref())
        .filter(|p| p.grammar().language == p.language())
}

/// VS Code language ids of each crate, in provider order, each crate once.
fn ids_by_crate(providers: &[Box<dyn LanguageProvider>]) -> Result<Vec<(String, Vec<String>)>> {
    let langs = xtask::lang_crate::load(&xtask::repo_root())?;
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for p in own_language(providers) {
        if out.iter().any(|(k, _)| k == p.crate_name()) {
            continue;
        }
        let lang = langs
            .iter()
            .find(|l| l.krate == p.crate_name())
            .with_context(|| format!("{}: no lang.yaml", p.crate_name()))?;
        let own = xtask::lang_crate::list(&lang.yaml, "vscode-languages");
        if own.is_empty() {
            bail!("{}: lang.yaml has no `vscode-languages`", p.crate_name());
        }
        out.push((p.crate_name().to_string(), own));
    }
    Ok(out)
}

/// VS Code language ids, in provider order, each once.
fn language_ids(providers: &[Box<dyn LanguageProvider>]) -> Result<Vec<String>> {
    let mut ids: Vec<String> = Vec::new();
    for id in ids_by_crate(providers)?
        .into_iter()
        .flat_map(|(_, ids)| ids)
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Display names of the languages, `C/C++` as `C` and `C++`; the first 28.
fn keywords(providers: &[Box<dyn LanguageProvider>]) -> Result<Vec<String>> {
    let mut words: Vec<String> = Vec::new();
    for p in own_language(providers) {
        for part in p.grammar().title.split('/') {
            if !words.iter().any(|w| w == part) {
                words.push(part.to_string());
            }
        }
    }
    // The generic words keep their places; configuration formats leave first.
    let room = MAX_KEYWORDS - GENERIC_KEYWORDS.len();
    for format in CONFIG_FORMATS {
        if words.len() <= room {
            break;
        }
        words.retain(|w| w != format);
    }
    words.extend(GENERIC_KEYWORDS.map(String::from));
    if words.len() > MAX_KEYWORDS {
        bail!(
            "{} keywords, VS Code allows {MAX_KEYWORDS}: {words:?}",
            words.len()
        );
    }
    Ok(words)
}

/// Replaces the `"key": [ ... ],` array of the package manifest with `items`.
fn replace_array(package: &mut String, key: &str, items: &[String]) -> Result<()> {
    let head = format!("\"{key}\": [");
    let from = package
        .find(&head)
        .with_context(|| format!("package.json: no {key}"))?;
    let len = package[from..]
        .find("],")
        .with_context(|| format!("unterminated {key}"))?
        + 2;
    let body: String = items.iter().map(|i| format!("    \"{i}\",\n")).collect();
    let body = body.trim_end_matches([',', '\n']);
    package.replace_range(from..from + len, &format!("{head}\n{body}\n  ],"));
    Ok(())
}

pub fn generate(providers: &[Box<dyn LanguageProvider>]) -> Result<Vec<(PathBuf, String)>> {
    let ids = language_ids(providers)?;
    // By extension as well, so that it works without an extension that names the language.
    let mut extensions: Vec<&str> = providers
        .iter()
        .flat_map(|p| p.file_matchers())
        .filter_map(|m| m.pattern.extension())
        .collect();
    extensions.dedup();
    let selector: String = ids
        .iter()
        .map(|id| format!("  {{ scheme: \"file\", language: \"{id}\" }},\n"))
        .chain(
            extensions
                .iter()
                .map(|e| format!("  {{ scheme: \"file\", pattern: \"**/*.{e}\" }},\n")),
        )
        .collect();
    let selector = format!(
        "// @generated by `cargo xtask codegen` from the language providers. Do not edit.\n\
         export const documentSelector = [\n{selector}];\n"
    );

    let rel = "editors/vscode/package.json";
    let mut package = std::fs::read_to_string(xtask::repo_root().join(rel))?;
    let mut events: Vec<String> = ids.iter().map(|id| format!("onLanguage:{id}")).collect();
    events.push("workspaceContains:**/sudsy.{yaml,yml}".into());
    replace_array(&mut package, "activationEvents", &events)?;
    replace_array(&mut package, "keywords", &keywords(providers)?)?;
    let from = package
        .find("\"description\": \"")
        .with_context(|| format!("{rel}: no description"))?;
    let end = from
        + package[from..]
            .find("\",")
            .context("unterminated description")?
        + 2;
    package.replace_range(
        from..end,
        &format!("\"description\": \"{DESCRIPTION}\","),
    );
    // `sudsy init` writes per-language editor settings from this: one line per crate, its ids.
    let table: String = ids_by_crate(providers)?
        .iter()
        .map(|(krate, ids)| format!("{krate} {}\n", ids.join(" ")))
        .collect();
    Ok(vec![
        (rel.into(), package),
        ("editors/vscode/src/selector.generated.ts".into(), selector),
        ("crates/languages/src/vscode_ids.txt".into(), table),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every generated id is a language id VS Code or a common extension registers: an internal id (`js`, `shell`) never leaks.
    #[test]
    fn generated_ids_are_real_vscode_ids() {
        let real: Vec<&str> = include_str!("../data/vscode-language-ids.txt")
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        let ids = language_ids(&languages::providers()).unwrap();
        let unknown: Vec<&String> = ids.iter().filter(|i| !real.contains(&i.as_str())).collect();
        assert!(unknown.is_empty(), "not VS Code language ids: {unknown:?}");
    }

    /// `formatter` and `linter` always stay; only configuration formats leave when the limit is hit.
    #[test]
    fn generic_keywords_stay_and_only_config_formats_leave() {
        let providers = languages::providers();
        let words = keywords(&providers).unwrap();
        assert!(words.len() <= MAX_KEYWORDS);
        for g in GENERIC_KEYWORDS {
            assert!(words.iter().any(|w| w == g), "{g} missing");
        }
        for p in own_language(&providers) {
            for part in p.grammar().title.split('/') {
                assert!(
                    words.iter().any(|w| w == part) || CONFIG_FORMATS.contains(&part),
                    "programming language {part} dropped from the keywords"
                );
            }
        }
    }
}
