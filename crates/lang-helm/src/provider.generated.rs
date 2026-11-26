// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Mapping, Spec, provider_of};
use provider_api::ParserProvider;

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    provider_of(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("helm", tree_sitter_gotmpl::LANGUAGE), grammar: grammars::HELM, mapping: &MAPPING, extensions: &[], file_names: &["templates/**/*.yaml", "templates/**/*.yml", "templates/**/*.tpl"], schema: option_schema(), formatter: &HelmFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[
    ],
    checked: &[],
    kinds: &[
        ("true", "Boolean"),
        ("false", "Boolean"),
        ("}}", "Close"),
        ("-}}", "Close"),
        ("comment", "Comment"),
        ("template", "Document"),
        ("identifier", "Ident"),
        ("field_identifier", "Ident"),
        ("if", "Keyword"),
        ("else", "Keyword"),
        ("end", "Keyword"),
        ("range", "Keyword"),
        ("with", "Keyword"),
        ("define", "Keyword"),
        ("block", "Keyword"),
        ("\"template\"", "Keyword"),
        ("break", "Keyword"),
        ("continue", "Keyword"),
        ("nil", "Nil"),
        ("int_literal", "Number"),
        ("float_literal", "Number"),
        ("imaginary_literal", "Number"),
        ("rune_literal", "Number"),
        ("{{", "Open"),
        ("{{-", "Open"),
        ("raw_string_literal", "String"),
        ("interpreted_string_literal", "String"),
        ("text", "Text"),
        ("yaml_no_injection_text", "Text"),
        ("if_action", "IfAction"),
        ("with_action", "WithAction"),
        ("range_action", "RangeAction"),
        ("define_action", "DefineAction"),
        ("block_action", "BlockAction"),
        ("template_action", "TemplateAction"),
        ("break_action", "BreakAction"),
        ("continue_action", "ContinueAction"),
        ("range_variable_definition", "RangeVariableDefinition"),
        ("variable_definition", "VariableDefinition"),
        ("assignment", "Assignment"),
        ("chained_pipeline", "ChainedPipeline"),
        ("parenthesized_pipeline", "ParenthesizedPipeline"),
        ("method_call", "MethodCall"),
        ("function_call", "FunctionCall"),
        ("argument_list", "ArgumentList"),
        ("selector_expression", "SelectorExpression"),
        ("field", "Field"),
        ("variable", "Variable"),
        ("$", "Dollar"),
        (".", "Dot"),
        ("|", "Pipe"),
        ("=", "Assign"),
        (":=", "Declare"),
        ("(", "LParen"),
        (")", "RParen"),
        (",", "Comma"),
    ],
};
