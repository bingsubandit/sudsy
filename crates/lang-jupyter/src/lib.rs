//! Jupyter notebooks (`.ipynb`): JSON of `lang-json`'s grammar and parser, whose code cells hold
//! the code of the notebook's kernel language. A notebook is laid out by its writer, so the
//! formatter keeps the text as written and the cells are formatted in place as embedded Python
//! (IPython syntax, the final `;` of a cell kept).
//!
//! concept: none (glue: provider entry)

use core_config::{OptionDef, OptionSchema, opt};
use core_format::verbatim::VerbatimFormatter;
use core_rules::embed_keys::{KeyEmbed, KeyEmbeds};
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

/// Cell magics that keep a cell Python: `%%time` and its kin run the cell's code; any other `%%`
/// line makes the cell a program of another language (`%%bash`).
const CELL_MAGIC: &str = r"^\s*%%\S";
const PYTHON_CELL_MAGIC: &str =
    r"^\s*%%(?:capture|debug|ipytest|prun|pypy|python|python3|time|timeit)(?:\s|$)";

/// The `source` of the code cells of a notebook whose kernel language is Python (the language
/// word is `language_info.name`, else `kernelspec.language`, else Python when there is no `kernelspec`).
static CELLS: KeyEmbeds = KeyEmbeds {
    rows: &[KeyEmbed {
        path: "cells.*.source",
        words: &[
            "metadata.language_info.name",
            "metadata.kernelspec.language",
        ],
        guess: None,
        language: "python",
        when: Some(("cells.$1.cell_type", "code")),
        settings: &[
            ("dialect", "ipython"),
            ("keep_final_semicolon", "true"),
            ("keep_statement_parentheses", "true"),
            ("module_docstring", "false"),
        ],
        unless: Some("metadata.kernelspec"),
    }],
    languages: &[(&["python"], "python")],
    carve: "",
    plain: "",
    block_header: "",
    literal: "",
    escaped: true,
    format: true,
    foreign: Some((CELL_MAGIC, PYTHON_CELL_MAGIC)),
};

pub fn option_schema() -> OptionSchema {
    lang_json::option_schema().extended(vec![OptionDef::choice(
        opt::EMBEDDED_LANGUAGE_FORMATTING,
        &["auto", "off"],
        "auto",
        "Format the code cells with the formatter of the notebook's kernel language (`auto`), or keep them as written (`off`). A cell that does not parse, a `%%` cell magic other than Python's and a notebook of another kernel language are kept either way.",
    )])
}

pub fn provider() -> ParserProvider {
    let spec = ParserSpec {
        crate_name: env!("CARGO_PKG_NAME"),
        grammar: grammars::JSON,
        extensions: &["ipynb"],
        file_names: &[],
        schema: option_schema(),
        formatter: &VerbatimFormatter { exact_end: &[] },
        hooks: &core_rules::DEFAULT_HOOKS,
        rules: &[],
        pattern_files: &[],
        families: &[],
    };
    ParserProvider::new(spec, lang_json::parse_file)
        .with_dialect("jupyter", "json")
        .with_key_embeds(&CELLS)
}
