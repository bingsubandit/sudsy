//! What PHP says to the shared scope rules (`core_rules::scope_rules`): a variable is declared by
//! the first write to it in its function (or by a parameter, `use`, `global`, `static`), a
//! function sees none of the names around it, and a construct that can create or read variables
//! by name makes the function's names unknowable.

use core_rules::{ScopeLang, ScopeRule};
use core_semantics::scopes::{Declares, Flags, Flow, Place, Row, Sel};

const fn function(kind: &'static str) -> Row {
    Row {
        kind,
        var_scope: true,
        closed: true,
        ..Row::NONE
    }
}

const fn flows(kind: &'static str, children: &'static [(Sel, Flow)]) -> Row {
    Row {
        kind,
        children,
        ..Row::NONE
    }
}

const fn assigns(kind: &'static str, children: &'static [(Sel, Flow)]) -> Row {
    Row {
        op_node: true,
        ..flows(kind, children)
    }
}

const fn dynamic(kind: &'static str) -> Row {
    Row {
        kind,
        dynamic: true,
        ..Row::NONE
    }
}

const ROWS: &[Row] = &[
    Row {
        kind: "Program",
        var_scope: true,
        ..Row::NONE
    },
    function("Function"),
    function("Method"),
    function("Closure"),
    Row {
        kind: "ArrowFunction",
        var_scope: true,
        ..Row::NONE
    },
    Row {
        kind: "FunctionLikeParameter",
        declares: Some(Declares {
            flags: Flags::VALUE.union(Flags::PARAMETER),
            ..Declares::NONE
        }),
        children: &[(Sel::Field("name"), Flow::Declare)],
        ..Row::NONE
    },
    Row {
        kind: "DirectVariable",
        reference: true,
        implicit: Some((Flags::VALUE.union(Flags::LET), Place::Var)),
        ..Row::NONE
    },
    // A call's name is no variable; `extract` and `parse_str` make the scope dynamic.
    Row {
        kind: "FunctionCall",
        tokens: &[(Sel::First, Flow::Type)],
        ..Row::NONE
    },
    assigns("Assignment", &[(Sel::Field("left"), Flow::Assigns)]),
    assigns("UnaryPrefix", &[(Sel::All, Flow::Assigns)]),
    assigns("UnaryPostfix", &[(Sel::All, Flow::Assigns)]),
    // Destructuring and element writes pass the write on to the variable they fill.
    flows("Array", &[(Sel::All, Flow::Through)]),
    flows("LegacyArray", &[(Sel::All, Flow::Through)]),
    flows("List", &[(Sel::All, Flow::Through)]),
    flows("ValueArrayElement", &[(Sel::All, Flow::Through)]),
    flows(
        "KeyValueArrayElement",
        &[(Sel::Field("value"), Flow::Through)],
    ),
    flows("ArrayAppend", &[(Sel::First, Flow::Through)]),
    flows("ArrayAccess", &[(Sel::Field("object"), Flow::Through)]),
    flows("ForeachKeyValueTarget", &[(Sel::All, Flow::Write)]),
    flows("ForeachValueTarget", &[(Sel::All, Flow::Write)]),
    flows(
        "TryCatchClause",
        &[(Sel::Kind("DirectVariable"), Flow::Write)],
    ),
    flows("Global", &[(Sel::All, Flow::Write)]),
    flows("StaticConcreteItem", &[(Sel::First, Flow::Write)]),
    flows("StaticAbstractItem", &[(Sel::First, Flow::Write)]),
    Row {
        kind: "ClosureUseClauseVariable",
        declares: Some(Declares {
            flags: Flags::VALUE.union(Flags::PARAMETER),
            ..Declares::NONE
        }),
        children: &[(Sel::All, Flow::Capture)],
        ..Row::NONE
    },
    flows(
        "NamedArgument",
        &[(Sel::Kind("DirectVariable"), Flow::Write)],
    ),
    flows("Unset", &[(Sel::All, Flow::Type)]),
    // A variable passed to a call may be filled by it (`preg_match($p, $s, $m)`): no verdict.
    flows("PositionalArgument", &[(Sel::All, Flow::Write)]),
    // Tested, not read.
    flows("IssetConstruct", &[(Sel::All, Flow::Type)]),
    flows("EmptyConstruct", &[(Sel::All, Flow::Type)]),
    flows(
        "StaticPropertyAccess",
        &[(Sel::Field("member"), Flow::Type)],
    ),
    flows("PropertyAbstractItem", &[(Sel::First, Flow::Type)]),
    flows("PropertyConcreteItem", &[(Sel::First, Flow::Type)]),
    dynamic("IndirectVariable"),
    dynamic("NestedVariable"),
    dynamic("IncludeConstruct"),
    dynamic("IncludeOnceConstruct"),
    dynamic("RequireConstruct"),
    dynamic("RequireOnceConstruct"),
    dynamic("EvalConstruct"),
];

const GLOBALS: &str = "$this $GLOBALS $_SERVER $_GET $_POST $_FILES $_COOKIE $_SESSION $_REQUEST $_ENV \
    $argc $argv $http_response_header";

static LANG: ScopeLang = ScopeLang::new(ROWS, &["Variable", "Identifier"])
    .dynamic(&["extract", "parse_str", "mb_parse_str"])
    .globals(GLOBALS)
    .top_unchecked();

pub static UNDEFINED_VARIABLE: ScopeRule = ScopeRule::undefined_variable(&LANG);
