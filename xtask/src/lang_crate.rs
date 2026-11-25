//! The provider glue and `Cargo.toml` of the tree-sitter language crates, from
//! `crates/lang-<name>/lang.yaml` (keys: docs/ai/architecture/adding-a-language.md).

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn list(v: &Value, key: &str) -> Vec<String> {
    let items = v.get(key).and_then(Value::as_array);
    let items = items.into_iter().flatten();
    items
        .map(|s| s.as_str().unwrap_or_default().to_string())
        .collect()
}

pub fn text<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn workspace_deps(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|d| match d.starts_with("lang-") {
            // A language crate used by another (a test's companion) links its grammar.
            true => format!("{d} = {{ workspace = true, features = [\"static-grammars\"] }}"),
            false => format!("{d}.workspace = true"),
        })
        .collect()
}

/// A `lang.yaml` language: its crate directory name and the parsed manifest.
pub struct Lang {
    pub krate: String,
    pub yaml: Value,
}

/// Every `crates/*/lang.yaml`, by crate name.
pub fn load(root: &Path) -> Result<Vec<Lang>> {
    let mut langs = Vec::new();
    for entry in std::fs::read_dir(root.join("crates"))? {
        let dir = entry?.path();
        let manifest = dir.join("lang.yaml");
        if !manifest.exists() {
            continue;
        }
        let krate = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let src = std::fs::read_to_string(&manifest)?;
        let yaml = serde_saphyr::from_str(&src)
            .with_context(|| format!("parsing {}", manifest.display()))?;
        langs.push(Lang {
            krate: krate.into(),
            yaml,
        });
    }
    langs.sort_by(|a, b| a.krate.cmp(&b.krate));
    no_duplicate_matchers(&langs)?;
    Ok(langs)
}

/// Two languages claiming the same extension or file name tie in `select_provider_index`.
fn no_duplicate_matchers(langs: &[Lang]) -> Result<()> {
    let mut seen = std::collections::HashMap::new();
    for l in langs {
        for key in ["extensions", "file-names"] {
            for m in list(&l.yaml, key) {
                if let Some(first) = seen.insert((key, m.clone()), &l.krate) {
                    anyhow::bail!("{first} and {} both claim {key} entry `{m}`", l.krate);
                }
            }
        }
    }
    Ok(())
}

/// The languages with a tree-sitter grammar; the others have a `lang.yaml` for their options
/// (and a `parse` function for a generated provider, see `adapter_provider`).
pub fn tree_sitter(langs: &[Lang]) -> Vec<&Lang> {
    let ts = |l: &&Lang| l.yaml.get("grammar-crate").is_some();
    langs.iter().filter(ts).collect()
}

/// The languages whose `provider()` is generated.
pub fn generated(langs: &[Lang]) -> Vec<&Lang> {
    let has = |l: &&Lang| l.yaml.get("grammar-crate").is_some() || l.yaml.get("parse").is_some();
    langs.iter().filter(has).collect()
}

pub fn generate(langs: &[Lang]) -> Result<Vec<(PathBuf, String)>> {
    let mut files = Vec::new();
    for l in langs {
        let Lang { krate, yaml } = l;
        let rel = PathBuf::from("crates").join(krate);
        let name = krate.trim_start_matches("lang-");
        if yaml.get("grammar-crate").is_some() {
            files.push((rel.join("Cargo.toml"), cargo_toml(krate, yaml)));
            let consts = crate::mapping::consts(name, l)?;
            files.push((
                rel.join("src/provider.generated.rs"),
                provider(name, yaml) + &consts,
            ));
        }
        if yaml.get("parse").is_some() {
            files.push((
                rel.join("src/provider.generated.rs"),
                adapter_provider(name, yaml) + &limits(yaml),
            ));
        }
        if let Some(schema) = crate::options::generate(name, l)? {
            files.push((rel.join("src/schema.generated.rs"), schema));
        }
    }
    Ok(files)
}

fn cargo_toml(krate: &str, v: &Value) -> String {
    let grammar = text(v, "grammar-crate", "");
    let mut names: Vec<String> = [
        "core-config",
        "core-format",
        "core-packs",
        "core-rules",
        "core-syntax",
        "core-treesitter",
        "provider-api",
    ]
    .map(String::from)
    .into();
    names.extend(list(v, "deps"));
    let mut deps = workspace_deps(&names);
    deps.push(format!(
        "{grammar} = {{ workspace = true, optional = true }}"
    ));
    let extra = list(v, "extra-grammars");
    deps.extend(
        extra
            .iter()
            .map(|g| format!("{g} = {{ workspace = true, optional = true }}")),
    );
    deps.sort();
    let mut out = format!(
        "# Generated from lang.yaml by `cargo xtask codegen`.\n[package]\nname = \"{krate}\"\nautotests = false\nedition.workspace = true\nlicense.workspace = true\nversion.workspace = true\npublish.workspace = true\nbuild = \"../../build/lang-crate.rs\"\n\n[dependencies]\n{}\n",
        deps.join("\n")
    );
    let dev = list(v, "dev-deps");
    if !dev.is_empty() {
        out += &format!(
            "\n[dev-dependencies]\n{}\n",
            workspace_deps(&dev).join("\n")
        );
    }
    out + &format!(
        "\n[build-dependencies]\ncore-syntax.workspace = true\n\n[features]\n# The grammar is linked in; without it the language loads from its language pack.\ndefault = [\"static-grammars\"]\nstatic-grammars = [{}]\n\n[lints]\nworkspace = true\n\n[[test]]\nname = \"integration\"\npath = \"tests/main.rs\"\n",
        std::iter::once(grammar)
            .chain(extra.iter().map(String::as_str))
            .map(|g| format!("\"dep:{g}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The static of a language with `symbols: true` (its `symbols.rows`) and the call that hands it
/// to the provider.
fn symbols(v: &Value) -> (&'static str, &'static str) {
    match v.get("symbols").and_then(Value::as_bool) {
        Some(true) => (
            "static SYMBOLS: provider_api::SymbolLang = provider_api::SymbolLang::new(include_str!(\"../symbols.rows\"));\n\n",
            ".with_symbols(&SYMBOLS)",
        ),
        _ => ("", ""),
    }
}

/// `format-rows` (`source-indent`, `exact-end`, `keep-comment-gap`): the static `FORMAT` of a language
/// that is written by the rows of its `format.rules` alone.
fn format_rows(name: &str, v: &Value) -> String {
    let Some(f) = v.get("format-rows") else {
        return String::new();
    };
    let flag = |key: &str| f.get(key).and_then(Value::as_bool).unwrap_or(false);
    format!(
        "static FORMAT: core_format::rules::LazyRules = core_format::rules::LazyRules::new(|| grammars::{}.get(), include_str!(\"../format.rules\")){}.facts(core_format::rules::RuleFacts {{ exact_end: &{:?}, keep_comment_gap: {}, ..core_format::rules::RuleFacts::DEFAULT }}){}{};\n\n",
        name.to_uppercase(),
        if flag("source-indent") {
            ".source_indent()"
        } else {
            ""
        },
        list(f, "exact-end"),
        flag("keep-comment-gap"),
        blanks(f),
        indents(f),
    )
}

/// `blanks` of `format-rows`: the `.blanks(..)` builder, or nothing.
fn blanks(f: &Value) -> String {
    let Some(b) = f.get("blanks") else {
        return String::new();
    };
    let field = |name: &str, key: &str| format!("{name}: &{:?}, ", list(b, key));
    format!(
        ".blanks(core_format::stream_blank::BlankFacts {{ {}{}{}{}{}{}{}{}{}{} }})",
        field("statements", "statements"),
        field("guard", "guard"),
        field("jump", "jump"),
        field("jump_words", "jump-words"),
        field("branch", "branch"),
        field("directives", "directives"),
        field("modifiers", "modifiers"),
        field("magic", "magic"),
        field("body", "body"),
        field("defs", "defs"),
    )
}

/// `indents` of `format-rows`: the `.indents(..)` builder, or nothing.
fn indents(f: &Value) -> String {
    let Some(b) = f.get("indents") else {
        return String::new();
    };
    let field = |name: &str, key: &str| format!("{name}: &{:?}, ", list(b, key));
    format!(
        ".indents(core_format::stream_indent::IndentFacts {{ {}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{} }})",
        field("statements", "statements"),
        field("frames", "frames"),
        field("operations", "operations"),
        field("conditions", "conditions"),
        field("opaque", "opaque"),
        field("pairs", "pairs"),
        field("chains", "chains"),
        field("leaders", "leaders"),
        field("attached", "attached"),
        field("keyword_frames", "keyword-frames"),
        field("branches", "branches"),
        field("closers", "closers"),
        field("heredoc_openers", "heredoc-openers"),
        field("heredoc_bodies", "heredoc-bodies"),
        field("heredoc_wrappers", "heredoc-wrappers"),
        field("open_brackets", "open-brackets"),
        field("close_brackets", "close-brackets"),
        field("elements", "elements"),
    )
}

fn provider(name: &str, v: &Value) -> String {
    let grammar = text(v, "grammar-crate", "").replace('-', "_");
    let language = text(v, "language-const", "LANGUAGE");
    let upper = name.to_uppercase();
    // A `mapping` map is generated (`mapping.rs`); a string names a hand-written `Mapping`.
    let data = v.get("mapping").is_some_and(Value::is_object);
    let mapping = match data {
        true => "MAPPING",
        false => text(v, "mapping", "mapping::MAPPING"),
    };
    let composite = match v.get("composite") {
        Some(Value::Object(_)) => Some("COMPOSITE"),
        c => c.and_then(Value::as_str),
    };
    let imports = format!(
        "{}{}Spec, {}",
        if v.get("composite").is_some_and(Value::is_object) {
            "Composite, "
        } else {
            ""
        },
        if data { "Mapping, " } else { "" },
        if composite.is_some() {
            "provider_of_composite"
        } else {
            "provider_of"
        },
    );
    let hooks = text(v, "hooks", "core_rules::DEFAULT_HOOKS");
    let validate = v
        .get("validate")
        .and_then(Value::as_str)
        .map_or("None".into(), |f| format!("Some({f})"));
    let strings = |key| {
        format!("{:?}", list(v, key))
            .replace('[', "")
            .replace(']', "")
    };
    let spec = format!(
        "Spec {{ crate_name: env!(\"CARGO_PKG_NAME\"), language: || core_packs::language!({name:?}, {grammar}::{language}), grammar: grammars::{upper}, mapping: &{mapping}, extensions: &[{}], file_names: &[{}], schema: option_schema(), formatter: &{}, hooks: &{hooks}, rules: {}, pattern_files: provider_api::pattern_files!(), families: {}, validate: {validate} }}",
        strings("extensions"),
        strings("file-names"),
        text(v, "formatter", ""),
        text(v, "rules", "rules::RULES"),
        text(v, "families", "&[]"),
    );
    let body = match composite {
        Some(c) => format!("provider_of_composite({spec}, Some({c}))"),
        None => format!("provider_of({spec})"),
    };
    let contexts: Vec<String> = v
        .get("snippet-contexts")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    let pair = |i| r.get(i).and_then(Value::as_str).unwrap_or_default();
                    format!("({:?}, {:?})", pair(0), pair(1))
                })
                .collect()
        })
        .unwrap_or_default();
    let body = if contexts.is_empty() {
        body
    } else {
        format!("{body}.with_snippet_contexts(&[{}])", contexts.join(", "))
    };
    let (statics, with) = symbols(v);
    format!(
        "// Generated from lang.yaml by `cargo xtask codegen`.\nuse core_syntax::grammars;\nuse core_treesitter::{{{imports}}};\nuse provider_api::ParserProvider;\n\ncore_syntax::include_generated!();\n\n{statics}#[rustfmt::skip]\npub fn provider() -> ParserProvider {{\n    {body}{with}\n}}\n"
    )
}

/// The provider of a language with a `parse` function (`fn(&SourceFile) -> ParseResult`) in
/// place of a tree-sitter grammar.
/// `LIMITS` for a `limits: {stack, bytes, depth, run, tagged: [mark, n]}` map: what the parser of
/// an adapter that recurses is given (`provider_api::adapter::guarded`).
fn limits(v: &Value) -> String {
    let Some(Value::Object(m)) = v.get("limits") else {
        return String::new();
    };
    let num = |k: &str| m.get(k).and_then(Value::as_u64).unwrap_or(0);
    let tagged = match m.get("tagged").and_then(Value::as_array) {
        Some(t) => format!(
            "Some((b{:?}, {}))",
            t[0].as_str()
                .unwrap_or_default()
                .chars()
                .next()
                .unwrap_or(' '),
            t[1].as_u64().unwrap_or(0)
        ),
        None => "None".to_string(),
    };
    format!(
        "\npub const LIMITS: core_syntax::scan::Limits = core_syntax::scan::Limits::new({}, {}, {}, {:?}, {tagged});\n",
        num("stack"),
        num("bytes"),
        num("depth"),
        text(v.get("limits").unwrap_or(v), "run", ""),
    )
}

fn adapter_provider(name: &str, v: &Value) -> String {
    let hooks = text(v, "hooks", "core_rules::DEFAULT_HOOKS");
    // `exclude-names`: file names (patterns) that the extensions and names do not claim.
    let excluded = list(v, "exclude-names");
    let without = if excluded.is_empty() {
        String::new()
    } else {
        format!(".without(&{excluded:?})")
    };
    let strings = |key| {
        format!("{:?}", list(v, key))
            .replace('[', "")
            .replace(']', "")
    };
    let (statics, with) = symbols(v);
    let statics = format!("{statics}{}", format_rows(name, v));
    format!(
        "// Generated from lang.yaml by `cargo xtask codegen`.\nuse core_syntax::grammars;\nuse provider_api::{{ParserProvider, ParserSpec}};\n\ncore_syntax::include_generated!();\n\n{statics}#[rustfmt::skip]\npub fn provider() -> ParserProvider {{\n    ParserProvider::new(ParserSpec {{ crate_name: env!(\"CARGO_PKG_NAME\"), grammar: grammars::{}, extensions: &[{}], file_names: &[{}], schema: option_schema(), formatter: &{}, hooks: &{hooks}, rules: {}, pattern_files: provider_api::pattern_files!(), families: {} }}, {}){without}{with}\n}}\n",
        name.to_uppercase(),
        strings("extensions"),
        strings("file-names"),
        text(v, "formatter", ""),
        text(v, "rules", "rules::RULES"),
        text(v, "families", "&[]"),
        text(v, "parse", ""),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(krate: &str, yaml: &str) -> Lang {
        Lang {
            krate: krate.into(),
            yaml: serde_saphyr::from_str(yaml).unwrap(),
        }
    }

    #[test]
    fn duplicate_matchers_name_both_languages() {
        let langs = [
            lang("lang-a", "extensions: [x]"),
            lang("lang-b", "extensions: [x]"),
        ];
        let err = no_duplicate_matchers(&langs).unwrap_err().to_string();
        assert!(err.contains("lang-a") && err.contains("lang-b"), "{err}");
        let ok = [
            lang("lang-a", "extensions: [x]"),
            lang("lang-b", "file-names: [x]"),
        ];
        assert!(no_duplicate_matchers(&ok).is_ok());
    }

    /// The cap on the committed bench inputs of one language (docs/ai/performance/tooling.md).
    #[test]
    fn bench_inputs_stay_under_150_kb_per_language() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bench/inputs");
        for lang in std::fs::read_dir(dir).unwrap() {
            let lang = lang.unwrap().path();
            let size = walk(&lang);
            assert!(size <= 150_000, "{} is {size} bytes", lang.display());
        }
    }

    fn walk(dir: &Path) -> u64 {
        let files = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path());
        files
            .map(|p| {
                if p.is_dir() {
                    walk(&p)
                } else {
                    p.metadata().unwrap().len()
                }
            })
            .sum()
    }
}
