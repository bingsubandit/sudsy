//! Go template (`text/template`) language support for tree-sitter (a copy of tree-sitter-go-template, see FORK.md).

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_gotmpl() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_gotmpl) };
