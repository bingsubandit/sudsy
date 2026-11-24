use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn read(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

const MESSY: &str = "{\"a\":   1,\n \"b\":[1,2]}";
const CLEAN: &str = "{\n  \"a\": 1,\n  \"b\": [1, 2]\n}\n";

#[test]
fn formats_files_in_place() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.json", MESSY);
    write(t.path(), "sub/b.json", MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read(t.path(), "a.json"), CLEAN);
    assert_eq!(read(t.path(), "sub/b.json"), CLEAN);
    let again = sudsy(t.path(), &["--check"]);
    assert_eq!(again.status.code(), Some(0));
}

#[test]
fn check_reports_and_leaves_files_alone() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.json", MESSY);
    write(t.path(), "ok.json", CLEAN);
    let out = sudsy(t.path(), &["--check"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("would reformat a.json"), "{stdout}");
    assert!(!stdout.contains("ok.json"));
    assert_eq!(read(t.path(), "a.json"), MESSY);
}

#[test]
fn respects_gitignore_skips_other_languages_and_takes_explicit_paths() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), ".gitignore", "ignored/\n");
    write(t.path(), "ignored/a.json", MESSY);
    write(t.path(), "notes.txt", "x  ");
    write(t.path(), "keep.json", MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(read(t.path(), "ignored/a.json"), MESSY);
    assert_eq!(read(t.path(), "notes.txt"), "x  ");
    assert_eq!(read(t.path(), "keep.json"), CLEAN);
    let explicit = sudsy(t.path(), &["ignored/a.json"]);
    assert_eq!(explicit.status.code(), Some(0));
    assert_eq!(read(t.path(), "ignored/a.json"), CLEAN);
}

#[test]
fn syntax_errors_fail_without_touching_the_file() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "bad.json", "{\"a\": }");
    write(t.path(), "good.json", MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bad.json") && stderr.contains("1:7"),
        "{stderr}"
    );
    assert_eq!(read(t.path(), "bad.json"), "{\"a\": }");
    assert_eq!(read(t.path(), "good.json"), CLEAN);
}

#[test]
fn config_and_dialects_apply() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "format:\n  indent_size: 3\njson:\n  format:\n    trailing_commas: multiline\n    collapse: never\n",
    );
    write(t.path(), "a.json", "{\"a\":[1]}");
    write(t.path(), "tsconfig.json", "{\"a\":[1]}");
    write(t.path(), ".vscode/settings.json", "// c\n{\"a\":1,}");
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        read(t.path(), "a.json"),
        "{\n   \"a\": [\n      1\n   ]\n}\n"
    );
    assert_eq!(
        read(t.path(), "tsconfig.json"),
        "{\n   \"a\": [\n      1,\n   ],\n}\n"
    );
    assert_eq!(
        read(t.path(), ".vscode/settings.json"),
        "// c\n{\n   \"a\": 1,\n}\n"
    );
}

#[test]
fn dialect_option_overrides_the_file_name() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "json:\n  format:\n    dialect: jsonc\n",
    );
    write(t.path(), "a.json", "[1, // one\n 2]");
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read(t.path(), "a.json"), "[\n  1, // one\n  2\n]\n");
}

#[test]
fn invalid_option_values_are_reported() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "json:\n  format:\n    collapse: sometimes\n",
    );
    write(t.path(), "a.json", "{}");
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("collapse"));
}

#[test]
fn keeps_crlf_and_byte_order_mark() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("a.json"), "\u{feff}{\"a\":\r\n1}").unwrap();
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(0));
    let bytes = fs::read(t.path().join("a.json")).unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "\u{feff}{\r\n  \"a\": 1\r\n}\r\n"
    );
}

const YAML_MESSY: &str = "a:   1
list:
    - x
    -   y
m: {b: 2}
";
const YAML_CLEAN: &str = "a: 1
list:
  - x
  - y
m: { b: 2 }
";

#[test]
fn formats_yaml_files() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.yaml", YAML_MESSY);
    write(t.path(), "sub/b.yml", YAML_MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read(t.path(), "a.yaml"), YAML_CLEAN);
    assert_eq!(read(t.path(), "sub/b.yml"), YAML_CLEAN);
    assert_eq!(sudsy(t.path(), &["--check"]).status.code(), Some(0));
}

#[test]
fn the_config_file_is_formatted_like_any_other_yaml_file() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "yaml:
    format:
        indent_size: 4
    # keep
format:   {line_width: 100}
",
    );
    write(
        t.path(),
        "a.yaml",
        "k:
  - x
",
    );
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The file's own `yaml:` section applies to it and to the other YAML files.
    assert_eq!(
        read(t.path(), "sudsy.yaml"),
        "yaml:
    format:
        indent_size: 4
    # keep
format: { line_width: 100 }
"
    );
    assert_eq!(
        read(t.path(), "a.yaml"),
        "k:
    - x
"
    );
}

#[test]
fn yaml_syntax_errors_fail_without_touching_the_file() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "bad.yaml",
        "a: b: c
",
    );
    write(t.path(), "ok.yaml", YAML_MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        read(t.path(), "bad.yaml"),
        "a: b: c
"
    );
    assert_eq!(read(t.path(), "ok.yaml"), YAML_CLEAN);
}

#[test]
fn formats_csharp_files_with_the_worker() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "Program.cs", "class  A{void M(){x();}}");
    write(t.path(), "bad.cs", "class A {");
    let out = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", lang_csharp::testing::built_worker())
        .current_dir(t.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        read(t.path(), "Program.cs"),
        "class A
{
    void M()
    {
        x();
    }
}
"
    );
    assert_eq!(read(t.path(), "bad.cs"), "class A {");
    assert!(String::from_utf8_lossy(&out.stderr).contains("bad.cs"));
}

#[test]
fn csharp_without_a_worker_fails_the_file_and_formats_the_rest() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "Program.cs", "class A{}");
    write(t.path(), "a.json", MESSY);
    let out = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", t.path().join("missing.dll"))
        .current_dir(t.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(read(t.path(), "Program.cs"), "class A{}");
    assert_eq!(read(t.path(), "a.json"), CLEAN);
}

#[test]
fn missing_dotnet_is_one_clear_error_and_other_languages_continue() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.json", MESSY);
    write(t.path(), "a.cs", "class A{}");
    write(t.path(), "b.cs", "class B{}");
    write(t.path(), "worker.dll", "");
    let out = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", t.path().join("worker.dll"))
        .env("PATH", t.path().join("empty"))
        .current_dir(t.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("Sudsy needs the .NET 10 runtime to format and lint C#. JSON and YAML keep working. Download it from https://dotnet.microsoft.com/download/dotnet/10.0"),
        "{stderr}"
    );
    assert_eq!(
        stderr.matches("Sudsy needs the .NET").count(),
        1,
        "{stderr}"
    );
    assert!(stderr.contains("skipped"), "{stderr}");
    assert_eq!(read(t.path(), "a.json"), CLEAN);
}

/// Embedded code reaches a fixed point on its own output: template literals keep their lines, an
/// interpolation that is not formatted is not indented further, an event handler that formats to
/// one expression loses its `;`.
#[test]
fn embedded_html_and_vue_formats_are_fixed_points() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "a.html",
        "<script>\n    function f() {\n        return `\n      a\n    `;\n    }\n</script>\n<button onclick=\"a.b(c => d(c))\">x</button>\n",
    );
    write(
        t.path(),
        "a.vue",
        "<template>\n<div>x{{\n\nx => {\n    const h = 'w'\n    return h;\n}\n}}</div>\n<b @click=\"[a].b(c => 1)\"></b>\n</template>\n",
    );
    assert_eq!(sudsy(t.path(), &[]).status.code(), Some(0));
    let (html, vue) = (read(t.path(), "a.html"), read(t.path(), "a.vue"));
    assert!(html.contains("`\n      a\n    `;"), "{html}");
    assert!(html.contains("onclick=\"a.b((c) => d(c))\""), "{html}");
    assert!(vue.contains("@click=\"[a].b((c) => 1)\""), "{vue}");
    assert_eq!(
        sudsy(t.path(), &["--check"]).status.code(),
        Some(0),
        "{html}{vue}"
    );
}

/// `content_newline_with_attributes` breaks the content of an element that has attributes (not an
/// inline one, not one without) and `indent_expression_operands` indents the operands of an
/// expression that continues on following lines; both are fixed points.
#[test]
fn vue_layout_options_content_lines_and_operand_indent() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "vue:\n  format:\n    content_newline_with_attributes: true\n    indent_expression_operands: true\n",
    );
    write(
        t.path(),
        "a.vue",
        "<template>\n<p>a</p>\n<p class=\"x\">b</p>\n<span class=\"x\">c</span>\n<p :class=\"cond_aaaaaaaaaaaaaaaaaaaa && cond_bbbbbbbbbbbbbbbbbbbbbb && cond_cccccccccccccccccccccc\">d</p>\n</template>\n",
    );
    assert_eq!(sudsy(t.path(), &[]).status.code(), Some(0));
    let vue = read(t.path(), "a.vue");
    assert!(vue.contains("<p>a</p>"), "{vue}");
    assert!(vue.contains("<p class=\"x\">\n    b\n  </p>"), "{vue}");
    assert!(vue.contains("<span class=\"x\">c</span>"), "{vue}");
    assert!(
        vue.contains("cond_aaaaaaaaaaaaaaaaaaaa &&\n        cond_bbbbbbbbbbbbbbbbbbbbbb"),
        "{vue}"
    );
    assert_eq!(
        sudsy(t.path(), &["--check"]).status.code(),
        Some(0),
        "{vue}"
    );
}

/// `dialect: template` keeps `{{ }}` actions as written and refuses a file whose blocks do not open
/// and close in one element, and keeps an action among the attributes as one attribute; `void_element_style`,
/// `self_closing_space` and `max_attributes_per_line` lay out the tags; all are fixed points.
#[test]
fn html_template_actions_and_tag_options() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "html:\n  format:\n    dialect: template\n    void_element_style: omit\n    self_closing_space: false\n    max_attributes_per_line: 2\n",
    );
    write(
        t.path(),
        "a.html",
        "<div>\n{{ if .x }}<br />{{ end }}\n<a href=\"{{< r \"/x\" >}}\" class=\"y\" id=\"z\">t</a><svg><use href=\"#a\" /></svg>\n</div>\n",
    );
    let open = "{{ define \"x\" }}<body>{{ end }}</body>\n";
    write(t.path(), "b.html", open);
    let spans = "{{ if .x }}<p>{{ else }}<p class=\"a\">{{ end }}</p>\n";
    write(t.path(), "c.html", spans);
    write(
        t.path(),
        "d.html",
        "<p   {{ if .y }}hidden{{ end }}>x</p>\n",
    );
    assert_ne!(sudsy(t.path(), &[]).status.code(), Some(0));
    let html = read(t.path(), "a.html");
    assert!(html.contains("{{ if .x }}<br>{{ end }}"), "{html}");
    assert!(html.contains("href='{{< r \"/x\" >}}'"), "{html}");
    assert!(html.contains("<use href=\"#a\"/>"), "{html}");
    assert!(html.contains("<a\n"), "{html}");
    assert_eq!(read(t.path(), "b.html"), open);
    let tag = read(t.path(), "d.html");
    assert!(tag.contains("{{ if .y }}\n  hidden\n  {{ end }}"), "{tag}");
    assert_eq!(read(t.path(), "c.html"), spans);
    assert_eq!(
        sudsy(t.path(), &["--check", "a.html"]).status.code(),
        Some(0),
        "{html}"
    );
}

/// `dialect: template-open` formats a file whose blocks span elements by its tree, and
/// `implied_end_tags: keep` leaves an element open at the end of the file without an end tag.
#[test]
fn html_template_open_blocks_keep_open_elements() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "html:\n  format:\n    dialect: template-open\n    implied_end_tags: keep\n    whitespace_sensitivity: ignore\n",
    );
    write(
        t.path(),
        "a.html",
        "{{ define \"x\" }}<body>{{ end }}\n{{ define \"y\" }}<p>a</p>{{ end }}\n",
    );
    assert_eq!(sudsy(t.path(), &[]).status.code(), Some(0));
    let html = read(t.path(), "a.html");
    assert!(!html.contains("</body>"), "{html}");
    assert!(html.contains("<body>\n  {{ end }}"), "{html}");
    assert_eq!(
        sudsy(t.path(), &["--check", "a.html"]).status.code(),
        Some(0),
        "{html}"
    );
}

/// The safety check accepts what the formatter rewrites in Vue and Svelte: the values of `.prop` and
/// `slot-scope` attributes, root custom blocks without blanks between them, and a destructuring
/// pattern in a block tag (`}}`).
#[test]
fn vue_and_svelte_rewrites_pass_the_safety_check() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "a.vue",
        "<template>\n<b .p=\"  a &&b \" slot-scope=\"  {row  }\"></b>\n</template>\n<docs lang=x></docs><docs lang=x></docs>\n",
    );
    write(
        t.path(),
        "a.svelte",
        "{#each a as {  b, c }}\n  {b}\n{/each}\n{#await p then {  d }}\n  {d}\n{/await}\n",
    );
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(read(t.path(), "a.vue").contains("</docs>\n<docs"));
    assert!(read(t.path(), "a.svelte").contains("as { b, c }}"));
}

/// The summary counts a file the engine mostly copied through instead of laying out; the parity
/// runner's stability score reads it.
#[test]
fn summary_counts_files_copied_through() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "html.php", "<p>\n<?php echo 1 ?>\n</p>\n");
    let out = sudsy(t.path(), &[]);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("1 copied through"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    write(t.path(), "html.php", "<?php\n$x   = 1;\n");
    let out = sudsy(t.path(), &[]);
    assert!(!String::from_utf8_lossy(&out.stderr).contains("copied through"));
}
