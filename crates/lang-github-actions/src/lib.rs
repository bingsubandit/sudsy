//! GitHub Actions workflows (`.github/workflows/*.yml`) and action metadata (`action.yml`): YAML
//! with its own rules. Parsing, formatting and options are `lang-yaml`'s; the `run:` scripts are
//! embedded Shell or PowerShell.
//!
//! concept: none (glue: provider entry)

mod expression;
mod jobs;
mod scripts;
mod shape;
mod view;

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::{Rule, RuleHooks};
use core_syntax::{Embedded, Grammar, LanguageId, SyntaxNode};
use provider_api::{
    Capabilities, FileMatcher, FilePattern, LanguageProvider, ParseResult, ParserProvider,
    ProviderError, SourceFile,
};
use std::sync::OnceLock;

static RULES: &[&dyn Rule] = &[
    &jobs::InvalidJobId,
    &jobs::DuplicateIds,
    &jobs::UnknownNeeds,
    &jobs::CyclicNeeds,
    &jobs::InvalidRunsOn,
    &jobs::MissingPermissions,
    &expression::InvalidExpression,
    &expression::ExpressionContext,
    &expression::ExpressionTypes,
    &expression::IfCondition,
    &scripts::DeprecatedCommands,
    &scripts::ScriptInjection,
    &shape::WorkflowSyntax,
    &shape::InvalidEvent,
    &shape::InvalidPermissions,
    &shape::OutdatedAction,
    &shape::InvalidGlob,
];

pub struct GithubActionsProvider {
    yaml: ParserProvider,
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    all: OnceLock<&'static [&'static dyn Rule]>,
    rust: OnceLock<&'static [&'static dyn Rule]>,
}

fn join(base: &[&'static dyn Rule]) -> &'static [&'static dyn Rule] {
    Box::leak([base, RULES].concat().into_boxed_slice())
}

impl Default for GithubActionsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubActionsProvider {
    pub fn new() -> Self {
        let matcher = |pattern| FileMatcher {
            pattern,
            dialect: None,
        };
        let in_workflows = |extension: &str| FilePattern::InDirectory {
            dir: "workflows".into(),
            extension: extension.into(),
        };
        GithubActionsProvider {
            yaml: lang_yaml::provider(),
            matchers: vec![
                matcher(in_workflows("yml")),
                matcher(in_workflows("yaml")),
                matcher(FilePattern::FileName("action.yml".into())),
                matcher(FilePattern::FileName("action.yaml".into())),
            ],
            schema: lang_yaml::option_schema().extended(
                vec![],
                vec![core_config::Preset::new(
                    "github-actions/actionlint",
                    include_str!("../presets/actionlint.yaml"),
                )],
            ),
            all: OnceLock::new(),
            rust: OnceLock::new(),
        }
    }
}

impl LanguageProvider for GithubActionsProvider {
    fn id(&self) -> LanguageId {
        self.yaml.id()
    }

    fn language(&self) -> &str {
        "github-actions"
    }

    fn crate_name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn grammar(&self) -> &Grammar {
        self.yaml.grammar()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        self.yaml.parse(file)
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        self.yaml.formatter()
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn config_base(&self) -> Option<&str> {
        Some(self.yaml.language())
    }

    fn base_rules(&self) -> &'static [&'static dyn Rule] {
        self.yaml.rules()
    }

    /// YAML's rules, pattern rules included, then the workflow rules.
    fn rules(&self) -> &'static [&'static dyn Rule] {
        self.all.get_or_init(|| join(self.yaml.rules()))
    }

    fn rust_rules(&self) -> &'static [&'static dyn Rule] {
        self.rust.get_or_init(|| join(self.yaml.rust_rules()))
    }

    fn rule_hooks(&self) -> &dyn RuleHooks {
        self.yaml.rule_hooks()
    }

    fn formats_embedded(&self) -> bool {
        false
    }

    fn embedded(&self, root: &SyntaxNode, text: &str) -> Vec<Embedded> {
        scripts::embedded(root, text, self.yaml.grammar(), self.yaml.rule_hooks())
    }
}
