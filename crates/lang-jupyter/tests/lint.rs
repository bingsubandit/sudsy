//! The code cells of a notebook are linted as one Python program: names and imports cross cells,
//! import blocks and the last statement of a cell stand alone, and a cell may `await` and use
//! the names IPython defines.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, lint};
use provider_api::LanguageProvider;
use std::path::PathBuf;

const CONFIG: &str = "python:\n  lint:\n    rules:\n      unused-import: warn\n      undefined-variable: warn\n      redeclared-variable: warn\n      unsorted-imports: warn\n      no-useless-expression: warn\n      useless-semicolon: warn\n      yield-outside-function: warn\n      invalid-syntax: error\n";

/// `rule@line` of each finding, the line counted in the file.
fn findings(cells: &[&str]) -> Vec<String> {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_json::provider()),
        Box::new(lang_jupyter::provider()),
        Box::new(lang_python::provider()),
    ];
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(CONFIG));
    let cells: Vec<String> = cells
        .iter()
        .map(|c| {
            let lines: Vec<String> = c.split_inclusive('\n').map(|l| format!("{l:?}")).collect();
            format!(
                "{{\"cell_type\": \"code\", \"metadata\": {{}}, \"source\": [{}]}}",
                lines.join(", ")
            )
        })
        .collect();
    let text = format!(
        "{{\n\"cells\": [\n{}\n],\n\"metadata\": {{}},\n\"nbformat\": 4\n}}\n",
        cells.join(",\n")
    );
    let file = File::new(
        &db,
        PathBuf::from("a.ipynb"),
        1,
        text.clone(),
        false,
        EditorConfig::default(),
    );
    let linted = lint(&db, file).as_ref().unwrap().clone();
    let line = |offset: usize| text[..offset].matches('\n').count() + 1;
    linted
        .diagnostics
        .iter()
        .map(|d| format!("{}@{}", d.rule, line(usize::from(d.range.start()))))
        .collect()
}

#[test]
fn names_and_imports_cross_cells() {
    // each cell is one line of the file, from line 3
    let found = findings(&[
        "import os\nimport sys\n",
        "print(os.name)\nprint(display)\n",
        "import sys\n",
    ]);
    assert_eq!(found, ["redeclared-variable@5", "unused-import@5"]);
}

#[test]
fn an_import_block_ends_with_its_cell() {
    let found = findings(&["import b\nimport a\n", "import c\n\nprint(a, b, c)\n"]);
    assert_eq!(found, ["unsorted-imports@3"]);
}

#[test]
fn the_last_statement_of_a_cell_shows_its_value() {
    let found = findings(&["x = 1\nx\n", "y = 2\ny;\n", "x\ny\n"]);
    assert_eq!(found, ["no-useless-expression@5"]);
}

#[test]
fn a_cell_may_await_but_not_yield() {
    let found = findings(&["await foo()\n", "def f():\n    await g()\n"]);
    assert_eq!(
        found,
        [
            "undefined-variable@3",
            "invalid-syntax@4",
            "undefined-variable@4"
        ]
    );
}
