//! The rule engine: the rule trait with its four scopes,
//! the diagnostics and fix model, suppression comments, the fix loop, and the rules every
//! language shares. Languages contribute their own rules through the provider.
//!
//! * [`rule`]: the `Rule` trait, `RuleMeta` and the contexts.
//! * [`engine`]: the single traversal per file and the project phase.
//! * [`configure`]: which rules run on a file, with which options.
//! * [`fixes`]: fix selection and application, and the loop around them.
//! * [`layout`]: layout directives that rules attach for the formatter.
//! * `suppress`: `sudsy-disable-next-line`, `-disable-line`, `-disable`/`-enable`, `-disable-file`.
//!
//! concept: none (glue: crate root)

pub mod blank_lines;
pub mod cases;
pub mod checks;
pub mod commands;
pub mod comments;
pub mod configure;
pub mod curly;
mod declare;
pub mod diagnostic;
pub mod embed_keys;
pub mod engine;
pub mod escapes;
pub mod extension;
pub mod family;
pub mod fixes;
pub mod flow_rules;
pub mod format_strings;
pub mod heading_style;
pub mod hooks;
pub mod imports;
pub mod keys;
pub mod layout;
pub mod lines;
pub mod listed;
pub mod max;
pub mod modifiers;
pub mod names;
pub mod naming;
pub mod options;
pub mod paths;
pub mod project;
mod regex;
pub mod regex_source;
pub mod rule;
pub mod scope_rules;
mod sections;
pub mod shared;
pub mod simplify;
pub mod slot_path;
pub mod sorted;
pub mod sorting;
pub mod suppress;
pub mod tree;
pub mod unique;

pub use configure::activate;
pub use diagnostic::{
    Category, Diagnostic, Edit, FORMAT, Fix, FixKind, Related, SYNTAX_ERROR, Severity,
};
pub use engine::{
    ActiveRule, FileReport, layers_needed, run_file, run_file_rules, run_project, run_rules,
    silence,
};
pub use extension::{ALL_LANGUAGES, ExtensionRule, Extensions};
pub use family::{Checked, Family, Params};
pub use fixes::{Analysis, FixOutcome, fix_loop};
pub use hooks::{Accessor, DEFAULT_HOOKS, DefaultHooks, RuleHooks, Scalar};
pub use layout::{Attach, LayoutRole, attached, run_layout};
pub use lines::LineIndex;
pub use options::{Check, Opt, OptionDecl, RuleOptions};
pub use project::{Fact, ProjectContext, ProjectFacts};
pub use regex::{Meta, Re, RegexCache, meta_regex};
pub use rule::{BodyContext, EmbeddedTree, FileContext, Layer, Rule, RuleContext, RuleMeta};
pub use scope_rules::{ScopeLang, ScopeRule};
