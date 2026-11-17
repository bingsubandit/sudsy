//! Declarative pattern rules: ast-grep-style YAML rule
//! files whose patterns are snippets of the target language with metavariables.
//!
//! * [`metavar`]: `$X` and `$$$ARGS`, and how they are written into a snippet the language's own
//!   parser accepts.
//! * [`matcher`]: matching a compiled pattern against the shared tree.
//! * [`expr`]: the rule expression (`pattern`, `kind`, `regex`, `inside`, `has`, `any`, ...).
//! * [`file`]: the rule file format and the [`PatternRule`] it becomes. A pattern rule is a
//!   plain `core_rules::Rule`, so it runs through the same engine as every other rule.
//!
//! The matcher does not parse anything itself: whoever compiles a pattern supplies the parse
//! function of the pattern's language ([`SnippetParser`]).

pub mod expr;
pub mod file;
pub mod matcher;
pub mod metavar;

pub use file::{PatternError, PatternRule, load, load_lazy};
pub use matcher::{Captures, Pattern, SnippetParser};

/// The ids of the rule files in `shared/` (rules written on roles, built into every language whose
/// grammar has them); their fixtures live in the languages that test them.
pub const SHARED_IDS: &[&str] = &[
    "no-assignment-in-condition",
    "no-constant-comparison",
    "no-constant-condition",
    "no-empty-block",
    "no-self-compare",
];

#[cfg(test)]
mod tests {
    #[test]
    fn shared_ids_are_the_shared_files() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/shared");
        let mut ids: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                e.unwrap()
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        ids.sort();
        assert_eq!(ids, super::SHARED_IDS);
    }
}
