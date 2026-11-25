//! The rules that read the symbol index, on Protobuf: an import nothing uses and a type that two
//! rpcs name, through the projects a run opens (`open_projects`).

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, lint, open_projects, reads_index};
use provider_api::LanguageProvider;
use salsa::EventKind;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

const ON: &str = "protobuf:\n  lint:\n    rules:\n      import-used: warn\n      rpc-request-response-unique: warn\n";

struct World {
    db: Database,
    dir: TempDir,
    log: Arc<Mutex<Vec<String>>>,
}

impl World {
    fn new(config: &str) -> World {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = log.clone();
        let providers: Vec<Box<dyn LanguageProvider>> = vec![
            Box::new(lang_protobuf::provider()),
            Box::new(lang_php::provider()),
        ];
        let mut db = Database::with_event_hook(
            providers,
            Box::new(move |event| {
                if let EventKind::WillExecute { database_key } = event.kind {
                    sink.lock().unwrap().push(format!("{database_key:?}"));
                }
            }),
        );
        db.set_config(ConfigChain::parse(config));
        World {
            db,
            dir: tempfile::tempdir().unwrap(),
            log,
        }
    }

    /// A file on disk and as an input.
    fn file(&self, path: &str, text: &str) -> File {
        let p = self.dir.path().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        File::new(
            &self.db,
            p,
            1,
            text.to_string(),
            false,
            EditorConfig::default(),
        )
    }

    /// `(line, message)` of the index rules' findings in the file.
    fn findings(&self, file: File) -> Vec<(usize, String)> {
        let linted = lint(&self.db, file).as_ref().unwrap();
        linted
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    &*d.rule,
                    "import-used" | "rpc-request-response-unique" | "class-not-found"
                )
            })
            .map(|d| {
                let line = file.text(&self.db)[..usize::from(d.range.start())]
                    .matches('\n')
                    .count()
                    + 1;
                (line, d.message.clone())
            })
            .collect()
    }

    fn runs(&self, query: &str) -> usize {
        let log = self.log.lock().unwrap();
        log.iter().filter(|k| k.contains(query)).count()
    }

    fn manifest(&self) {
        std::fs::write(self.dir.path().join("buf.yaml"), "version: v2\n").unwrap();
    }
}

fn open(w: &mut World, files: &[File]) {
    let roots = [w.dir.path().to_path_buf()];
    open_projects(&mut w.db, files, &roots);
}

const PHP_ON: &str = "php:\n  lint:\n    rules:\n      class-not-found: warn\n";
const HEAD: &str = "syntax = \"proto3\";\npackage p;\n";

#[test]
fn an_import_none_of_whose_names_is_used_is_reported() {
    let mut w = World::new(ON);
    w.manifest();
    let a = w.file(
        "a.proto",
        &format!(
            "{HEAD}import \"b.proto\";\nimport \"c.proto\";\nimport public \"d.proto\";\nimport \"gone.proto\";\nmessage A {{ B b = 1; }}\n"
        ),
    );
    let files = [
        a,
        w.file("b.proto", &format!("{HEAD}message B {{}}\n")),
        w.file("c.proto", &format!("{HEAD}message C {{}}\n")),
        w.file("d.proto", &format!("{HEAD}message D {{}}\n")),
    ];
    open(&mut w, &files);
    assert_eq!(
        w.findings(a),
        [(4, "import \"c.proto\" is unused".to_string())],
        "b is used, d is public, gone.proto is not readable"
    );
}

#[test]
fn an_import_used_through_an_option_or_a_public_import_is_used() {
    let mut w = World::new(ON);
    w.manifest();
    let a = w.file(
        "a.proto",
        &format!(
            "{HEAD}import \"opts.proto\";\nimport \"re.proto\";\nmessage A {{ int32 x = 1 [(p.flag) = true]; E e = 2; }}\n"
        ),
    );
    let files = [
        a,
        w.file(
            "opts.proto",
            &format!("{HEAD}import \"google/protobuf/descriptor.proto\";\nextend google.protobuf.FieldOptions {{ bool flag = 50000; }}\n"),
        ),
        w.file("re.proto", &format!("{HEAD}import public \"inner.proto\";\n")),
        w.file("inner.proto", &format!("{HEAD}enum E {{ Z = 0; }}\n")),
    ];
    open(&mut w, &files);
    assert_eq!(
        w.findings(a),
        [],
        "the extension and the re-exported enum are used"
    );
}

#[test]
fn a_type_that_two_rpcs_of_the_project_name_is_reported_on_each() {
    let mut w = World::new(ON);
    w.manifest();
    let x = w.file(
        "x.proto",
        &format!(
            "{HEAD}message Q1 {{}}\nmessage Q2 {{}}\nmessage Shared {{}}\nmessage Own {{}}\nservice S {{\n  rpc A(Q1) returns (Shared);\n  rpc B(Q2) returns (Shared);\n  rpc C(Own) returns (Own2);\n}}\nmessage Own2 {{}}\n"
        ),
    );
    let y = w.file(
        "y.proto",
        "syntax = \"proto3\";\npackage r;\nimport \"x.proto\";\nmessage Q3 {}\nservice T {\n  rpc D(Q3) returns (p.Shared);\n}\n",
    );
    open(&mut w, &[x, y]);
    let message = "\"p.Shared\" is used as the request or response type for multiple RPCs";
    assert_eq!(
        w.findings(x),
        [(8, message.to_string()), (9, message.to_string())]
    );
    assert_eq!(w.findings(y), [(6, message.to_string())]);
}

#[test]
fn an_rpc_that_names_one_type_twice_is_reported() {
    let mut w = World::new(ON);
    w.manifest();
    let a = w.file(
        "a.proto",
        &format!("{HEAD}message M {{}}\nmessage N {{}}\nservice S {{\n  rpc A(M) returns (M);\n  rpc B(N) returns (N);\n}}\n"),
    );
    open(&mut w, &[a]);
    let lines: Vec<usize> = w.findings(a).into_iter().map(|(line, _)| line).collect();
    assert_eq!(lines, [6, 7], "one report for each rpc");
}

#[test]
fn a_file_under_no_manifest_has_no_index_findings() {
    let mut w = World::new(ON);
    let a = w.file(
        "a.proto",
        &format!("{HEAD}import \"c.proto\";\nmessage A {{}}\n"),
    );
    let c = w.file("c.proto", &format!("{HEAD}message C {{}}\n"));
    open(&mut w, &[a, c]);
    assert_eq!(w.findings(a), []);
}

#[test]
fn with_the_rules_off_no_project_is_opened_and_no_symbol_is_read() {
    let mut w = World::new("");
    w.manifest();
    let a = w.file(
        "a.proto",
        &format!("{HEAD}import \"c.proto\";\nmessage A {{}}\n"),
    );
    let c = w.file("c.proto", &format!("{HEAD}message C {{}}\n"));
    assert!(!reads_index(&w.db, a));
    open(&mut w, &[a, c]);
    assert_eq!(w.findings(a), []);
    assert_eq!(w.runs("symbols"), 0);
}

#[test]
fn a_body_edit_of_another_file_leaves_the_findings_of_a_file_as_they_are() {
    let mut w = World::new(ON);
    w.manifest();
    let a = w.file(
        "a.proto",
        &format!("{HEAD}import \"c.proto\";\nmessage A {{}}\n"),
    );
    let c = w.file("c.proto", &format!("{HEAD}message C {{}}\n"));
    open(&mut w, &[a, c]);
    assert_eq!(w.findings(a).len(), 1);
    let before = w.runs("lint(");
    let text = format!("{HEAD}message C {{}}\n// a comment\n");
    w.db.set_file_text(c, 2, text);
    assert_eq!(w.findings(a).len(), 1);
    assert_eq!(
        w.runs("lint("),
        before,
        "no lint runs again: the symbols are equal"
    );
}

const COMPOSER: &str = r#"{"autoload": {"psr-4": {"App\\": "src/"}}}"#;

#[test]
fn a_class_that_nothing_declares_is_reported_where_an_autoload_prefix_owns_its_name() {
    let mut w = World::new(PHP_ON);
    std::fs::write(w.dir.path().join("composer.json"), COMPOSER).unwrap();
    let a = w.file(
        "src/A.php",
        "<?php\nnamespace App;\nuse Other\\Thing;\nclass A extends B {\n    public function f(B $b, ?Missing $m, Thing $t, \\Exception $e): Gone {\n        $x = new Absent();\n        Nowhere::run();\n        try {} catch (Lost $l) {}\n        return $b instanceof Never ? Fine::class : null;\n    }\n}\n",
    );
    let b = w.file("src/B.php", "<?php\nnamespace App;\nclass B {}\n");
    open(&mut w, &[a, b]);
    let lines: Vec<usize> = w.findings(a).into_iter().map(|(line, _)| line).collect();
    assert_eq!(
        lines,
        [5, 5, 6, 7, 8, 9],
        "Missing and Gone, new, static call, catch, instanceof; Thing and \\Exception are outside the prefix, ::class is not a use"
    );
}
