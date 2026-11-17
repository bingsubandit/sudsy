//! Checks on class and module bodies: access modifiers, constants, constructors.

use super::tree::{child, has_token, method_name, span, tokens};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::checks::NodeCheck;
use core_syntax::{SyntaxNode, SyntaxToken};

/// A receiver-less call with no arguments: its name.
fn bare_call(call: &SyntaxNode) -> Option<SyntaxToken> {
    if call.kind() != k::CALL
        || child(call, k::ARGUMENTS).is_some()
        || has_token(call, k::L_PAREN)
        || has_token(call, k::DOT)
    {
        return None;
    }
    method_name(call)
}

/// `private`, `protected` or `public` on a line of its own.
fn bare_modifier(call: &SyntaxNode) -> Option<SyntaxToken> {
    bare_call(call).filter(|n| matches!(n.text(), "private" | "protected" | "public"))
}

/// The name of a method definition.
fn def_name(def: &SyntaxNode) -> Option<SyntaxToken> {
    tokens(def)
        .skip(1)
        .find(|t| matches!(t.kind(), k::IDENTIFIER | k::CONSTANT | k::OPERATOR))
}

/// The names `private_class_method` makes private anywhere in `body`.
fn private_class_methods(body: &SyntaxNode) -> Vec<String> {
    let mut names = Vec::new();
    for call in body.descendants().filter(|d| d.kind() == k::CALL) {
        if method_name(&call).is_some_and(|n| n.text() == "private_class_method")
            && let Some(args) = child(&call, k::ARGUMENTS)
        {
            for a in args.children().filter(|a| a.kind() == k::SYMBOL) {
                let text = a.text().to_string();
                names.push(
                    text.trim()
                        .trim_start_matches(':')
                        .trim_matches(['"', '\''])
                        .to_owned(),
                );
            }
        }
    }
    names
}

/// `private` before `def self.name`, which it does not affect.
pub fn ineffective_access_modifier(
    check: &NodeCheck,
    node: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
) {
    let Some(body) = child(node, k::STATEMENTS) else {
        return;
    };
    let mut modifier: Option<SyntaxToken> = None;
    let mut ignored: Option<Vec<String>> = None;
    let mut scan = vec![body.clone()];
    while let Some(statements) = scan.pop() {
        for stmt in statements.children() {
            match stmt.kind() {
                k::CALL => {
                    if let Some(m) = bare_modifier(&stmt) {
                        modifier = Some(m);
                    }
                }
                k::DEF if has_token(&stmt, k::DOT) => {
                    let Some(m) = modifier.as_ref().filter(|m| m.text() != "public") else {
                        continue;
                    };
                    let ignored = ignored.get_or_insert_with(|| private_class_methods(&body));
                    let name = def_name(&stmt);
                    if name.is_some_and(|n| ignored.iter().any(|i| i == n.text())) {
                        continue;
                    }
                    if let Some(def) = tokens(&stmt).next() {
                        let range = def.text_range();
                        check.hit(ctx, range, m.text());
                    }
                }
                k::BEGIN => scan.extend(child(&stmt, k::STATEMENTS)),
                _ => {}
            }
        }
    }
}

/// A constant or class defined inside a block (it belongs to the enclosing scope, not the block).
pub fn constant_definition_in_block(
    check: &NodeCheck,
    node: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
) {
    let Some(statements) = node.parent().filter(|p| p.kind() == k::STATEMENTS) else {
        return;
    };
    let Some(block) = statements
        .parent()
        .filter(|b| matches!(b.kind(), k::BLOCK | k::LAMBDA))
    else {
        return;
    };
    if block.kind() == k::BLOCK
        && block
            .parent()
            .and_then(|call| method_name(&call))
            .is_some_and(|n| n.text() == "enums")
    {
        return;
    }
    let range = span(ctx, node);
    check.hit(ctx, range, "");
}

/// The superclass of a `class` node.
fn superclass(class: &SyntaxNode) -> Option<SyntaxNode> {
    if !tokens(class).any(|t| t.text() == "<") {
        return None;
    }
    class
        .children()
        .nth(1)
        .filter(|n| n.kind() != k::STATEMENTS)
}

fn is_stateless(superclass: &SyntaxNode) -> bool {
    let text = superclass.text().to_string();
    matches!(
        text.trim().trim_start_matches("::"),
        "Object" | "BasicObject"
    )
}

const CALLBACKS: &[&str] = &[
    "inherited",
    "method_added",
    "method_removed",
    "method_undefined",
    "singleton_method_added",
    "singleton_method_removed",
    "singleton_method_undefined",
];

/// A constructor of a subclass, or a lifecycle callback, that does not call `super`.
pub fn missing_super(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(name) = def_name(node) else { return };
    let singleton = has_token(node, k::DOT);
    let constructor = name.text() == "initialize" && !singleton;
    let callback = CALLBACKS.contains(&name.text())
        && node
            .ancestors()
            .any(|a| matches!(a.kind(), k::CLASS | k::SINGLETON_CLASS | k::MODULE));
    if !(constructor || callback)
        || node
            .descendants()
            .any(|d| matches!(d.kind(), k::SUPER | k::FORWARDING_SUPER))
    {
        return;
    }
    let stateful = constructor && {
        match node.ancestors().find(|a| a.kind() == k::BLOCK) {
            Some(block) => class_new_parent(&block).is_some_and(|p| !is_stateless(&p)),
            None => node
                .ancestors()
                .find(|a| a.kind() == k::CLASS)
                .and_then(|class| superclass(&class))
                .is_some_and(|p| !is_stateless(&p)),
        }
    };
    if stateful {
        let range = span(ctx, node);
        ctx.report(
            range,
            "call `super` to initialize the state of the parent class",
        );
    } else if callback {
        let range = span(ctx, node);
        check.hit(ctx, range, name.text());
    }
}

/// The argument of `Class.new(parent) do ... end` for its block.
fn class_new_parent(block: &SyntaxNode) -> Option<SyntaxNode> {
    let call = block.parent().filter(|c| c.kind() == k::CALL)?;
    let object = call.children().next()?;
    let args = child(&call, k::ARGUMENTS)?;
    let mut parents = args.children();
    let (parent, None) = (parents.next()?, parents.next()) else {
        return None;
    };
    let is_class = object.kind() == k::CONSTANT_READ
        && object.text().to_string().trim().trim_start_matches("::") == "Class";
    (is_class && method_name(&call).is_some_and(|n| n.text() == "new")).then_some(parent)
}

type Scan = (Visibility, Option<SyntaxNode>);

/// An access modifier that changes nothing: the same visibility again, or no method after it.
pub fn useless_access_modifier(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let body = match node.kind() {
        k::CALL if is_eval_call(node) => child(node, k::BLOCK),
        k::CALL => None,
        _ => Some(node.clone()),
    };
    let Some(statements) = body.and_then(|b| child(&b, k::STATEMENTS)) else {
        return;
    };
    let mut stmts = statements.children();
    let (first, second) = (stmts.next(), stmts.next());
    if node.kind() == k::PROGRAM {
        for stmt in statements.children() {
            if let Some(m) = bare_call(&stmt).filter(|m| is_access(m.text())) {
                let range = span(ctx, &stmt);
                check.hit(ctx, range, m.text());
            }
        }
    } else if let (Some(only), None) = (&first, &second) {
        if let Some(m) = bare_modifier(only) {
            let range = span(ctx, only);
            check.hit(ctx, range, m.text());
        }
    } else {
        let mut state: Scan = (Visibility::Public, None);
        scan_scope(check, &statements, &mut state, ctx);
        if let (vis, Some(unused)) = state {
            let range = span(ctx, &unused);
            check.hit(ctx, range, vis.name());
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
enum Visibility {
    Public,
    Protected,
    Private,
    ModuleFunction,
}

impl Visibility {
    fn name(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Protected => "protected",
            Visibility::Private => "private",
            Visibility::ModuleFunction => "module_function",
        }
    }
}

fn is_access(name: &str) -> bool {
    matches!(name, "private" | "protected" | "public" | "module_function")
}

/// `class_eval`, `Class.new`, `Struct.new` and the like with a block: a scope of its own.
fn is_eval_call(call: &SyntaxNode) -> bool {
    if call.kind() != k::CALL || child(call, k::BLOCK).is_none() {
        return false;
    }
    let Some(name) = method_name(call) else {
        return false;
    };
    if matches!(name.text(), "class_eval" | "instance_eval") {
        return true;
    }
    let object = call
        .children()
        .next()
        .filter(|o| o.kind() == k::CONSTANT_READ)
        .map(|o| o.text().to_string());
    matches!(
        (object.as_deref().map(str::trim), name.text()),
        (Some("Class" | "Module" | "Struct"), "new") | (Some("Data"), "define")
    )
}

/// A method the following modifier applies to: `def` (not `def self.x`), `attr_*`, `define_method`.
fn defines_method(n: &SyntaxNode) -> bool {
    match n.kind() {
        k::DEF => !has_token(n, k::DOT),
        k::CALL => {
            !has_token(n, k::DOT)
                && method_name(n).is_some_and(|m| {
                    matches!(
                        m.text(),
                        "attr" | "attr_reader" | "attr_writer" | "attr_accessor" | "define_method"
                    )
                })
        }
        _ => false,
    }
}

fn scan_scope(check: &NodeCheck, node: &SyntaxNode, state: &mut Scan, ctx: &mut RuleContext<'_>) {
    for c in node.children() {
        let bare = bare_call(&c);
        if let Some(m) = bare.as_ref().filter(|m| is_access(m.text())) {
            let new = match m.text() {
                "private" => Visibility::Private,
                "protected" => Visibility::Protected,
                "module_function" => Visibility::ModuleFunction,
                _ => Visibility::Public,
            };
            if new == state.0 {
                let range = span(ctx, &c);
                check.hit(ctx, range, new.name());
            } else {
                if let Some(unused) = state.1.take() {
                    let range = span(ctx, &unused);
                    check.hit(ctx, range, state.0.name());
                }
                *state = (new, Some(c));
            }
        } else if bare.is_some_and(|m| m.text() == "private_class_method") {
            let range = span(ctx, &c);
            check.hit(ctx, range, "private_class_method");
        } else if defines_method(&c) {
            state.1 = None;
        } else if matches!(c.kind(), k::CLASS | k::MODULE | k::SINGLETON_CLASS)
            || is_eval_call(&c)
            || (c.kind() == k::DEF && has_token(&c, k::DOT))
        {
            continue;
        } else {
            scan_scope(check, &c, state, ctx);
        }
    }
}

/// `class Foo < Exception`, `Class.new(Exception)`: it is rescued by nothing but `rescue Exception`.
pub fn inherit_exception(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let parent = if node.kind() == k::CLASS {
        superclass(node)
    } else if method_name(node).is_some_and(|n| n.text() == "new")
        && node
            .children()
            .next()
            .is_some_and(|o| o.text().to_string().trim() == "Class")
    {
        child(node, k::ARGUMENTS).and_then(|a| a.children().next())
    } else {
        None
    };
    if let Some(parent) = parent.filter(|p| {
        p.kind() == k::CONSTANT_READ
            && p.text().to_string().trim().trim_start_matches("::") == "Exception"
    }) {
        let range = span(ctx, &parent);
        check.hit(ctx, range, "");
    }
}

/// A method whose body is a bare `super`.
pub fn useless_method_definition(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(body) = child(node, k::STATEMENTS) else {
        return;
    };
    let mut all = body.children();
    let (Some(only), None) = (all.next(), all.next()) else {
        return;
    };
    let flexible = node.descendants().any(|d| {
        matches!(
            d.kind(),
            k::OPTIONAL_PARAMETER
                | k::OPTIONAL_KEYWORD_PARAMETER
                | k::REST_PARAMETER
                | k::KEYWORD_REST_PARAMETER
        )
    });
    let modified = node.parent().is_some_and(|p| p.kind() == k::ARGUMENTS);
    let delegating = match only.kind() {
        k::FORWARDING_SUPER => child(&only, k::BLOCK).is_none(),
        k::SUPER => {
            let params = child(node, k::PARAMETERS)
                .map(|p| {
                    p.children()
                        .map(|c| c.text().to_string().trim().to_owned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let args = child(&only, k::ARGUMENTS)
                .map(|a| {
                    a.children()
                        .map(|c| c.text().to_string().trim().to_owned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            child(&only, k::BLOCK).is_none()
                && params == args
                && child(node, k::PARAMETERS)
                    .is_some_and(|p| p.children().all(|c| c.kind() == k::REQUIRED_PARAMETER))
        }
        _ => false,
    };
    if delegating && !flexible && !modified {
        let range = span(ctx, node);
        check.hit(ctx, range, "");
    }
}

/// `return value` in `initialize` or a setter, which ignore the value.
pub fn return_in_void_context(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if child(node, k::ARGUMENTS).is_none() {
        return;
    }
    let mut scopes = node.ancestors().skip(1);
    let void = scopes.find(|a| a.kind() == k::DEF).and_then(|def| {
        let name = def_name(&def)?;
        let (text, setter) = (
            name.text(),
            name.text().ends_with('=') && !name.text().ends_with("=="),
        );
        (text == "initialize" || setter).then(|| text.to_owned())
    });
    let Some(def) = void else { return };
    let scoped = node
        .ancestors()
        .skip(1)
        .take_while(|a| a.kind() != k::DEF)
        .any(|a| {
            a.kind() == k::LAMBDA
                || a.kind() == k::BLOCK
                    && a.parent().and_then(|c| method_name(&c)).is_some_and(|n| {
                        matches!(
                            n.text(),
                            "lambda" | "define_method" | "define_singleton_method"
                        )
                    })
        });
    if !scoped && let Some(keyword) = tokens(node).next() {
        check.hit(ctx, keyword.text_range(), &def);
    }
}
