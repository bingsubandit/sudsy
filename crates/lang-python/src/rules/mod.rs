//! The rules of Python: pattern files in `rules/*.yaml`, and the `check` family they name for
//! ruff's rules that read the syntax tree and the scope model. `cargo xtask new-rule python <id>
//! --rust` adds a module under `rules/` and lists it here.

mod branch;
mod control;
mod decl;
mod isort;
mod lints;
mod literal;
mod misc;
mod scope;
mod stmts;
mod tree;
mod types;

use crate::generated::kind as k;
use core_rules::{Family, Params, Rule, RuleMeta};

pub static RULES: &[&dyn Rule] = &[];

pub static FAMILIES: &[Family] = &[
    Family {
        name: "check",
        build,
    },
    Family {
        name: "scope",
        build: |meta, p| core_rules::scope_rules::build(&crate::semantic::LANG, meta, p),
    },
    core_rules::regex_source::FAMILY,
];

pub(super) use core_rules::checks::{Entry, NodeCheck as Check};

const CALL: &[core_syntax::SyntaxKind] = &[k::EXPR_CALL];
const UNARY: &[core_syntax::SyntaxKind] = &[k::EXPR_UNARY_OP];
const IF_EXPR: &[core_syntax::SyntaxKind] = &[k::EXPR_IF];
const HANDLER: &[core_syntax::SyntaxKind] = &[k::EXCEPT_HANDLER_EXCEPT_HANDLER];
const MODULE: &[core_syntax::SyntaxKind] = &[k::MOD_MODULE];

const CHECKS: &[Entry] = &[
    (
        "forward-annotation-syntax-error",
        MODULE,
        scope::forward_annotation_syntax_error,
    ),
    (
        "needless-bool",
        &[k::STMT_IF, k::ELIF_ELSE_CLAUSE],
        branch::needless_bool,
    ),
    (
        "if-with-same-arms",
        &[k::STMT_IF],
        branch::if_with_same_arms,
    ),
    (
        "open-file-without-with",
        CALL,
        branch::open_file_without_with,
    ),
    ("nested-with", &[k::STMT_WITH], branch::nested_with),
    (
        "raise-vanilla-class",
        &[k::STMT_RAISE],
        branch::raise_vanilla_class,
    ),
    (
        "type-check-without-type-error",
        &[k::STMT_IF],
        branch::type_check_without_type_error,
    ),
    (
        "explicit-return-none",
        &[k::STMT_RETURN],
        branch::explicit_return_none,
    ),
    (
        "useless-expression",
        &[k::STMT_EXPR],
        branch::useless_expression,
    ),
    (
        "implicit-concat-in-collection",
        &[k::EXPR_LIST, k::EXPR_TUPLE, k::EXPR_SET],
        literal::implicit_concat_in_collection,
    ),
    (
        "unsorted-dunder-all",
        &[k::STMT_ASSIGN, k::STMT_AUG_ASSIGN],
        literal::unsorted_dunder_all,
    ),
    (
        "unsorted-dunder-slots",
        &[k::STMT_ASSIGN],
        literal::unsorted_dunder_slots,
    ),
    ("union-annotation", &[k::EXPR_SUBSCRIPT], types::union),
    ("optional-annotation", &[k::EXPR_SUBSCRIPT], types::optional),
    (
        "deprecated-typing-alias",
        &[k::EXPR_SUBSCRIPT],
        types::deprecated_alias,
    ),
    (
        "default-type-arguments",
        &[k::EXPR_SUBSCRIPT],
        types::default_type_arguments,
    ),
    (
        "quoted-annotation",
        &[k::STMT_FUNCTION_DEF, k::STMT_ANN_ASSIGN, k::PARAMETER],
        types::quoted_annotation,
    ),
    (
        "type-alias-annotation",
        &[k::STMT_ANN_ASSIGN],
        types::type_alias_annotation,
    ),
    ("generic-class", &[k::STMT_CLASS_DEF], types::generic_class),
    (
        "generic-function",
        &[k::STMT_FUNCTION_DEF],
        types::generic_function,
    ),
    (
        "datetime-utc-alias",
        &[k::EXPR_ATTRIBUTE],
        types::datetime_utc,
    ),
    (
        "deprecated-import",
        &[k::STMT_IMPORT_FROM],
        types::deprecated_import,
    ),
    (
        "outdated-version-block",
        &[k::STMT_IF, k::ELIF_ELSE_CLAUSE],
        types::outdated_version_block,
    ),
    ("unsorted-imports", lints::BODIES, isort::unsorted_imports),
    (
        "cached-method",
        &[k::STMT_FUNCTION_DEF],
        misc::cached_method,
    ),
    (
        "function-call-in-default",
        &[k::PARAMETER_WITH_DEFAULT],
        misc::function_call_in_default,
    ),
    (
        "custom-type-var-return",
        &[k::STMT_FUNCTION_DEF],
        misc::custom_type_var_return,
    ),
    (
        "any-eq-parameter",
        &[k::STMT_FUNCTION_DEF],
        misc::any_eq_parameter,
    ),
    (
        "subprocess-run-without-check",
        CALL,
        misc::subprocess_run_without_check,
    ),
    (
        "type-var-name-variance",
        &[k::STMT_ASSIGN],
        misc::type_var_name_variance,
    ),
    ("static-join", CALL, misc::static_join),
    ("verbose-raise", HANDLER, misc::verbose_raise),
    (
        "bare-raise-outside-handler",
        &[k::STMT_RAISE],
        misc::bare_raise_outside_handler,
    ),
    (
        "mutable-class-default",
        &[k::STMT_CLASS_DEF],
        decl::mutable_class_default,
    ),
    (
        "useless-import-alias",
        &[k::ALIAS],
        decl::useless_import_alias,
    ),
    ("blind-except", HANDLER, control::blind_except),
    (
        "collapsible-if",
        &[k::STMT_IF, k::ELIF_ELSE_CLAUSE],
        control::collapsible_if,
    ),
    ("try-except-pass", HANDLER, control::try_except_pass),
    (
        "leftover-debugger",
        &[k::EXPR_CALL, k::STMT_IMPORT, k::STMT_IMPORT_FROM],
        control::debugger,
    ),
    (
        "undefined-local-with-import-star",
        MODULE,
        scope::import_star,
    ),
    (
        "undefined-local-with-nested-import-star-usage",
        MODULE,
        scope::nested_import_star,
    ),
    (
        "module-import-not-at-top-of-file",
        MODULE,
        stmts::module_import_not_at_top,
    ),
    (
        "multiple-statements-on-one-line-colon",
        stmts::COMPOUND,
        stmts::colon_statement,
    ),
    (
        "useless-semicolon",
        tree::STATEMENTS,
        stmts::useless_semicolon,
    ),
    (
        "multiple-statements-on-one-line-semicolon",
        tree::STATEMENTS,
        stmts::statement_semicolon,
    ),
    ("ambiguous-variable-name", MODULE, stmts::ambiguous_variable),
    ("ambiguous-class-name", MODULE, stmts::ambiguous_class),
    ("ambiguous-function-name", MODULE, stmts::ambiguous_function),
    ("is-literal", &[k::EXPR_COMPARE], stmts::is_literal),
    (
        "type-comparison",
        &[k::EXPR_COMPARE],
        stmts::type_comparison,
    ),
    (
        "f-string-missing-placeholders",
        &[k::EXPR_F_STRING],
        stmts::f_string_missing_placeholders,
    ),
    ("bare-except", &[k::STMT_TRY], stmts::bare_except),
    (
        "default-except-not-last",
        &[k::STMT_TRY],
        stmts::default_except_not_last,
    ),
    (
        "lambda-assignment",
        &[k::STMT_ASSIGN, k::STMT_ANN_ASSIGN],
        stmts::lambda_assignment,
    ),
    (
        "invalid-print-syntax",
        &[k::EXPR_BIN_OP],
        stmts::invalid_print_syntax,
    ),
    (
        "multiple-starred-expressions",
        &[k::STMT_ASSIGN, k::STMT_ANN_ASSIGN],
        stmts::multiple_starred,
    ),
    (
        "break-outside-loop",
        &[k::STMT_BREAK],
        stmts::break_outside_loop,
    ),
    (
        "continue-outside-loop",
        &[k::STMT_CONTINUE],
        stmts::continue_outside_loop,
    ),
    (
        "return-outside-function",
        &[k::STMT_RETURN],
        stmts::return_outside_function,
    ),
    (
        "yield-outside-function",
        &[k::EXPR_YIELD, k::EXPR_YIELD_FROM, k::EXPR_AWAIT],
        stmts::yield_outside_function,
    ),
    (
        "get-attr-with-constant",
        CALL,
        lints::get_attr_with_constant,
    ),
    ("print", CALL, lints::print),
    ("pprint", CALL, lints::pprint),
    (
        "zip-without-explicit-strict",
        CALL,
        lints::zip_without_strict,
    ),
    (
        "unnecessary-collection-call",
        CALL,
        lints::unnecessary_collection_call,
    ),
    (
        "in-dict-keys",
        &[k::EXPR_COMPARE, k::COMPREHENSION, k::STMT_FOR],
        lints::in_dict_keys,
    ),
    ("negate-equal-op", UNARY, lints::negate_equal_op),
    ("negate-not-equal-op", UNARY, lints::negate_not_equal_op),
    ("double-negation", UNARY, lints::double_negation),
    (
        "if-expr-with-true-false",
        IF_EXPR,
        lints::if_expr_true_false,
    ),
    (
        "if-expr-with-false-true",
        IF_EXPR,
        lints::if_expr_false_true,
    ),
    (
        "if-expr-with-twisted-arms",
        IF_EXPR,
        lints::if_expr_twisted_arms,
    ),
    (
        "multiple-starts-ends-with",
        &[k::EXPR_BOOL_OP],
        lints::multiple_starts_ends_with,
    ),
    ("assert-false", &[k::STMT_ASSERT], lints::assert_false),
    (
        "set-attr-with-constant",
        &[k::STMT_EXPR],
        lints::set_attr_with_constant,
    ),
    (
        "unnecessary-placeholder",
        lints::BODIES,
        lints::unnecessary_placeholder,
    ),
    (
        "useless-object-inheritance",
        &[k::STMT_CLASS_DEF],
        lints::useless_object_inheritance,
    ),
    (
        "os-error-alias",
        &[k::STMT_RAISE, k::EXCEPT_HANDLER_EXCEPT_HANDLER],
        lints::os_error_alias_check,
    ),
    (
        "mutable-argument-default",
        &[k::PARAMETER_WITH_DEFAULT],
        lints::mutable_argument_default,
    ),
    (
        "redundant-tuple-in-exception-handler",
        HANDLER,
        lints::redundant_tuple_in_handler,
    ),
    (
        "raise-without-from-inside-except",
        HANDLER,
        lints::raise_without_from,
    ),
    (
        "unicode-literal",
        &[k::STRING_LITERAL],
        lints::unicode_literal,
    ),
    (
        "invalid-syntax",
        &[k::PARAMETERS, k::ARGUMENTS, k::STMT_NONLOCAL, k::EXPR_AWAIT],
        stmts::invalid_syntax,
    ),
    (
        "late-future-import",
        &[k::STMT_IMPORT_FROM],
        stmts::late_future_import,
    ),
    (
        "future-feature-not-defined",
        &[k::STMT_IMPORT_FROM],
        stmts::future_feature_not_defined,
    ),
];

fn build(meta: &'static RuleMeta, p: Params) -> Result<Box<dyn Rule>, String> {
    core_rules::checks::build(CHECKS, meta, p)
}
