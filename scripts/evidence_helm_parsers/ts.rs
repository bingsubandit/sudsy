//! tree-sitter-go-template (ngalaiko, commit 06b2fd2, the root grammar `gotmpl`; parser built by build.rs from
//! fetch-grammar.sh): whole template text, YAML text kept as the grammar's text nodes. Error: the tree has an error or a
//! missing node. Share: bytes of the top-most error nodes over the text length.
use evidence_helm_parsers::{Check, load, probes, run};
use std::path::PathBuf;
use tree_sitter::{Language, Node, Parser};
use tree_sitter_language::LanguageFn;

// The grammar's parser.c exports `tree_sitter_gotmpl` (make_grammar('gotmpl')); the crate's own binding names another symbol.
unsafe extern "C" {
    fn tree_sitter_gotmpl() -> *const ();
}

fn error_bytes(node: Node) -> usize {
    if node.is_error() || node.is_missing() {
        return node.end_byte() - node.start_byte();
    }
    if !node.has_error() {
        return 0;
    }
    (0..node.child_count()).filter_map(|i| node.child(i)).map(error_bytes).sum()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let items = load(&PathBuf::from(&args[1]));
    let probe_list = probes(&PathBuf::from(&args[2]));
    let language: Language = unsafe { LanguageFn::from_raw(tree_sitter_gotmpl) }.into();
    let mut parser = Parser::new();
    parser.set_language(&language).expect("go-template language");
    run("tree-sitter-go-template 06b2fd2 (grammar 0.19.5, tree-sitter runtime 0.25)", &items, &probe_list, |text| {
        let tree = parser.parse(text, None).expect("tree");
        let root = tree.root_node();
        let error = root.has_error();
        let share = if error { error_bytes(root) as f64 / text.len().max(1) as f64 } else { 0.0 };
        Check { error, share }
    });
}
