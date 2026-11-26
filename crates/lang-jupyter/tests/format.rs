//! A notebook is formatted in its code cells only: every byte outside the `source` values stays as
//! written, and a `source` keeps its form (a list of lines, or one string), its escapes and its
//! indentation.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, format};
use provider_api::LanguageProvider;
use std::path::PathBuf;

fn formatted(text: &str) -> String {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_json::provider()),
        Box::new(lang_jupyter::provider()),
        Box::new(lang_python::provider()),
    ];
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(""));
    let file = File::new(
        &db,
        PathBuf::from("a.ipynb"),
        1,
        text.into(),
        false,
        EditorConfig::default(),
    );
    format(&db, file).as_ref().unwrap().clone()
}

fn notebook(language: &str, cells: &str) -> String {
    format!(
        "{{\n \"cells\": [{cells}],\n \"metadata\": {{\"kernelspec\": {{\"language\": \"{language}\"}}}},\n \"nbformat\": 4\n}}\n"
    )
}

fn cell(kind: &str, source: &str) -> String {
    format!(
        "\n  {{\n   \"cell_type\": \"{kind}\",\n   \"metadata\": {{}},\n   \"source\": {source}\n  }}"
    )
}

#[test]
fn code_cells_are_formatted_and_everything_else_is_kept() {
    let source = "[\n    \"x=[1,2]\\n\",\n    \"y  =  {'a':1}\"\n   ]";
    let text = notebook(
        "python",
        &[cell("code", source), cell("markdown", "[\"x=[1,2]\"]")].join(","),
    );
    let want = text
        .replace("x=[1,2]\\n", "x = [1, 2]\\n")
        .replace("y  =  {'a':1}", "y = {\\\"a\\\": 1}");
    let out = formatted(&text);
    assert_eq!(out, want);
    assert_eq!(formatted(&out), out);
}

#[test]
fn a_string_source_keeps_its_form_and_escapes() {
    let text = notebook("python", &cell("code", "\"x=1\\ny  = '\\u00e9'\""));
    assert_eq!(
        formatted(&text),
        text.replace("x=1", "x = 1")
            .replace("y  = '\\u00e9'", "y = \\\"\\u00e9\\\"")
    );
}

#[test]
fn a_list_of_one_line_grows_into_lines_of_the_files_indent() {
    let text = notebook("python", &cell("code", "[\"a=1;b=2\"]"));
    assert_eq!(
        formatted(&text),
        text.replace("[\"a=1;b=2\"]", "[\"a = 1\\n\", \"b = 2\"]")
    );
}

#[test]
fn the_last_semicolon_of_a_cell_stays_and_ipython_syntax_is_read() {
    let text = notebook(
        "python",
        &cell("code", "[\"%matplotlib inline\\n\", \"x=f(1);\"]"),
    );
    assert_eq!(formatted(&text), text.replace("x=f(1);", "x = f(1);"));
}

#[test]
fn cells_that_are_not_python_stay_as_written() {
    for source in [
        "[\"%%bash\\n\", \"x=1\"]",
        "[\"x=(\"]",
        "[\"!ls -l\\n\", \"x=1\"]",
    ] {
        let text = notebook("python", &cell("code", source));
        let out = formatted(&text);
        // (a cell that does not parse, a cell magic of another language)
        assert!(out == text || out.contains("x = 1"), "{out}");
        if source.contains("%%bash") || source.contains("x=(") {
            assert_eq!(out, text);
        }
    }
    let r = notebook("R", &cell("code", "[\"x=1\"]"));
    assert_eq!(formatted(&r), r);
    let time = notebook("python", &cell("code", "[\"%%time\\n\", \"x=1\"]"));
    assert_eq!(formatted(&time), time.replace("x=1", "x = 1"));
}

#[test]
fn a_rewritten_comment_passes_the_cells_check() {
    let text = notebook("python", &cell("code", "[\"#note\\n\", \"x=1\"]"));
    assert_eq!(
        formatted(&text),
        text.replace("#note", "# note").replace("x=1", "x = 1")
    );
}

#[test]
fn a_kernelspec_without_a_language_is_not_python() {
    let text = notebook("python", &cell("code", "[\"x=1\"]"))
        .replace("{\"language\": \"python\"}", "{\"name\": \"python3\"}");
    assert_eq!(formatted(&text), text);
}
