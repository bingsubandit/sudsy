//! Where Python imports come from, for the sections of an isort block (`core_rules::imports`).
//!
//! concept: none (data: module sources)

use core_rules::imports::ImportLang;

pub struct Python;

impl ImportLang for Python {
    const STDLIB: &'static str = include_str!("../globals/stdlib.txt");
    const FUTURE: &'static str = "__future__";
    const MARKERS: &'static [&'static str] = &[
        "pyproject.toml",
        "ruff.toml",
        ".ruff.toml",
        "sudsy.yaml",
        ".git",
    ];
    const SOURCE_DIRS: &'static [&'static str] = &["src"];
    const EXTENSION: &'static str = "py";
}
