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

pub mod allowed_values;
pub mod blank_lines;
pub mod cases;
pub mod cells;
pub mod checks;
pub mod clause_style;
pub mod commands;
pub mod comments;
pub mod configure;
pub mod content_model;
pub mod curly;
pub mod decl_rows;
mod declare;
pub mod declared_name;
pub mod detached_token;
pub mod diagnostic;
pub mod directory_facts;
pub mod doc_comments;
pub mod doc_first_word;
pub mod doc_lines;
pub mod doc_params;
pub mod doc_run;
pub mod doc_style;
pub mod documented_command;
pub mod embed_keys;
pub mod engine;
pub mod escapes;
pub mod expr;
pub mod extension;
pub mod family;
pub mod feature_support;
pub mod file_name;
pub mod fixes;
pub mod flow_rules;
pub mod format_diff;
pub mod format_strings;
pub mod hard_tabs;
pub mod heading_outline;
pub mod heading_style;
pub mod hooks;
pub mod imports;
pub mod index_rules;
pub mod keys;
pub mod layout;
pub mod lines;
pub mod listed;
pub mod markup;
pub mod markup_checks;
pub mod max;
pub mod missing_arguments;
pub mod missing_return;
pub mod modifiers;
pub mod module_names;
pub mod multiline_start;
pub mod names;
pub mod naming;
pub mod options;
pub mod paths;
pub mod project;
pub mod query_scope;
pub mod redundant_return_type;
pub mod references;
mod regex;
pub mod regex_source;
pub mod rule;
pub mod rule_text;
pub mod scope_rules;
mod sections;
pub mod shape;
pub mod shared;
pub mod simplify;
pub mod slot_path;
pub mod sorted;
pub mod sorting;
pub mod statement_blank_lines;
pub mod statement_indent;
pub mod suppress;
pub mod todo_comment;
pub mod token_case;
pub mod tree;
pub mod unicode_escape;
pub mod unique;
pub mod unit_doc;
pub mod unknown_reference;
pub mod word_case;
pub mod word_names;
pub mod word_table;

pub use configure::activate;
pub use diagnostic::{
    Category, Diagnostic, Edit, FORMAT, Fix, FixKind, Related, SYNTAX_ERROR, Severity,
};
pub use engine::{
    ActiveRule, FileReport, collect_directory, collect_nothing, directory_rules, layers_needed,
    run_directory, run_file, run_file_rules, run_project, run_rules, silence,
};
pub use extension::{ALL_LANGUAGES, ExtensionRule, Extensions};
pub use family::{Checked, Family, Params};
pub use fixes::{Analysis, FixOutcome, fix_loop};
pub use hooks::{
    Accessor, BlockHooks, DEFAULT_HOOKS, DefaultHooks, Fold, RuleHooks, Scalar, WithVersion, Word,
};
pub use layout::{Attach, LayoutRole, attached, run_layout};
pub use lines::LineIndex;
pub use options::{Check, Opt, OptionDecl, RuleOptions};
pub use project::{DirectoryScope, Fact, ProjectContext, ProjectFacts, Scope};
pub use regex::{Meta, Re, RegexCache, meta_regex};
pub use rule::{BodyContext, EmbeddedTree, FileContext, Layer, Rule, RuleContext, RuleMeta};
pub use scope_rules::{ScopeLang, ScopeRule};
