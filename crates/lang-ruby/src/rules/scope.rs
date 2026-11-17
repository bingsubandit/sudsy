//! What Ruby says to the shared scope rules (`core_rules::scope_rules`): a local variable exists
//! from its first assignment in the method, class body or program (or in the block that makes it,
//! when no scope around the block has it), a method, class or module sees none of the names
//! around it, a block sees all of them.

use super::tree::child;
use crate::generated::kind as k;
use core_rules::{ScopeLang, ScopeRule};
use core_semantics::scopes::{Declares, Flags, Flow, Place, Row, Sel};
use core_syntax::{SyntaxKind, SyntaxNode};

const fn closed(kind: &'static str) -> Row {
    Row {
        kind,
        var_scope: true,
        closed: true,
        ..Row::NONE
    }
}

/// A parameter: the first name token of the node.
const fn parameter(kind: &'static str) -> Row {
    Row {
        kind,
        declares: Some(Declares {
            flags: Flags::VALUE.union(Flags::PARAMETER),
            ..Declares::NONE
        }),
        tokens: &[(Sel::First, Flow::Declare)],
        ..Row::NONE
    }
}

/// A node whose first name token is a local variable that is written (`x = 1`, `x += 1`; a target
/// of `a, b = ...`, `for x in`, `rescue => x`: a plain write).
const fn writes(kind: &'static str) -> Row {
    Row {
        kind,
        tokens: &[(Sel::First, Flow::Assign("="))],
        implicit: Some((Flags::VALUE.union(Flags::LET), Place::Unseen)),
        ..Row::NONE
    }
}

const ROWS: &[Row] = &[
    Row {
        kind: "Program",
        var_scope: true,
        ..Row::NONE
    },
    closed("Def"),
    closed("Class"),
    closed("Module"),
    closed("SingletonClass"),
    parameter("RequiredParameter"),
    parameter("OptionalParameter"),
    parameter("RestParameter"),
    parameter("RequiredKeywordParameter"),
    parameter("OptionalKeywordParameter"),
    parameter("KeywordRestParameter"),
    parameter("BlockParameter"),
    Row {
        kind: "LocalVariableRead",
        reference: true,
        ..Row::NONE
    },
    writes("LocalVariableWrite"),
    writes("LocalVariableOperatorWrite"),
    writes("LocalVariableOrWrite"),
    writes("LocalVariableAndWrite"),
    Row {
        tokens: &[(Sel::First, Flow::Write)],
        ..writes("LocalVariableTarget")
    },
];

/// Parameters that are left alone: those of an empty method or block, of a method that raises
/// NotImplementedError, and of a method that calls a bare `super` (which passes them all on); a
/// `&block` of a method that yields.
fn skip_unused(scope: &SyntaxNode, param: SyntaxKind, name: &str) -> bool {
    if param == k::BLOCK_PARAMETER && scope.descendants().any(|d| d.kind() == k::YIELD) {
        return true;
    }
    if scope.descendants().any(|d| uses_name_dynamically(&d, name)) {
        return true;
    }
    let Some(body) = child(scope, k::STATEMENTS).or_else(|| child(scope, k::BEGIN)) else {
        return true;
    };
    if scope.kind() == k::DEF && body.descendants().any(|d| d.kind() == k::FORWARDING_SUPER) {
        return true;
    }
    let mut statements = body.children();
    let (Some(only), None) = (statements.next(), statements.next()) else {
        return false;
    };
    body.kind() == k::STATEMENTS && raises_not_implemented(&only)
}

/// A call of `binding` (it sees every variable), `def name.x` and `class << name` (the parameter is
/// the receiver).
fn uses_name_dynamically(n: &SyntaxNode, name: &str) -> bool {
    match n.kind() {
        k::CALL => {
            !super::tree::has_token(n, k::DOT)
                && super::tree::method_name(n).is_some_and(|m| m.text() == "binding")
        }
        k::DEF => {
            super::tree::has_token(n, k::DOT)
                && n.children()
                    .next()
                    .is_some_and(|r| r.text().to_string().trim() == name)
        }
        k::SINGLETON_CLASS => n
            .children()
            .next()
            .is_some_and(|e| e.text().to_string().trim() == name),
        _ => false,
    }
}

/// `raise NotImplementedError`, `raise NotImplementedError, "..."` or `fail ...`.
fn raises_not_implemented(call: &SyntaxNode) -> bool {
    if call.kind() != k::CALL {
        return false;
    }
    let name = super::tree::method_name(call);
    match name.as_ref().map(|n| n.text()) {
        Some("fail") => true,
        Some("raise") => child(call, k::ARGUMENTS)
            .and_then(|a| a.children().next())
            .is_some_and(|first| {
                first.kind() == k::CONSTANT_READ
                    && first.text().to_string().trim() == "NotImplementedError"
            }),
        _ => false,
    }
}

static LANG: ScopeLang = ScopeLang::new(ROWS, &["Identifier", "Label"])
    .labels(&["Label"])
    .arguments(&["Def"], &["Block", "Lambda"], skip_unused)
    .interpolated("HeredocBody")
    .unused_prefix("_");

pub static UNUSED_LOCAL_VARIABLE: ScopeRule = ScopeRule::unused_local(&LANG);
pub static UNUSED_METHOD_ARGUMENT: ScopeRule = ScopeRule::unused_method_argument(&LANG);
pub static UNUSED_BLOCK_ARGUMENT: ScopeRule = ScopeRule::unused_block_argument(&LANG);
pub static UNDERSCORE_PREFIXED: ScopeRule = ScopeRule::underscore_prefixed(&LANG);
