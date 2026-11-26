//! A notebook's code cells are linted as one Python program run cell by cell: a name defined in
//! one cell is known in the next, IPython syntax is read, and the rules with per-cell semantics
//! (import blocks, the cell's output, top-level `await`) see the cells.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, lint};
use provider_api::LanguageProvider;
use std::path::PathBuf;

const CONFIG: &str = "python:
  lint:
    rules:
      invalid-syntax: warn
      no-useless-expression: warn
      unsorted-imports: warn
      unused-import: warn
      yield-outside-function: warn
      undefined-variable:
        level: warn
        options:
          globals: [display]
";

/// `line:rule` of each finding, the line counted in the notebook file.
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
        .map(|c| format!("  {{\"cell_type\": \"code\", \"metadata\": {{}}, \"source\": {c}}}"))
        .collect();
    let text = format!(
        "{{\"cells\": [\n{}\n],\n \"metadata\": {{\"kernelspec\": {{\"language\": \"python\"}}}},\n \"nbformat\": 4}}\n",
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
    let linted = lint(&db, file).as_ref().unwrap();
    let mut out: Vec<String> = linted
        .diagnostics
        .iter()
        .map(|d| {
            let line = text[..usize::from(d.range.start())].matches('\n').count() + 1;
            format!("{line}:{}", d.rule)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn names_cross_cells_and_magics_are_read() {
    // Line 2 is the first cell. `x` comes from the first cell; `display` is a global by option.
    let found = findings(&[
        r#"["%matplotlib inline\n", "x = 1"]"#,
        r#"["display(x)\n", "print(y)"]"#,
    ]);
    assert_eq!(found, ["3:undefined-variable"]);
}

#[test]
fn an_import_block_ends_with_its_cell() {
    // Sorted in each cell, though `import a` after `import b` would not be sorted in one file;
    // and the blank lines isort wants after a block are not asked for at a cell's end.
    let found = findings(&[
        r#"["import b\n", "\n", "b"]"#,
        r#"["import os"]"#,
        r#"["def f():\n", "    return os"]"#,
        r#"["import a\n", "\n", "a"]"#,
    ]);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn the_last_expression_of_a_cell_is_its_output() {
    let found = findings(&[r#"["x = 1\n", "x\n", "x"]"#, r#"["x"]"#]);
    assert_eq!(found, ["2:no-useless-expression"]);
}

#[test]
fn await_at_the_top_of_a_cell_runs_in_the_kernels_loop() {
    // A `yield` there is still outside a function.
    let found = findings(&[
        r#"["async def f():\n", "    pass\n", "await f()"]"#,
        r#"["yield 1"]"#,
    ]);
    assert_eq!(found, ["3:yield-outside-function"]);
}

#[test]
fn a_cell_that_does_not_parse_is_left_out() {
    let found = findings(&[r#"["x = (\n"]"#, r#"["print(y)"]"#]);
    assert_eq!(found, ["3:undefined-variable"]);
}
