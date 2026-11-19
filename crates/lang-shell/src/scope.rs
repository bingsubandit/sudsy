//! What Shell says to the shared scope rules (`core_rules::scope_rules`): `../scope.rows`, the
//! token kinds that hold `$name` reads and arithmetic, and that names are words.
//!
//! concept: scope-tables

use core_rules::{Family, ScopeLang};

static LANG: ScopeLang = ScopeLang::new(include_str!("../scope.rows"))
    .expansions(&["Word", "HeredocBody"])
    .arithmetic(&["Arith"])
    .identifier_names()
    .lowercase_undefined()
    .first_undefined();

pub static FAMILIES: &[Family] = &[Family {
    name: "scope",
    build: |meta, p| core_rules::scope_rules::build(&LANG, meta, p),
}];
