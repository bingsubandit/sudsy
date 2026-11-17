//! The rules of JavaScript and TypeScript: pattern files in `rules/*.yaml`, and the `check`
//! family they name for ESLint's core rules that read the syntax tree. `cargo xtask new-rule js
//! <id> --rust` adds a module under `rules/` and lists it here.

mod binder;
mod dupes;
mod escapes;
mod imports;
mod misc;
mod reach;
mod regexp;
mod scope;
mod shadow;
mod stmts;
mod ts;
mod unused;

use crate::generated::kind as k;
use core_rules::tree::significant_range;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

pub static RULES: &[&dyn Rule] = &[];

pub static FAMILIES: &[Family] = &[FAMILY];

const FAMILY: Family = Family {
    name: "check",
    build,
};

pub(super) use core_rules::checks::{Entry, NodeCheck as Check};
pub(crate) use dupes::key_name;

const CHECKS: &[Entry] = &[
    (
        "dupe-class-members",
        &[k::CLASS_BODY],
        dupes::dupe_class_members,
    ),
    ("dupe-else-if", &[k::IF_STATEMENT], dupes::dupe_else_if),
    ("useless-catch", &[k::TRY_STATEMENT], misc::useless_catch),
    (
        "self-assign",
        &[k::ASSIGNMENT_EXPRESSION],
        misc::self_assign,
    ),
    (
        "use-isnan",
        &[k::BINARY_EXPRESSION, k::SWITCH_STATEMENT, k::SWITCH_CASE],
        misc::use_isnan,
    ),
    ("valid-typeof", &[k::BINARY_EXPRESSION], misc::valid_typeof),
    (
        "extra-boolean-cast",
        &[k::UNARY_EXPRESSION, k::CALL_EXPRESSION, k::NEW_EXPRESSION],
        misc::extra_boolean_cast,
    ),
    (
        "unsafe-finally",
        &[
            k::RETURN_STATEMENT,
            k::THROW_STATEMENT,
            k::BREAK_STATEMENT,
            k::CONTINUE_STATEMENT,
        ],
        stmts::unsafe_finally,
    ),
    ("unreachable", reach::UNITS, reach::unreachable),
    (
        "getter-return",
        &[k::METHOD_DEFINITION, k::OBJECT_PROPERTY],
        reach::getter_return,
    ),
    ("require-yield", &[k::FUNCTION], stmts::require_yield),
    (
        "useless-escape",
        &[k::STRING_LITERAL, k::TEMPLATE_LITERAL, k::REG_EXP_LITERAL],
        escapes::useless_escape,
    ),
    (
        "regex-spaces",
        &[k::REG_EXP_LITERAL, k::NEW_EXPRESSION, k::CALL_EXPRESSION],
        escapes::regex_spaces,
    ),
    (
        "empty-character-class",
        &[k::REG_EXP_LITERAL],
        escapes::empty_character_class,
    ),
    (
        "irregular-whitespace",
        &[k::PROGRAM],
        escapes::irregular_whitespace,
    ),
    (
        "loss-of-precision",
        &[k::NUMERIC_LITERAL],
        escapes::loss_of_precision,
    ),
    ("fallthrough", &[k::SWITCH_STATEMENT], reach::fallthrough),
    (
        "unused-expressions",
        &[k::EXPRESSION_STATEMENT],
        stmts::unused_expressions,
    ),
    (
        "unexpected-multiline",
        &[
            k::COMPUTED_MEMBER_EXPRESSION,
            k::CALL_EXPRESSION,
            k::TAGGED_TEMPLATE_EXPRESSION,
        ],
        stmts::unexpected_multiline,
    ),
    (
        "duplicate-enum-values",
        &[k::TS_ENUM_DECLARATION],
        stmts::duplicate_enum_values,
    ),
    (
        "control-regex",
        &[k::REG_EXP_LITERAL, k::NEW_EXPRESSION, k::CALL_EXPRESSION],
        regexp::control_regex,
    ),
    (
        "misleading-character-class",
        &[k::REG_EXP_LITERAL, k::NEW_EXPRESSION, k::CALL_EXPRESSION],
        regexp::misleading_class,
    ),
    ("undef", &[k::PROGRAM], scope::undef),
    ("const-assign", &[k::PROGRAM], scope::const_assign),
    ("class-assign", &[k::PROGRAM], scope::class_assign),
    ("func-assign", &[k::PROGRAM], scope::func_assign),
    ("import-assign", &[k::PROGRAM], scope::import_assign),
    ("redeclare", &[k::PROGRAM], scope::redeclare),
    ("use-before-define", &[k::PROGRAM], scope::use_before_define),
    ("shadow", &[k::PROGRAM], shadow::shadow),
    ("prefer-const", &[k::PROGRAM], binder::prefer_const),
    (
        "prefer-rest-params",
        &[k::PROGRAM],
        binder::prefer_rest_params,
    ),
    (
        "unsafe-function-type",
        &[k::PROGRAM],
        binder::unsafe_function,
    ),
    ("wrapper-object-types", &[k::PROGRAM], binder::wrapper_types),
    ("unused-vars", &[k::PROGRAM], unused::unused_vars),
    (
        "require-imports",
        &[k::CALL_EXPRESSION, k::TS_EXTERNAL_MODULE_REFERENCE],
        imports::require_imports,
    ),
    (
        "type-imports",
        &[k::PROGRAM, k::TS_IMPORT_TYPE],
        imports::type_imports,
    ),
    ("ban-ts-comment", &[k::PROGRAM], ts::ban_ts_comment),
    (
        "extra-non-null",
        &[k::TS_NON_NULL_EXPRESSION],
        ts::extra_non_null,
    ),
    (
        "misused-new",
        &[
            k::METHOD_DEFINITION,
            k::TS_CONSTRUCT_SIGNATURE_DECLARATION,
            k::TS_METHOD_SIGNATURE,
        ],
        ts::misused_new,
    ),
    (
        "prefer-as-const",
        &[k::TS_LITERAL_TYPE],
        ts::prefer_as_const,
    ),
    (
        "inferrable-types",
        &[
            k::VARIABLE_DECLARATOR,
            k::FORMAL_PARAMETER,
            k::PROPERTY_DEFINITION,
            k::ACCESSOR_PROPERTY,
        ],
        ts::inferrable_types,
    ),
    ("array-type", &[k::TS_TYPE_REFERENCE], ts::array_type),
    (
        "triple-slash-reference",
        &[k::PROGRAM],
        ts::triple_slash_reference,
    ),
    (
        "adjacent-overloads",
        &[
            k::CLASS_BODY,
            k::PROGRAM,
            k::TS_MODULE_BLOCK,
            k::TS_INTERFACE_BODY,
            k::FUNCTION_BODY,
            k::BLOCK_STATEMENT,
            k::TS_TYPE_LITERAL,
        ],
        ts::adjacent_overloads,
    ),
];

fn build(meta: &'static RuleMeta, p: Params) -> Result<Box<dyn Rule>, String> {
    core_rules::checks::build(CHECKS, meta, p)
}

/// The span of `n` without its surrounding trivia.
fn span(ctx: &RuleContext<'_>, n: &SyntaxNode) -> TextRange {
    significant_range(n, ctx.file.grammar)
}

/// The own tokens of `n` that are code (not trivia).
fn own_code<'g>(n: &SyntaxNode, g: &'g Grammar) -> impl Iterator<Item = SyntaxToken> + 'g {
    (n.children_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()))
}

/// The first token of `n` that is code (the node's own first token may be leading trivia).
fn code_token(n: &SyntaxNode, g: &Grammar) -> Option<SyntaxToken> {
    (n.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| !g.is_trivia(t.kind()))
}

/// Does `n` have an own code token with this text?
fn has_own(n: &SyntaxNode, g: &Grammar, text: &str) -> bool {
    own_code(n, g).any(|t| t.text() == text)
}

/// The text of the code tokens of `n` joined: the node without its whitespace and comments.
fn code_text(n: &SyntaxNode, g: &Grammar) -> String {
    (n.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()))
        .map(|t| t.text().to_string())
        .collect()
}

/// The expression inside any number of parentheses.
fn un(mut n: SyntaxNode) -> SyntaxNode {
    while n.kind() == k::PARENTHESIZED_EXPRESSION {
        match n.children().next() {
            Some(inner) => n = inner,
            None => break,
        }
    }
    n
}

/// The operator of a unary, binary, logical or assignment expression.
fn operator(n: &SyntaxNode, g: &Grammar) -> String {
    let ops = |t: &SyntaxToken| {
        matches!(
            t.kind(),
            k::OPERATOR | k::KEYWORD | k::L_ANGLE | k::R_ANGLE | k::PIPE
        )
    };
    own_code(n, g)
        .find(ops)
        .map_or_else(String::new, |t| t.text().to_string())
}

/// The condition of an `if`, `while`, `do`, `for` or `?:`, as written (parentheses included).
fn test_of(n: &SyntaxNode) -> Option<SyntaxNode> {
    match n.kind() {
        k::DO_WHILE_STATEMENT => n.children().nth(1),
        k::FOR_STATEMENT => {
            let mut semicolons = 0;
            for e in n.children_with_tokens() {
                match e {
                    SyntaxElement::Token(t) if t.kind() == k::SEMICOLON => semicolons += 1,
                    SyntaxElement::Node(c) if semicolons == 1 => return Some(c),
                    _ => {}
                }
            }
            None
        }
        _ => n.children().next(),
    }
}
