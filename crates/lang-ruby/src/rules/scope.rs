//! What Ruby says to the shared scope rules (`core_rules::scope_rules`): `../../scope.rows`, and
//! which parameters are left alone.
//!
//! concept: scope-tables

use crate::generated::kind as k;
use core_rules::{ScopeLang, ScopeRule};
use core_syntax::SyntaxNodeExt;
use core_syntax::{SyntaxKind, SyntaxNode};

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
    let Some(body) = scope.child(k::STATEMENTS).or_else(|| scope.child(k::BEGIN)) else {
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
            !n.token(k::DOT).is_some()
                && super::tree::method_name(n).is_some_and(|m| m.text() == "binding")
        }
        k::DEF => {
            n.token(k::DOT).is_some()
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
        Some("raise") => call
            .child(k::ARGUMENTS)
            .and_then(|a| a.children().next())
            .is_some_and(|first| {
                first.kind() == k::CONSTANT_READ
                    && first.text().to_string().trim() == "NotImplementedError"
            }),
        _ => false,
    }
}

static LANG: ScopeLang = ScopeLang::new(include_str!("../../scope.rows")).skip_unused(skip_unused);

pub static UNUSED_LOCAL_VARIABLE: ScopeRule = ScopeRule::unused_local(&LANG);
pub static UNUSED_METHOD_ARGUMENT: ScopeRule = ScopeRule::unused_method_argument(&LANG);
pub static UNUSED_BLOCK_ARGUMENT: ScopeRule = ScopeRule::unused_block_argument(&LANG);
pub static UNDERSCORE_PREFIXED: ScopeRule = ScopeRule::underscore_prefixed(&LANG);
