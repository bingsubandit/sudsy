//! The symbol index: `Known`, `Absent` and `Unknown` for PHP (project files, Composer
//! dependencies read on demand) and Protobuf (imports), and what an edit re-runs.

use core_config::EditorConfig;
use core_incremental::{Answer, Database, File, Index, Project};
use core_semantics::symbols::Name;
use provider_api::LanguageProvider;
use salsa::EventKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct World {
    db: Database,
    dir: TempDir,
    log: Arc<Mutex<Vec<String>>>,
}

fn name(candidates: &[&str]) -> Name {
    candidates.iter().map(|c| (*c).into()).collect()
}

impl World {
    fn new(manifest: &str) -> World {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = log.clone();
        let providers: Vec<Box<dyn LanguageProvider>> = vec![
            Box::new(lang_php::provider()),
            Box::new(lang_protobuf::provider()),
        ];
        let db = Database::with_event_hook(
            providers,
            Box::new(move |event| {
                if let EventKind::WillExecute { database_key } = event.kind {
                    sink.lock().unwrap().push(format!("{database_key:?}"));
                }
            }),
        );
        let w = World {
            db,
            dir: tempfile::tempdir().unwrap(),
            log,
        };
        if !manifest.is_empty() {
            w.write("composer.json", manifest);
        }
        w
    }

    fn write(&self, path: &str, text: &str) -> PathBuf {
        let p = self.dir.path().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    /// A project file: on disk and as an input.
    fn file(&self, path: &str, text: &str) -> File {
        let p = self.write(path, text);
        File::new(
            &self.db,
            p,
            1,
            text.to_string(),
            false,
            EditorConfig::default(),
        )
    }

    fn project(&self, system: &str, files: Vec<File>, complete: bool) -> Project {
        let locator = core_packages::open(system, self.dir.path());
        Project::new(&self.db, files, complete, locator, 0)
    }

    fn runs(&self, query: &str) -> usize {
        let log = self.log.lock().unwrap();
        log.iter().filter(|k| k.contains(query)).count()
    }
}

const MANIFEST: &str = r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#;
const INSTALLED: &str = r#"{"packages":[{"name":"acme/lib","install-path":"../acme/lib","autoload":{"psr-4":{"Acme\\Lib\\":"src"}}}]}"#;

/// A project of two classes, one extending a vendor class.
fn php_world() -> (World, Vec<File>) {
    let w = World::new(MANIFEST);
    w.write("vendor/composer/installed.json", INSTALLED);
    w.write(
        "vendor/acme/lib/src/Tool.php",
        "<?php\nnamespace Acme\\Lib;\nclass Tool { public function run(int $a) {} }\n",
    );
    w.write(
        "vendor/acme/lib/src/Other.php",
        "<?php\nnamespace Acme\\Lib;\nclass Other {}\n",
    );
    let files = vec![
        w.file(
            "src/Base.php",
            "<?php\nnamespace App;\nclass Base extends \\Acme\\Lib\\Tool { public $p; }\n",
        ),
        w.file(
            "src/Child.php",
            "<?php\nnamespace App;\nclass Child extends Base { function own() {} }\n",
        ),
    ];
    (w, files)
}

fn is_known<T>(a: &Answer<T>) -> bool {
    matches!(a, Answer::Known(_))
}

fn known<T: std::fmt::Debug>(a: Answer<T>) -> T {
    match a {
        Answer::Known(t) => t,
        other => panic!("expected Known, got {other:?}"),
    }
}

#[test]
fn a_name_is_known_in_the_project_and_in_a_dependency_read_on_demand() {
    let (w, files) = php_world();
    let from = files[1];
    let project = w.project("composer", files, true);
    let index = Index::new(&w.db, project);
    let child = known(index.type_(from, &name(&["App\\Child"])));
    assert_eq!(&*child.item.name, "App\\Child");
    assert_eq!(
        w.runs("symbols"),
        2,
        "the project's two files are read, nothing else"
    );
    let tool = known(index.type_(from, &name(&["Acme\\Lib\\Tool"])));
    assert!(
        tool.file
            .path(&w.db)
            .ends_with("vendor/acme/lib/src/Tool.php")
    );
    let lowered = index.type_(from, &name(&["acme\\lib\\TOOL"]));
    assert_eq!(known(lowered).file, tool.file, "names compare without case");
    assert_eq!(
        w.runs("symbols"),
        3,
        "Tool.php is read once, Other.php never"
    );
}

#[test]
fn a_name_the_locator_covers_and_no_file_declares_is_absent() {
    let (w, files) = php_world();
    let from = files[0];
    let project = w.project("composer", files, true);
    let index = Index::new(&w.db, project);
    assert!(index.type_(from, &name(&["App\\Nope"])).is_absent());
    // A prefix of an installed package may share its namespace with an extension: no proof.
    assert_eq!(
        index.type_(from, &name(&["Acme\\Lib\\Nope"])),
        Answer::Unknown
    );
    // The first candidate is absent, the second is declared: the second is meant.
    let both = index.type_(from, &name(&["App\\Tool", "Acme\\Lib\\Tool"]));
    assert_eq!(&*known(both).item.name, "Acme\\Lib\\Tool");
}

#[test]
fn a_missing_fact_makes_the_answer_unknown() {
    let (w, files) = php_world();
    let from = files[0];
    let ask = |project: Project, n: &str| Index::new(&w.db, project).type_(from, &name(&[n]));
    // Outside every prefix: a PHP built-in, or a package that is not installed.
    let project = w.project("composer", files.clone(), true);
    assert_eq!(ask(project, "Exception"), Answer::Unknown);
    assert_eq!(ask(project, "Other\\Lib\\Foo"), Answer::Unknown);
    // The project is not known to be complete.
    let open = w.project("composer", files.clone(), false);
    assert_eq!(ask(open, "App\\Nope"), Answer::Unknown);
    // No manifest, no locator.
    let bare = Project::new(&w.db, files.clone(), true, None, 0);
    assert_eq!(ask(bare, "App\\Nope"), Answer::Unknown);
    assert!(is_known(&ask(bare, "App\\Child")));
    // A file with a syntax error, or one that declares names at run time, may declare anything.
    let broken = w.file("src/Broken.php", "<?php\nclass {{{");
    let with_broken = w.project("composer", vec![files[0], files[1], broken], true);
    assert_eq!(ask(with_broken, "App\\Nope"), Answer::Unknown);
    let alias = w.file("src/Alias.php", "<?php\nclass_alias($a, $b);\n");
    let with_alias = w.project("composer", vec![files[0], alias], true);
    assert_eq!(ask(with_alias, "App\\Nope"), Answer::Unknown);
    assert!(is_known(&ask(with_alias, "App\\Base")));
}

#[test]
fn a_declaration_that_the_file_does_not_prove_is_unknown() {
    let w = World::new(MANIFEST);
    let files = vec![
        w.file(
            "src/Cond.php",
            "<?php\nnamespace App;\nif (!class_exists('App\\Cond')) { class Cond {} }\n",
        ),
        w.file("src/Twice.php", "<?php\nnamespace App;\nclass Twice {}\n"),
        w.file("src/Twice2.php", "<?php\nnamespace App;\nclass Twice {}\n"),
        // The class a PSR-4 path names is not in its file.
        w.file("src/Wrong.php", "<?php\nnamespace App;\nclass Other {}\n"),
    ];
    let from = files[0];
    let project = w.project("composer", files, true);
    let index = Index::new(&w.db, project);
    for n in ["App\\Cond", "App\\Twice", "App\\Wrong"] {
        assert_eq!(index.type_(from, &name(&[n])), Answer::Unknown, "{n}");
    }
    assert!(is_known(&index.type_(from, &name(&["App\\Other"]))));
}

#[test]
fn a_member_is_found_through_parents_and_proved_absent_only_on_a_closed_chain() {
    let (w, files) = php_world();
    let from = files[1];
    let project = w.project("composer", files, true);
    let index = Index::new(&w.db, project);
    let child = name(&["App\\Child"]);
    assert_eq!(&*known(index.member(from, &child, "own")).item.name, "own");
    assert_eq!(&*known(index.member(from, &child, "RUN")).item.name, "run");
    assert_eq!(&*known(index.member(from, &child, "p")).item.name, "p");
    // Tool is `Acme\Lib\Tool` with no parent: the chain is closed.
    assert!(index.member(from, &child, "missing").is_absent());
}

#[test]
fn a_chain_with_an_unresolved_or_dynamic_part_has_unknown_members() {
    let w = World::new(MANIFEST);
    let files = vec![
        w.file(
            "src/A.php",
            "<?php\nnamespace App;\nclass A extends \\Vendor\\Gone {}\n",
        ),
        w.file(
            "src/B.php",
            "<?php\nnamespace App;\nclass B { function __call($n, $a) {} }\n",
        ),
        w.file("src/C.php", "<?php\nnamespace App;\nclass C extends D {}\n"),
        w.file("src/D.php", "<?php\nnamespace App;\nclass D extends C {}\n"),
        w.file(
            "src/E.php",
            "<?php\nnamespace App;\nclass E { use Hook { a as b; } }\n",
        ),
    ];
    let from = files[0];
    let project = w.project("composer", files, true);
    let index = Index::new(&w.db, project);
    for n in ["A", "B", "C", "E"] {
        let ty = name(&[&format!("App\\{n}")]);
        assert_eq!(index.member(from, &ty, "x"), Answer::Unknown, "{n}");
    }
}

#[test]
fn a_body_edit_re_runs_no_index_query_and_a_signature_edit_only_the_join() {
    let (mut w, files) = php_world();
    let from = files[1];
    let project = w.project("composer", files.clone(), true);
    {
        let index = Index::new(&w.db, project);
        known(index.type_(from, &name(&["App\\Child"])));
    }
    let joins = w.runs("project_declared");
    assert_eq!(joins, 1);
    let edit = |w: &mut World, text: &str| {
        let version = files[1].version(&w.db) + 1;
        w.db.set_file_text(files[1], version, text.to_string());
        let index = Index::new(&w.db, project);
        known(index.type_(from, &name(&["App\\Child"])));
    };
    edit(
        &mut w,
        "<?php\nnamespace App;\nclass Child extends Base { function own() { return 1; } }\n",
    );
    assert_eq!(
        w.runs("project_declared"),
        joins,
        "equal symbols: the join stands"
    );
    edit(
        &mut w,
        "<?php\nnamespace App;\nclass Child extends Base { function own() {} function more() {} }\n",
    );
    assert_eq!(w.runs("project_declared"), joins + 1);
}

#[test]
fn protobuf_names_are_visible_through_imports_only() {
    let w = World::new("");
    let a = w.file(
        "a.proto",
        "syntax = \"proto3\";\npackage p;\nimport \"b.proto\";\nmessage A {}\n",
    );
    let b = w.file(
        "b.proto",
        "syntax = \"proto3\";\npackage q;\nimport public \"c.proto\";\nmessage B {}\n",
    );
    let c = w.file(
        "c.proto",
        "syntax = \"proto3\";\npackage r;\nmessage C {}\n",
    );
    w.write(
        "d.proto",
        "syntax = \"proto3\";\npackage s;\nmessage D {}\n",
    );
    let project = w.project("import-paths", vec![a, b, c], true);
    let index = Index::new(&w.db, project);
    assert!(is_known(&index.type_(a, &name(&["p.A"]))));
    assert!(
        is_known(&index.type_(a, &name(&["q.B"]))),
        "a direct import"
    );
    assert!(
        is_known(&index.type_(a, &name(&["r.C"]))),
        "a public import of it"
    );
    // `s.D` is a file of the project that `a.proto` does not import.
    assert!(index.type_(a, &name(&["p.s.D", "s.D"])).is_absent());
    assert!(
        index.type_(b, &name(&["p.A"])).is_absent(),
        "b does not import a"
    );
}

#[test]
fn a_protobuf_import_that_cannot_be_found_declares_nothing_when_the_roots_are_all_there_are() {
    let w = World::new("");
    let a = w.file(
        "a.proto",
        "syntax = \"proto3\";\npackage p;\nimport \"google/protobuf/empty.proto\";\nmessage A {}\n",
    );
    let project = w.project("import-paths", vec![a], true);
    let index = Index::new(&w.db, project);
    assert!(index.type_(a, &name(&["p.Nope"])).is_absent());
    // Roots that may not be all of them: the file may be in another one.
    let open = Project::new(
        &w.db,
        vec![a],
        true,
        Some(Arc::new(core_packages::ImportPaths::new(
            vec![w.dir.path().to_path_buf()],
            false,
        ))),
        0,
    );
    let index = Index::new(&w.db, open);
    assert_eq!(index.type_(a, &name(&["p.Nope"])), Answer::Unknown);
}

/// One file under two spellings (names that differ only by case on Windows and macOS, here a
/// symlink) is one input, so a name looked up with another case finds the same file.
#[cfg(unix)]
#[test]
fn a_dependency_under_two_spellings_is_one_file() {
    use core_incremental::Db;
    let (w, _) = php_world();
    let real = w.dir.path().join("vendor/acme/lib/src/Tool.php");
    let alias = w.dir.path().join("vendor/acme/lib/src/TOOL.php");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let a = w.db.dependency_file(&real).unwrap();
    let b = w.db.dependency_file(&alias).unwrap();
    assert_eq!(a, b);
}
