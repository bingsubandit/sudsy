// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const DIALECT: core_config::OptId = core_config::opt::DIALECT;
#[allow(dead_code)]
pub const COMMA_POSITION: core_config::OptId = core_config::opt::COMMA_POSITION;
#[allow(dead_code)]
pub const ALTER_TABLE_INDENT: core_config::OptId = core_config::opt::ALTER_TABLE_INDENT;
#[allow(dead_code)]
pub const KEYWORD_CASE: core_config::OptId = core_config::opt::KEYWORD_CASE;
#[allow(dead_code)]
pub const IDENTIFIER_CASE: core_config::OptId = core_config::opt::IDENTIFIER_CASE;
#[allow(dead_code)]
pub const FUNCTION_CASE: core_config::OptId = core_config::opt::FUNCTION_CASE;
#[allow(dead_code)]
pub const LITERAL_CASE: core_config::OptId = core_config::opt::LITERAL_CASE;
#[allow(dead_code)]
pub const TYPE_CASE: core_config::OptId = core_config::opt::TYPE_CASE;
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect { Ansi, Postgres, Mysql, Tsql }
#[allow(dead_code)]
pub fn dialect(o: &core_config::ResolvedOptions) -> Dialect {
    match o.text(DIALECT) {
        "postgres" => Dialect::Postgres,
        "mysql" => Dialect::Mysql,
        "tsql" => Dialect::Tsql,
        _ => Dialect::Ansi,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommaPosition { Trailing, Leading }
#[allow(dead_code)]
pub fn comma_position(o: &core_config::ResolvedOptions) -> CommaPosition {
    match o.text(COMMA_POSITION) {
        "leading" => CommaPosition::Leading,
        _ => CommaPosition::Trailing,
    }
}
#[allow(dead_code)]
pub fn alter_table_indent(o: &core_config::ResolvedOptions) -> bool {
    o.flag(ALTER_TABLE_INDENT, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeywordCase { Consistent, Upper, Lower, Capitalise }
#[allow(dead_code)]
pub fn keyword_case(o: &core_config::ResolvedOptions) -> KeywordCase {
    match o.text(KEYWORD_CASE) {
        "upper" => KeywordCase::Upper,
        "lower" => KeywordCase::Lower,
        "capitalise" => KeywordCase::Capitalise,
        _ => KeywordCase::Consistent,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierCase { Consistent, Upper, Lower, Capitalise }
#[allow(dead_code)]
pub fn identifier_case(o: &core_config::ResolvedOptions) -> IdentifierCase {
    match o.text(IDENTIFIER_CASE) {
        "upper" => IdentifierCase::Upper,
        "lower" => IdentifierCase::Lower,
        "capitalise" => IdentifierCase::Capitalise,
        _ => IdentifierCase::Consistent,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionCase { Consistent, Upper, Lower, Capitalise }
#[allow(dead_code)]
pub fn function_case(o: &core_config::ResolvedOptions) -> FunctionCase {
    match o.text(FUNCTION_CASE) {
        "upper" => FunctionCase::Upper,
        "lower" => FunctionCase::Lower,
        "capitalise" => FunctionCase::Capitalise,
        _ => FunctionCase::Consistent,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiteralCase { Consistent, Upper, Lower, Capitalise }
#[allow(dead_code)]
pub fn literal_case(o: &core_config::ResolvedOptions) -> LiteralCase {
    match o.text(LITERAL_CASE) {
        "upper" => LiteralCase::Upper,
        "lower" => LiteralCase::Lower,
        "capitalise" => LiteralCase::Capitalise,
        _ => LiteralCase::Consistent,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeCase { Consistent, Upper, Lower, Capitalise }
#[allow(dead_code)]
pub fn type_case(o: &core_config::ResolvedOptions) -> TypeCase {
    match o.text(TYPE_CASE) {
        "upper" => TypeCase::Upper,
        "lower" => TypeCase::Lower,
        "capitalise" => TypeCase::Capitalise,
        _ => TypeCase::Consistent,
    }
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::OptionDef::choice(core_config::opt::DIALECT, &["ansi", "postgres", "mysql", "tsql"], "ansi", "The SQL dialect the file is parsed as."),
        core_config::shared::line_width(80),
        shared::indent_style(),
        core_config::shared::indent_size(4),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice(core_config::opt::COMMA_POSITION, &["trailing", "leading"], "trailing", "Where a comma goes when a list is broken over lines: at the end of a line, or at the start of the next one."),
        core_config::OptionDef::boolean(core_config::opt::ALTER_TABLE_INDENT, false, "Indent the actions of `ALTER TABLE name` one level under the statement."),
        core_config::OptionDef::choice(core_config::opt::KEYWORD_CASE, &["consistent", "upper", "lower", "capitalise"], "consistent", "Case of keywords: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it."),
        core_config::OptionDef::choice(core_config::opt::IDENTIFIER_CASE, &["consistent", "upper", "lower", "capitalise"], "consistent", "Case of unquoted identifiers: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it."),
        core_config::OptionDef::choice(core_config::opt::FUNCTION_CASE, &["consistent", "upper", "lower", "capitalise"], "consistent", "Case of function names: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it."),
        core_config::OptionDef::choice(core_config::opt::LITERAL_CASE, &["consistent", "upper", "lower", "capitalise"], "consistent", "Case of `NULL`, `TRUE` and `FALSE`: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it."),
        core_config::OptionDef::choice(core_config::opt::TYPE_CASE, &["consistent", "upper", "lower", "capitalise"], "consistent", "Case of data types: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it."),
        ])
        .with_presets(vec![
        core_config::Preset::new("sql/sqlfluff", include_str!("../presets/sqlfluff.yaml")),
        ])
    })
}
