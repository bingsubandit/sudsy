//! The nesting limit and the per-file failure: every language's file nested 20,000 deep is one
//! failure (exit 2) and the other files are still processed.

use std::fs;
use std::process::Command;

const DEPTH: usize = 20_000;

/// `(language, file name, text before, opener, closer)`: the file is `before`, then `DEPTH`
/// openers and as many closers. Every provider needs a row.
const NESTED: &[(&str, &str, &str, &str, &str)] = &[
    ("json", "a.json", "", "[", "]"),
    ("jupyter", "a.ipynb", "", "[", "]"),
    ("css", "a.css", "a { width: ", "calc(", ")"),
    ("scss", "a.scss", "a { width: ", "calc(", ")"),
    ("less", "a.less", "a { width: ", "calc(", ")"),
    ("cpp", "a.cpp", "int x = ", "(", ")"),
    ("c", "a.c", "int x = ", "(", ")"),
    ("dockerfile", "Dockerfile", "RUN ", "$(", ")"),
    ("editorconfig", ".editorconfig", "a = ", "[", "]"),
    ("gitignore", ".gitignore", "", "[", "]"),
    ("go", "a.go", "package a; var x = ", "(", ")"),
    ("graphql", "a.graphql", "query ", "{", "}"),
    ("hcl", "a.tf", "a = ", "[", "]"),
    ("html", "a.html", "<p>", "(", ")"),
    ("ini", "a.ini", "a = ", "[", "]"),
    ("java", "A.java", "class A { int x = ", "(", ")"),
    ("js", "a.js", "x = ", "[", "]"),
    ("kotlin", "a.kt", "val x = ", "(", ")"),
    ("makefile", "Makefile", "x = ", "$(", ")"),
    ("markdown", "a.md", "", "[", "]"),
    ("php", "a.php", "<?php $x = ", "(", ")"),
    ("powershell", "a.ps1", "$x = ", "(", ")"),
    ("protobuf", "a.proto", "message A ", "{", "}"),
    ("python", "a.py", "x = ", "(", ")"),
    ("ruby", "a.rb", "x = ", "(", ")"),
    ("shell", "a.sh", "x=", "$(", ")"),
    ("sql", "a.sql", "select ", "(", ")"),
    ("toml", "a.toml", "a = ", "[", "]"),
    ("xml", "a.xml", "<a>", "(", ")"),
    ("yaml", "a.yaml", "a: ", "[", "]"),
    ("github-actions", ".github/workflows/a.yml", "a: ", "[", "]"),
    ("angular", "a.component.html", "<p>", "(", ")"),
    ("vue", "a.vue", "<template>", "(", ")"),
    ("svelte", "a.svelte", "", "{", "}"),
    ("csharp", "A.cs", "class A { int x = ", "(", ")"),
    ("cmake", "CMakeLists.txt", "f", "(", ")"),
    ("batch", "a.bat", "", "(", ")"),
];

#[test]
fn nested_files_fail_alone_in_every_language() {
    let providers = languages::providers();
    let missing: Vec<_> = (providers.iter().map(|p| p.language()))
        .filter(|l| !NESTED.iter().any(|row| row.0 == *l))
        .collect();
    assert!(missing.is_empty(), "NESTED has no row for {missing:?}");
    let t = tempfile::tempdir().unwrap();
    for &(language, name, before, open, close) in NESTED {
        let path = t.path().join(name);
        let (provider, _) = provider_api::select_provider(&providers, &path)
            .unwrap_or_else(|| panic!("{name} matches no provider"));
        assert_eq!(provider.language(), language, "{name}");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = format!("{before}{}{}\n", open.repeat(DEPTH), close.repeat(DEPTH));
        fs::write(&path, text).unwrap();
    }
    fs::write(t.path().join("ok.json"), "{\"a\":   1}\n").unwrap();
    for command in ["format", "lint"] {
        let mut run = Command::new(env!("CARGO_BIN_EXE_sudsy"));
        run.arg(command).current_dir(t.path());
        if command == "format" {
            run.arg("--check");
        }
        let out = run.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{command}: {stderr}");
        let failures = stderr.matches("nesting is deeper than").count();
        assert_eq!(failures, NESTED.len(), "{command}: {stderr}");
        if command == "format" {
            assert!(stdout.contains("would reformat ok.json"), "{stdout}");
        }
    }
}
