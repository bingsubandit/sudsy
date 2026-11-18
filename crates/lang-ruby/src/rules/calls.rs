//! Checks on the shape of a call: what its arguments and its block are.
//!
//! concept: call-shape-checks

use super::tree::{method_name, span};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::checks::NodeCheck;
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;
use core_syntax::roles::code_tokens;

fn text_of<'a>(ctx: &RuleContext<'a>, n: &SyntaxNode) -> &'a str {
    &ctx.file.text[span(ctx, n)]
}

/// The first argument of a call.
fn first_argument(call: &SyntaxNode) -> Option<SyntaxNode> {
    call.child(k::ARGUMENTS).and_then(|a| a.children().next())
}

fn is_constant(ctx: &RuleContext<'_>, n: &SyntaxNode, name: &str) -> bool {
    n.kind() == k::CONSTANT_READ && text_of(ctx, n).trim_start_matches("::") == name
}

/// Methods whose block is the point of the call: `puts items.map do ... end` hands the `do` block
/// to `puts`.
const BLOCK_METHODS: &[&str] = &[
    "map",
    "collect",
    "flat_map",
    "collect_concat",
    "select",
    "filter",
    "find_all",
    "reject",
    "find",
    "detect",
    "each",
    "each_with_object",
    "each_with_index",
    "reduce",
    "inject",
    "sort_by",
    "min_by",
    "max_by",
    "group_by",
    "filter_map",
];

fn is_lambda_or_proc(call: &SyntaxNode) -> bool {
    method_name(call).is_some_and(|n| matches!(n.text(), "lambda" | "proc"))
}

/// `foo a { }`: the braces bind to `a`, not to `foo`; `foo a.map do ... end`: the `do` block binds
/// to `foo`.
pub fn ambiguous_block_association(
    check: &NodeCheck,
    node: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
) {
    let Some(args) = node.child(k::ARGUMENTS) else {
        return;
    };
    if node.token(k::L_PAREN).is_some() {
        return;
    }
    let Some(last) = args.children().last().filter(|l| l.kind() == k::CALL) else {
        return;
    };
    if let Some(block) = node.child(k::BLOCK) {
        let first = code_tokens(&block).next();
        if first.is_some_and(|t| t.text() == "do") {
            let end = span(ctx, &args).end();
            let found = last
                .descendants()
                .filter(|d| d.kind() == k::CALL)
                .find(|d| {
                    d.child(k::ARGUMENTS).is_none()
                        && method_name(d).is_some_and(|n| BLOCK_METHODS.contains(&n.text()))
                        && span(ctx, d).end() == end
                });
            if let Some(inner) = found {
                let range = span(ctx, &inner);
                ctx.report(
                    range,
                    "the `do` block binds to the outer call, not to this call: use braces or a variable",
                );
            }
        }
        return;
    }
    if last.child(k::BLOCK).is_none()
        || last.child(k::ARGUMENTS).is_some()
        || last.token(k::L_PAREN).is_some()
        || is_lambda_or_proc(&last)
        || node.token(k::L_BRACKET).is_some()
        || node.token(k::ASSIGN).is_some()
        || method_name(node).is_some_and(|n| n.kind() == k::OPERATOR)
    {
        return;
    }
    let range = span(ctx, node);
    let name = core_rules::tree::significant_range(&last, ctx.file.grammar);
    let text = &ctx.file.text[name];
    check.hit(ctx, range, text);
}

/// `raise Exception`: it is rescued by nothing but `rescue Exception`.
pub fn raise_exception(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if node.token(k::DOT).is_some()
        || !method_name(node).is_some_and(|n| matches!(n.text(), "raise" | "fail"))
    {
        return;
    }
    let Some(arg) = first_argument(node) else {
        return;
    };
    let class = if arg.kind() == k::CALL && method_name(&arg).is_some_and(|n| n.text() == "new") {
        arg.children().next()
    } else {
        Some(arg)
    };
    if let Some(class) = class.filter(|c| is_constant(ctx, c, "Exception")) {
        let range = span(ctx, &class);
        check.hit(ctx, range, "");
    }
}

/// `foo (1)`: the parentheses are around the argument, not the arguments.
pub fn parentheses_as_grouped_expression(
    check: &NodeCheck,
    node: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
) {
    let Some(args) = node.child(k::ARGUMENTS) else {
        return;
    };
    let mut all = args.children();
    let (Some(group), None) = (
        all.next().filter(|g| g.kind() == k::PARENTHESES),
        all.next(),
    ) else {
        return;
    };
    let spaced = args
        .prev_sibling_or_token()
        .is_some_and(|p| p.kind() == k::WHITESPACE);
    let setter = node.token(k::ASSIGN).is_some() || node.token(k::L_BRACKET).is_some();
    let operator = method_name(node).is_some_and(|n| n.kind() == k::OPERATOR);
    if node.token(k::L_PAREN).is_some() || !spaced || setter || operator {
        return;
    }
    let mut inner = group.children().filter(|c| c.kind() == k::STATEMENTS);
    let (Some(statements), None) = (inner.next(), inner.next()) else {
        return;
    };
    let mut stmts = statements.children();
    let (Some(only), None) = (stmts.next(), stmts.next()) else {
        return;
    };
    let blocked = matches!(
        only.kind(),
        k::IF
            | k::UNLESS
            | k::AND
            | k::OR
            | k::HASH
            | k::RANGE
            | k::MULTI_WRITE
            | k::WHILE
            | k::UNTIL
    ) || command_call(&only);
    if !blocked {
        let range = span(ctx, &group);
        check.hit(ctx, range, text_of(ctx, &group));
    }
}

/// `a.b c`: a call with arguments and no parentheses.
fn command_call(n: &SyntaxNode) -> bool {
    n.kind() == k::CALL
        && n.child(k::ARGUMENTS).is_some()
        && !n.token(k::L_PAREN).is_some()
        && !n.token(k::L_BRACKET).is_some()
        && method_name(n).is_some_and(|m| m.kind() != k::OPERATOR)
}

/// `hash[x.object_id]`, `hash.key?(x.object_id)`: `compare_by_identity` does that.
pub fn hash_compare_by_identity(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let method = node.token(k::DOT).is_some()
        && method_name(node).is_some_and(|n| matches!(n.text(), "key?" | "has_key?" | "fetch"));
    if !(method || node.token(k::L_BRACKET).is_some()) {
        return;
    }
    let by_id = first_argument(node).is_some_and(|a| {
        a.kind() == k::CALL
            && a.token(k::DOT).is_some()
            && a.child(k::ARGUMENTS).is_none()
            && method_name(&a).is_some_and(|n| n.text() == "object_id")
    });
    if by_id {
        let range = span(ctx, node);
        check.hit(ctx, range, "");
    }
}

/// `0.times { }`, `1.times { }`: the block runs never or once.
pub fn useless_times(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let literal = node.children().next().filter(|r| r.kind() == k::INTEGER);
    let small = literal.is_some_and(|r| {
        let text = text_of(ctx, &r).replace('_', "");
        text.parse::<i64>().is_ok_and(|v| v <= 1)
    });
    let block = node.child(k::BLOCK).is_some() || node.child(k::BLOCK_ARGUMENT).is_some();
    if small
        && block
        && node.token(k::DOT).is_some()
        && method_name(node).is_some_and(|n| n.text() == "times")
    {
        let range = span(ctx, node);
        check.hit(ctx, range, "");
    }
}

/// The methods every `Struct` has: a member of that name replaces one.
const STRUCT_METHODS: &str = "all? any? chain chunk chunk_while class clone collect collect_concat compact count cycle deconstruct deconstruct_keys define_singleton_method detect display dig drop drop_while dup each each_cons each_entry each_pair each_slice each_with_index each_with_object entries enum_for eql? equal? extend filter filter_map find find_all find_index first flat_map freeze frozen? grep grep_v group_by hash include? inject inspect instance_eval instance_exec instance_of? instance_variable_defined? instance_variable_get instance_variable_set instance_variables is_a? itself kind_of? lazy length map max max_by member? members method methods min min_by minmax minmax_by nil? none? object_id one? partition private_methods protected_methods public_method public_methods public_send reduce reject remove_instance_variable respond_to? reverse_each select send singleton_class singleton_method singleton_methods size slice_after slice_before slice_when sort sort_by sum take take_while tally tap then to_a to_enum to_h to_s to_set uniq values values_at yield_self zip";

/// `Struct.new(:hash)`: a member that overrides a method of `Struct`.
pub fn struct_new_override(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(object) = node
        .children()
        .next()
        .filter(|o| is_constant(ctx, o, "Struct"))
    else {
        return;
    };
    let _ = object;
    if !method_name(node).is_some_and(|n| n.text() == "new") {
        return;
    }
    let Some(args) = node.child(k::ARGUMENTS) else {
        return;
    };
    for symbol in args.children().filter(|a| a.kind() == k::SYMBOL) {
        let range = span(ctx, &symbol);
        let name = ctx.file.text[range].trim_start_matches(':');
        if STRUCT_METHODS.split(' ').any(|m| m == name) {
            check.hit(ctx, range, name);
        }
    }
}

/// `"#{x.to_s}"`, `puts x.to_s`: the conversion happens anyway.
pub fn redundant_string_coercion(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let to_s = |c: &SyntaxNode| {
        let plain = c.kind() == k::CALL
            && c.child(k::ARGUMENTS).is_none()
            && !c.token(k::L_PAREN).is_some();
        method_name(c).filter(|n| plain && n.text() == "to_s")
    };
    let hits: Vec<_> = if node.kind() == k::EMBEDDED_STATEMENTS {
        node.child(k::STATEMENTS)
            .and_then(|s| s.children().last())
            .and_then(|c| to_s(&c))
            .into_iter()
            .collect()
    } else {
        if node.token(k::DOT).is_some()
            || !method_name(node).is_some_and(|n| matches!(n.text(), "print" | "puts" | "warn"))
        {
            return;
        }
        node.child(k::ARGUMENTS)
            .into_iter()
            .flat_map(|a| a.children().collect::<Vec<_>>())
            .filter_map(|c| to_s(&c))
            .collect()
    };
    for name in hits {
        check.hit(ctx, name.text_range(), "");
    }
}
