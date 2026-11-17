//! The rules of Kotlin; `cargo xtask new-rule kotlin <id> --rust` adds a module under `rules/` and
//! lists it here.

use core_rules::Rule;

pub static RULES: &[&dyn Rule] = &[&crate::scope::UNUSED_IMPORT];
