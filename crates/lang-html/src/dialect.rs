//! The providers of the HTML family: HTML, Vue, Svelte and Angular share one tree
//! (`grammars/html.grammar`), their rules and the printer; each has its own file matchers, parse
//! mode and formatter, and `vue`, `svelte` and `angular` config sections on top of `html`.
//!
//! concept: none (glue: dialect providers)

use crate::format::{
    ANGULAR_FORMATTER, INDENT_SCRIPT_AND_STYLE, SVELTE_FORMATTER, VUE_FORMATTER, option_schema,
};
use crate::parser::{self, Mode};
use core_config::{OptionDef, OptionSchema};
use core_format::NodeFormatter;
use provider_api::{ParserProvider, ParserSpec};

/// The options of HTML and, after them, `extra`, with HTML's presets.
fn schema(extra: Vec<OptionDef>) -> OptionSchema {
    option_schema().extended(extra)
}

fn spec(
    extensions: &'static [&'static str],
    file_names: &'static [&'static str],
    schema: OptionSchema,
    formatter: &'static dyn NodeFormatter,
) -> ParserSpec {
    ParserSpec {
        crate_name: env!("CARGO_PKG_NAME"),
        grammar: core_syntax::grammars::HTML,
        extensions,
        file_names,
        schema,
        formatter,
        hooks: &crate::hooks::HtmlHooks,
        rules: crate::rules::RULES,
        pattern_files: provider_api::pattern_files!(),
        families: crate::rules::FAMILIES,
    }
}

/// `.html` and `.htm` files.
pub fn provider() -> ParserProvider {
    let options = vec![OptionDef::choice(
        core_config::opt::DIALECT,
        &["auto", "html", "template", "template-open"],
        "auto",
        "`template`: the file is a template whose `{{ }}` actions (Go templates, Hugo) stay as written; a block of actions (`if` ... `end`) must open and close inside one element (a file that does not is not formatted); an action among the attributes is one attribute, kept as written. `template-open`: as `template`, and a block may span elements (the file is laid out by the tree of its markup).",
    )];
    let spec = spec(
        &["html", "htm"],
        &[],
        schema(options),
        &crate::HtmlFormatter,
    );
    ParserProvider::new(spec, |f| {
        let dialect = f.language_version.as_deref().unwrap_or_default();
        let templates = dialect.starts_with("template");
        parser::parse_with(&f.text, Mode::Html, templates, dialect != "template-open")
    })
}

/// `.vue` single-file components.
pub fn vue_provider() -> ParserProvider {
    let options = vec![OptionDef::boolean(
        INDENT_SCRIPT_AND_STYLE,
        false,
        "Indent the content of the root `<script>` and `<style>` blocks of a `.vue` file by one level.",
    )];
    let spec = spec(&["vue"], &[], schema(options), &VUE_FORMATTER);
    ParserProvider::new(spec, |f| parser::parse(&f.text, Mode::Vue)).with_dialect("vue", "html")
}

/// `.svelte` components.
pub fn svelte_provider() -> ParserProvider {
    let options = vec![OptionDef::boolean(
        INDENT_SCRIPT_AND_STYLE,
        true,
        "Indent the content of the root `<script>` and `<style>` blocks of a `.svelte` file by one level.",
    )];
    let spec = spec(&["svelte"], &[], schema(options), &SVELTE_FORMATTER);
    ParserProvider::new(spec, |f| parser::parse(&f.text, Mode::Svelte))
        .with_dialect("svelte", "html")
}

/// Angular component templates (`*.component.html`): control flow blocks, `@let`, ICU messages.
pub fn angular_provider() -> ParserProvider {
    let spec = spec(
        &[],
        &["*.component.html"],
        option_schema(),
        &ANGULAR_FORMATTER,
    );
    ParserProvider::new(spec, |f| parser::parse(&f.text, Mode::Angular))
        .with_dialect("angular", "html")
}
