//! The rules of PHP; `cargo xtask new-rule php <id> --rust` adds a module under `rules/` and
//! lists it here.

use core_rules::Rule;

pub static RULES: &[&dyn Rule] = &[&crate::scope::UNDEFINED_VARIABLE];
