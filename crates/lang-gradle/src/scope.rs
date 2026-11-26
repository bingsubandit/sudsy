//! What Gradle says to the shared scope rules (`core_rules::scope_rules`): `../scope.rows`.
//!
//! concept: scope-tables

use core_rules::{Family, ScopeLang, scope_rules};

// A double-quoted string may hold `$name` and `${expression}`: its words are reads.
static LANG: ScopeLang = ScopeLang::new(include_str!("../scope.rows")).expansions(&["String"]);

pub const FAMILY: Family = Family {
    name: "scope",
    build: |meta, p| scope_rules::build(&LANG, meta, p),
};
