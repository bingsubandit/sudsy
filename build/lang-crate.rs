//! Shared `build` script of the language crates (`build = "../../build/lang-crate.rs"`):
//! - lists `rules/*.yaml` for `include_str!`, so that adding a built-in pattern rule is adding a file;
//!   a grammar that declares `roles` also gets `crates/core-patterns/shared/*.yaml`, the rules written on roles;
//! - generates the typed wrappers (`generated.rs`: kinds, AST nodes, grammar hash) from
//!   `grammars/<name>.grammar`, `<name>` being the crate name without `lang-`.

use core_syntax::{Grammar, codegen};
use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let name = env::var("CARGO_PKG_NAME").unwrap();
    let rel = format!("grammars/{}.grammar", name.trim_start_matches("lang-"));
    println!("cargo:rerun-if-changed=../../{rel}");
    let src = fs::read_to_string(manifest.join("../..").join(&rel)).unwrap();
    let grammar = Grammar::parse(&src).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let module = codegen::rust_module(&grammar, &rel).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let dir = manifest.join("rules");
    // Without the directory, watch the crate so that creating it is noticed.
    println!(
        "cargo:rerun-if-changed={}",
        if dir.exists() { "rules" } else { "." }
    );
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    files.sort();
    let shared = manifest.join("../core-patterns/shared");
    println!("cargo:rerun-if-changed=../core-patterns/shared");
    let mut shared_files: Vec<PathBuf> = match grammar.requires_roles {
        true => fs::read_dir(&shared)
            .into_iter()
            .flatten()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
            // A rule none of whose roles a node of the grammar has is not built in at all.
            .filter(|p| {
                let text = fs::read_to_string(p).unwrap();
                let roles = text
                    .lines()
                    .find_map(|l| l.strip_prefix("  role:"))
                    .unwrap_or("");
                let mut names = roles
                    .split(|c: char| !(c.is_alphanumeric() || c == '-'))
                    .filter(|n| !n.is_empty());
                names.any(|n| !grammar.nodes_with_tag(n).is_empty())
            })
            .collect(),
        false => Vec::new(),
    };
    shared_files.sort();
    let shared_items: String = shared_files
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            format!("    include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../core-patterns/shared/{name}\")),\n")
        })
        .collect();
    let items: String = files
        .iter()
        // File names only: the crate's own directory is resolved when it compiles, so a build
        // slot shared by several checkouts never reads another checkout's files.
        .map(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            format!("    include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/rules/{name}\")),\n")
        })
        .collect();
    let items = format!("{items}{shared_items}");
    fs::write(out.join("pattern_rules.rs"), format!("&[\n{items}]\n")).unwrap();

    fs::write(out.join("generated.rs"), module).unwrap();
}
