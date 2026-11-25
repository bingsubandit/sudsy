use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

fn provider() -> impl provider_api::LanguageProvider {
    lang_batch::provider()
}

#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(provider(), "a.bat", input)
}

#[test]
fn file_names() {
    assert_file_names(&provider(), &["a.bat", "x/b.CMD"], &["a.sh", "bat"]);
}

#[test]
fn a_label_in_a_block_keeps_its_place() {
    let out = fmt("if a==b (\n\t:help\n\techo x\n)\n");
    assert_eq!(out, "if a==b (\n\t:help\n\techo x\n)\n");
}

#[test]
fn deep_nesting_does_not_overflow() {
    let n = 100_000;
    for text in [
        format!("{}echo{}\n", "(".repeat(n), ")".repeat(n)),
        format!("{}echo\n", "if a==b ".repeat(n)),
        format!("{}echo\n", "for %%i in (a) do ".repeat(n)),
        "(".repeat(n),
        ")".repeat(n),
    ] {
        lossless(&text);
    }
}

#[track_caller]
fn lossless(text: &str) -> provider_api::ParseResult {
    provider_api::format_testing::assert_lossless(&provider(), "a.bat", text)
}

/// The corpus files as text, as the format tests read them.
fn corpus() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/batchfile");
    let (mut files, mut dirs) = (Vec::new(), vec![root]);
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let path = e.unwrap().path();
            let ext = path.extension().and_then(|e| e.to_str());
            if path.is_dir() {
                dirs.push(path);
            } else if matches!(ext, Some("bat" | "cmd"))
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                files.push((path.display().to_string(), text));
            }
        }
    }
    files.sort();
    files
}

/// A missing `)` is one error and the tree prints back: `cargo test -p lang-batch mutants -- --ignored --nocapture`.
#[test]
#[ignore]
fn mutants() {
    let (mut tried, mut reported) = (0, 0);
    for (name, text) in corpus() {
        if !lossless(&text).errors.is_empty() {
            continue;
        }
        let Some(at) = text.rfind(')') else { continue };
        let mut mutant = text.clone();
        mutant.remove(at);
        let r = provider_api::format_testing::assert_lossless(&provider(), &name, &mutant);
        tried += 1;
        reported += usize::from(!r.errors.is_empty());
        if tried == 258 {
            break;
        }
    }
    println!("{tried} mutants, {reported} with an error");
}

/// Parse speed over the corpus: `cargo test --release -p lang-batch speed -- --ignored --nocapture`.
#[test]
#[ignore]
fn speed() {
    use provider_api::{LanguageProvider, SourceFile};
    let files = corpus();
    let bytes: usize = files.iter().map(|f| f.1.len()).sum();
    let p = provider();
    let start = std::time::Instant::now();
    for _ in 0..10 {
        for (name, text) in &files {
            std::hint::black_box(p.parse(&SourceFile::new(name, 1, text)).unwrap());
        }
    }
    let secs = start.elapsed().as_secs_f64();
    println!(
        "{} files, {bytes} bytes, {:.1} MB/s",
        files.len(),
        bytes as f64 * 10.0 / secs / 1e6
    );
}

#[test]
fn caret_at_the_end_of_a_quoted_line() {
    // A `^` that starts its own line joins nothing to the line before.
    fmt("set \"u=!u!^\r\n\r\n^\r\nx\r\n");
}

#[track_caller]
fn fmt_with(input: &str, options: &[(&'static str, core_config::OptionValue)]) -> String {
    let subject = provider_api::format_testing::Formatting {
        options: options.to_vec(),
        ..provider_api::format_testing::Formatting::new(provider(), "a.bat")
    };
    core_format::testing::assert_formats(&subject, input)
}

fn text(v: &str) -> core_config::OptionValue {
    core_config::OptionValue::String(v.into())
}

#[test]
fn command_words_and_keywords_go_lower_and_nothing_else() {
    assert_eq!(
        fmt(
            "@ECHO OFF\nSET A=B\nIF /I \"%A%\" EQU \"b\" ECHO Hi\nREM X\nFOR %%I IN (A B) DO ECHO %%I\nGoTo :Done\n:Done\n"
        ),
        "@echo OFF\nset A=B\nif /I \"%A%\" EQU \"b\" echo Hi\nrem X\nfor %%I in (A B) do echo %%I\ngoto :Done\n:Done\n"
    );
    // An external program, a label and a variable keep their case.
    assert_eq!(fmt("Tool.EXE %PATH%\n"), "Tool.EXE %PATH%\n");
}

#[test]
fn blanks_that_are_content_stay() {
    assert_eq!(
        fmt("set x=y \necho x \nset /p a=Hi  \n"),
        "set x=y \necho x \nset /p a=Hi  \n"
    );
    assert_eq!(fmt("echo a   b\n"), "echo a   b\n");
    assert_eq!(fmt("cd .. \ndir  /b  \n"), "cd ..\ndir  /b\n");
}

#[test]
fn blocks_else_and_closing_parenthesis() {
    assert_eq!(
        fmt("IF a==b  (\n  echo x\n  )ELSE   (\n  echo y\n)\n"),
        "if a==b (\n  echo x\n) else (\n  echo y\n)\n"
    );
    assert_eq!(
        fmt("for %%i in (a b) do (\n\techo %%i\n\t)\n"),
        "for %%i in (a b) do (\n\techo %%i\n)\n"
    );
}

#[test]
fn parentheses_in_arguments_do_not_close_the_block() {
    assert_eq!(
        fmt("if a==b (\n  echo %x% (y)\n  echo (z)\n)\n"),
        "if a==b (\n  echo %x% (y)\n  echo (z)\n)\n"
    );
}

#[test]
fn a_variable_that_stands_for_if_opens_a_block() {
    fmt("%test% (\n  echo a\n)\n");
}

#[test]
fn a_redirection_in_front_of_a_block() {
    fmt("if a==b >&2 (\n  echo x\n)\n");
}

#[test]
fn rem_and_colons_comments() {
    assert_eq!(
        fmt("rem   a\nREM\nrem.\n::  b\n::c\n"),
        "rem   a\nrem\nrem.\n::  b\n:: c\n"
    );
    assert_eq!(
        fmt_with(
            "::c\n",
            &[("space_after_colons", core_config::OptionValue::Bool(false))]
        ),
        "::c\n"
    );
    // A comment swallows the rest of the line, a `)` and a `^` included.
    assert_eq!(
        fmt("if a==b (\n  rem x )\n)\n"),
        "if a==b (\n  rem x )\n)\n"
    );
}

#[test]
fn labels_stand_at_column_0_outside_blocks() {
    assert_eq!(fmt("  :a\n  echo x\n"), ":a\n  echo x\n");
}

#[test]
fn blank_lines_collapse_and_the_file_ends_with_one_newline() {
    assert_eq!(fmt("echo a\n\n\n\necho b"), "echo a\n\necho b\n");
}

#[test]
fn indent_conversion_and_continued_lines() {
    assert_eq!(
        fmt_with(
            "if a==b (\n\techo x\n)\n",
            &[("indent_conversion", text("spaces"))]
        ),
        "if a==b (\n    echo x\n)\n"
    );
    assert_eq!(
        fmt_with(
            "if a==b (\n  echo x\n)\n",
            &[
                ("indent_conversion", text("tabs")),
                ("indent_size", core_config::OptionValue::Int(2))
            ]
        ),
        "if a==b (\n\techo x\n)\n"
    );
    assert_eq!(fmt("copy a ^\n   b\n"), "copy a ^\n   b\n");
    assert_eq!(
        fmt_with("copy a ^\n   b\n", &[("continued_indent", text("none"))]),
        "copy a ^\nb\n"
    );
}

#[test]
fn line_ends_are_kept_per_file() {
    assert_eq!(fmt("echo a\r\necho  b\r\n"), "echo a\r\necho  b\r\n");
}

#[test]
fn unbalanced_parentheses_are_errors_and_lossless() {
    assert_eq!(lossless("if a==b (\n  echo x\n").errors.len(), 1);
    assert_eq!(lossless("echo x\n)\n").errors.len(), 1);
}

#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(provider(), "a.bat", "batchfile", &["bat", "cmd"]);
    println!("{unparsed:#?}");
}
