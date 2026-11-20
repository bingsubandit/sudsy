//! The `simplify` family: statements and expressions that say in several words what fewer say,
//! and jumps with nothing to jump out of, read from the grammar's roles (`#conditional` with its
//! `condition`, `then` and `else` fields, `#branch` for the `elif` clauses of a chain, `#return`,
//! `#break`, `#continue`, `#loop`, `#function`, `#unary`, `#binary`, `#literal`). Params:
//! - `check`: `collapsible-if` (an `if` whose only statement is an `if`, neither with a further
//!   branch), `needless-bool` (`return` of a boolean in each branch), `if-with-same-arms` (two
//!   neighbouring branches with the same body), `boolean-conditional` (a ternary with the boolean
//!   literals `then` and `else`; the params are `true` or `false`), `twisted-conditional`
//!   (`b if not a else a`), `negated-comparison` (`not a == b`; param `operator` is the operator
//!   whose negation is reported, `except-in` names the functions where it is left alone and
//!   `except-guard: true` leaves a test whose `if` only throws), `double-negation`,
//!   `redundant-cast` (`!!x` or a call of one of the names in `calls` where a boolean is expected
//!   anyway), `jump-outside` (param `jump`: `break`, `continue`, `return`, `yield` or `await`, or
//!   several words of them; `inside` lists the tags a `break` may be in besides `loop`; a `yield`
//!   is also outside in a comprehension), `identical-operands` (`a == a`: a `#binary`, or a
//!   `#call` of an operator method on an `object` with one argument; `operators` lists the
//!   operators checked), `repeat-once` (a call of `method` on an integer of at most `max` with a
//!   block, in the fields named by `with`: Ruby `0.times { }`), `constant-first` (a comparison
//!   whose left operand is a constant and whose right is not; `calls` name the functions whose
//!   call is a constant, `owners` and `members` the `owner.member` constants), `assert-false`
//!   (an `#assert` of `false`), `nested-with` (a `#with` whose only statement is a `#with` of
//!   the same kind), `lonely-if` (an `if` that is the only statement of an `else`),
//!   `superfluous-else` (a final `else` after branches that all end in the `jump` statement
//!   `return`, `throw`, `break` or `continue`; an `if` with an `else` that does counts).
//! - the message has `{name}` where the check passes a name.
//!
//! concept: simplify
//! what: Statements and expressions that say in several words what fewer say, and jumps with nothing to jump out of.
//! tools: ruff SIM, PLR, RET and B rules, ESLint no-lonely-if no-else-return no-unneeded-ternary no-extra-boolean-cast, RuboCop Lint simplifications
//! rules: collapsible-if, needless-bool, if-with-same-arms, boolean-conditional, twisted-conditional, negated-comparison, double-negation, redundant-cast, jump-outside, identical-operands, repeat-once, constant-first, assert-false, nested-with, lonely-if, superfluous-else
//! reads: family:simplify

use crate::tree::significant_range;
use crate::{Edit, Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::roles::{code_text, code_tokens, outer, un};
use core_syntax::{Grammar, SyntaxNode, TextRange};

pub const FAMILY: Family = Family {
    name: "simplify",
    build,
};

#[derive(Clone)]
enum Check {
    CollapsibleIf,
    NeedlessBool,
    SameArms,
    BooleanConditional(Option<bool>, Option<bool>),
    TwistedConditional,
    NegatedComparison {
        operator: String,
        except_in: Vec<String>,
        guard: bool,
    },
    DoubleNegation,
    RedundantCast(Vec<String>),
    Jump {
        jump: Vec<&'static str>,
        inside: Vec<String>,
    },
    IdenticalOperands(Vec<String>),
    RepeatOnce {
        method: String,
        max: i64,
        with: Vec<String>,
    },
    ConstantFirst(Constants),
    AssertFalse,
    NestedWith,
    LonelyIf,
    SuperfluousElse(&'static str),
}

/// What makes an expression a constant besides a literal.
#[derive(Clone)]
struct Constants {
    calls: Vec<String>,
    owners: Vec<String>,
    members: Vec<String>,
}

/// The tags of the jumps `jump-outside` knows.
const JUMPS: [&str; 5] = ["break", "continue", "return", "yield", "await"];

struct Simplify {
    meta: &'static RuleMeta,
    message: String,
    check: Check,
    tags: Vec<&'static str>,
}

fn words(p: &mut Params, key: &str) -> Vec<String> {
    p.optional(key)
        .map(|s| s.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let name = p.text("check")?;
    let flag = |p: &mut Params, k: &str| {
        p.choice(
            k,
            true,
            &[("true", Some(true)), ("false", Some(false)), ("any", None)],
        )
    };
    let (check, tags): (Check, Vec<&'static str>) = match name.as_str() {
        "collapsible-if" => (Check::CollapsibleIf, vec!["conditional"]),
        "needless-bool" => (Check::NeedlessBool, vec!["conditional"]),
        "if-with-same-arms" => (Check::SameArms, vec!["conditional"]),
        "boolean-conditional" => {
            let (a, b) = (flag(&mut p, "then")?, flag(&mut p, "else")?);
            (Check::BooleanConditional(a, b), vec!["conditional"])
        }
        "twisted-conditional" => (Check::TwistedConditional, vec!["conditional"]),
        "negated-comparison" => (
            Check::NegatedComparison {
                operator: p.text("operator")?,
                except_in: words(&mut p, "except-in"),
                guard: p.flag("except-guard")?,
            },
            vec!["unary"],
        ),
        "double-negation" => (Check::DoubleNegation, vec!["unary"]),
        "redundant-cast" => (
            Check::RedundantCast(words(&mut p, "calls")),
            vec!["unary", "call"],
        ),
        "jump-outside" => {
            let mut jump = Vec::new();
            for w in words(&mut p, "jump") {
                match JUMPS.iter().find(|j| **j == w) {
                    Some(j) => jump.push(*j),
                    None => return Err(format!("param `jump`: unknown jump `{w}`")),
                }
            }
            (
                Check::Jump {
                    jump: jump.clone(),
                    inside: words(&mut p, "inside"),
                },
                jump,
            )
        }
        "identical-operands" => (
            Check::IdenticalOperands(words(&mut p, "operators")),
            vec!["binary", "call"],
        ),
        "repeat-once" => (
            Check::RepeatOnce {
                method: p.text("method")?,
                max: p
                    .text("max")?
                    .parse()
                    .map_err(|_| "param `max`: not an integer".to_string())?,
                with: words(&mut p, "with"),
            },
            vec!["call"],
        ),
        "constant-first" => (
            Check::ConstantFirst(Constants {
                calls: words(&mut p, "calls"),
                owners: words(&mut p, "owners"),
                members: words(&mut p, "members"),
            }),
            vec!["binary"],
        ),
        "assert-false" => (Check::AssertFalse, vec!["assert"]),
        "nested-with" => (Check::NestedWith, vec!["with"]),
        "lonely-if" => (Check::LonelyIf, vec!["conditional"]),
        "superfluous-else" => {
            let jump = p.text("jump")?;
            let Some(jump) = ["return", "throw", "break", "continue"]
                .into_iter()
                .find(|j| *j == jump)
            else {
                return Err(format!("param `jump`: unknown jump `{jump}`"));
            };
            (Check::SuperfluousElse(jump), vec!["conditional"])
        }
        other => return Err(format!("param `check`: unknown check `{other}`")),
    };
    let rule = Simplify {
        meta,
        message: p.text("message")?,
        check,
        tags,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Simplify {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
    }

    fn tags(&self) -> &[&'static str] {
        &self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let range = || significant_range(node, g);
        let hit = match &self.check {
            Check::CollapsibleIf => collapsible_if(g, node).then(String::new),
            Check::NeedlessBool => needless_bool(g, node).then(String::new),
            Check::SameArms => {
                for later in same_arms(g, node) {
                    ctx.report(
                        significant_range(&later, g),
                        self.message.replace("{name}", ""),
                    );
                }
                return;
            }
            Check::BooleanConditional(a, b) => {
                boolean_conditional(g, node, *a, *b).then(String::new)
            }
            Check::TwistedConditional => twisted(g, node).then(String::new),
            Check::NegatedComparison {
                operator,
                except_in,
                guard,
            } => {
                let Some((raw, inner)) = negated_comparison(g, node, operator, except_in, *guard)
                else {
                    return;
                };
                let edits = negation_edits(ctx.file.text, node, &raw, Some((&inner, operator)));
                self.report(ctx, range(), "", edits);
                return;
            }
            Check::DoubleNegation => {
                let Some(raw) = negation(g, node).and_then(|o| negation(g, &un(&o))) else {
                    return;
                };
                let edits = negation_edits(ctx.file.text, node, &raw, None);
                self.report(ctx, range(), "", edits);
                return;
            }
            Check::RedundantCast(calls) => redundant_cast(g, node, calls),
            Check::Jump { jump, inside } => jump_outside(g, node, jump, inside).then(String::new),
            Check::IdenticalOperands(operators) => identical_operands(g, node, ctx, operators),
            Check::RepeatOnce { method, max, with } => {
                repeat_once(g, node, method, *max, with).then(String::new)
            }
            Check::ConstantFirst(constants) => {
                self.constant_first(g, node, ctx, constants);
                return;
            }
            Check::AssertFalse => assert_false(g, node).then(String::new),
            Check::NestedWith => nested_with(g, node).then(String::new),
            Check::LonelyIf => {
                if let Some(inner) = lonely_if(g, node) {
                    self.report(ctx, significant_range(&inner, g), "", None);
                }
                return;
            }
            Check::SuperfluousElse(jump) => {
                if let Some(e) = superfluous_else(g, node, jump) {
                    self.report(ctx, significant_range(&e, g), "", None);
                }
                return;
            }
        };
        if let Some(name) = hit {
            self.report(ctx, range(), &name, None);
        }
    }
}

impl Simplify {
    fn report(
        &self,
        ctx: &mut RuleContext<'_>,
        range: TextRange,
        name: &str,
        edits: Option<Vec<Edit>>,
    ) {
        let message = self.message.replace("{name}", name);
        ctx.report_fix(
            range,
            message,
            edits.map(|e| ("Simplify the expression", e)),
        );
    }

    /// `constant-first`: the report is on the left operand; the fix swaps the operands and the
    /// operator's direction, unless a comment is inside.
    fn constant_first(
        &self,
        g: &Grammar,
        n: &SyntaxNode,
        ctx: &mut RuleContext<'_>,
        c: &Constants,
    ) {
        let Some(op) = g.slot(n, "operator").and_then(|o| o.into_token()) else {
            return;
        };
        if !matches!(g.operator_kind(op.text()), Some("equality" | "comparison")) {
            return;
        }
        let mut operands = n.children();
        let (Some(left), Some(right), None) = (operands.next(), operands.next(), operands.next())
        else {
            return;
        };
        if !is_constant(g, &left, c) || is_constant(g, &right, c) {
            return;
        }
        let whole = significant_range(n, g);
        let has_comment = n
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| whole.contains_range(t.text_range()) && g.has_tag(t.kind(), "comment"));
        let flip = match op.text() {
            "<" => ">",
            "<=" => ">=",
            ">" => "<",
            ">=" => "<=",
            other @ ("==" | "!=" | "===" | "!==") => other,
            _ => "",
        };
        let fix = (!has_comment && !flip.is_empty()).then(|| {
            let text = |x: &SyntaxNode| ctx.file.text[significant_range(x, g)].to_owned();
            let swapped = format!("{} {flip} {}", text(&right), text(&left));
            (
                "Put the constant on the right",
                vec![Edit::replace(whole, swapped)],
            )
        });
        ctx.report_fix(significant_range(&left, g), self.message.clone(), fix);
    }
}

/// The operator of `a == a`: both operands are the same code. A `#binary` has them in its
/// fields; so has a call of an operator method (`operators` lists them) on an `object` with one
/// argument (Ruby `a == b` is the call `a.==(b)`), found by its own operator token first, as
/// most calls are not one.
fn identical_operands(
    g: &Grammar,
    n: &SyntaxNode,
    ctx: &RuleContext<'_>,
    operators: &[String],
) -> Option<String> {
    let (left, op, right) = if g.has_tag(n.kind(), "binary") {
        let op = g.slot(n, "operator")?.into_token()?;
        (node(g, n, "left")?, op, node(g, n, "right")?)
    } else {
        // the method name is the first token, or the second after a `.`
        let op = code_tokens(n)
            .take(2)
            .find(|t| g.has_tag(t.kind(), "operator"))?;
        if !operators.iter().any(|o| o == op.text()) {
            return None;
        }
        let mut args = node(g, n, "arguments")?.children();
        let (Some(right), None) = (args.next(), args.next()) else {
            return None;
        };
        (node(g, n, "object")?, op, right)
    };
    let same = |a: &SyntaxNode, b: &SyntaxNode| {
        ctx.file.text[significant_range(a, g)] == ctx.file.text[significant_range(b, g)]
    };
    (operators.iter().any(|o| o == op.text()) && same(&left, &right)).then(|| op.text().to_string())
}

/// `0.times { }`: a call of `method` on an integer of at most `max` that has a block.
fn repeat_once(g: &Grammar, n: &SyntaxNode, method: &str, max: i64, with: &[String]) -> bool {
    g.slot(n, "member")
        .and_then(|m| m.into_token())
        .is_some_and(|m| m.text() == method)
        && node(g, n, "object").is_some_and(|o| {
            g.has_tag(o.kind(), "literal")
                && code_text(&o)
                    .replace('_', "")
                    .parse::<i64>()
                    .is_ok_and(|v| v <= max)
        })
        && with.iter().any(|f| g.slot(n, f).is_some())
}

/// Is the value known without names: a literal, a call of a function of `calls`, a member of an
/// owner of `owners` (`int.MaxValue`) or an arithmetic, bitwise or logical expression of those?
fn is_constant(g: &Grammar, n: &SyntaxNode, c: &Constants) -> bool {
    let n = un(n);
    let k = n.kind();
    let all = |n: &SyntaxNode| n.children().all(|x| is_constant(g, &x, c));
    if g.has_tag(k, "literal") {
        return true;
    }
    if g.has_tag(k, "parenthesized") {
        return all(&n);
    }
    let op = g.slot(&n, "operator").and_then(|o| o.into_token());
    let kind = op.as_ref().and_then(|o| g.operator_kind(o.text()));
    if g.has_tag(k, "unary") {
        return op.is_some_and(|o| !matches!(o.text(), "++" | "--"))
            && matches!(kind, Some("arithmetic" | "logical" | "bitwise"))
            && all(&n);
    }
    if g.has_tag(k, "binary") {
        return matches!(kind, Some("arithmetic" | "bitwise")) && all(&n);
    }
    if g.has_tag(k, "call") {
        return node(g, &n, "callee").is_some_and(|f| c.calls.contains(&code_text(&f)));
    }
    if g.has_tag(k, "member-access") {
        let text = |e: core_syntax::SyntaxElement| match e {
            core_syntax::SyntaxElement::Node(m) => code_text(&m),
            core_syntax::SyntaxElement::Token(t) => t.text().to_string(),
        };
        return node(g, &n, "object").is_some_and(|o| c.owners.contains(&code_text(&o)))
            && g.slot(&n, "member")
                .is_some_and(|m| c.members.contains(&text(m)));
    }
    false
}

fn assert_false(g: &Grammar, n: &SyntaxNode) -> bool {
    node(g, n, "test").is_some_and(|t| boolean(g, &t) == Some(false))
}

/// A `with` whose only statement is another `with` of the same kind (both `async`, or neither).
fn nested_with(g: &Grammar, n: &SyntaxNode) -> bool {
    let own: Vec<SyntaxNode> = n
        .children()
        .filter(|c| g.has_tag(c.kind(), "statement"))
        .collect();
    let [inner] = &unblock(g, own)[..] else {
        return false;
    };
    let lead = |n: &SyntaxNode| code_tokens(n).next().map(|t| t.text().to_string());
    g.has_tag(inner.kind(), "with") && lead(n) == lead(inner)
}

/// The `if` that is the only statement of the last `else` of `n`.
fn lonely_if(g: &Grammar, n: &SyntaxNode) -> Option<SyntaxNode> {
    if !is_statement_conditional(g, n) || !has_condition(g, n) || is_elif(g, n) {
        return None;
    }
    let (_, tested, stmts) = following(g, n).pop()?;
    match &stmts[..] {
        [s] if !tested && is_statement_conditional(g, s) && has_condition(g, s) => Some(s.clone()),
        _ => None,
    }
}

/// Does a statement of `b` always end in a statement tagged `jump`: it is one, or an `if` with
/// an `else` all of whose branches do?
fn always_jumps(g: &Grammar, b: &[SyntaxNode], jump: &str) -> bool {
    b.iter()
        .any(|s| g.has_tag(s.kind(), jump) || exhaustive_if(g, s, jump))
}

fn exhaustive_if(g: &Grammar, s: &SyntaxNode, jump: &str) -> bool {
    if !is_statement_conditional(g, s) || !has_condition(g, s) {
        return false;
    }
    let rest = following(g, s);
    matches!(rest.last(), Some((_, false, _)))
        && always_jumps(g, &body(g, s), jump)
        && rest.iter().all(|(_, _, b)| always_jumps(g, b, jump))
}

/// The last `else` of `n` when every branch before it always ends in a statement tagged `jump`.
fn superfluous_else(g: &Grammar, n: &SyntaxNode, jump: &str) -> Option<SyntaxNode> {
    if !is_statement_conditional(g, n) || !has_condition(g, n) || is_elif(g, n) {
        return None;
    }
    let rest = following(g, n);
    let ((last, tested, _), before) = rest.split_last()?;
    (!tested
        && always_jumps(g, &body(g, n), jump)
        && before.iter().all(|(_, _, b)| always_jumps(g, b, jump)))
    .then(|| last.clone())
}

fn nodes(g: &Grammar, n: &SyntaxNode, label: &str) -> Vec<SyntaxNode> {
    g.slots(n, label).filter_map(|e| e.into_node()).collect()
}

fn node(g: &Grammar, n: &SyntaxNode, label: &str) -> Option<SyntaxNode> {
    g.slot(n, label).and_then(|e| e.into_node())
}

/// The statements of a body: those of a single block, or the statements as they are.
fn unblock(g: &Grammar, mut stmts: Vec<SyntaxNode>) -> Vec<SyntaxNode> {
    if let [only] = &stmts[..]
        && (g.has_tag(only.kind(), "block") || g.has_tag(only.kind(), "statements"))
    {
        stmts = nodes(g, only, "items");
    }
    stmts
}

fn body(g: &Grammar, n: &SyntaxNode) -> Vec<SyntaxNode> {
    unblock(g, nodes(g, n, "then"))
}

fn is_statement_conditional(g: &Grammar, n: &SyntaxNode) -> bool {
    g.has_tag(n.kind(), "conditional") && !g.has_tag(n.kind(), "expression")
}

fn has_condition(g: &Grammar, n: &SyntaxNode) -> bool {
    g.slot(n, "condition").is_some()
}

/// Is the `if` an `elif`: a clause of a chain, or the `if` in the `else` of another?
fn is_elif(g: &Grammar, n: &SyntaxNode) -> bool {
    g.has_tag(n.kind(), "branch")
        || n.parent().is_some_and(|p| {
            is_statement_conditional(g, &p) && node(g, &p, "else").as_ref() == Some(n)
        })
}

/// A branch of the chain after the first: its node, whether it has a test, its statements.
type Branch = (SyntaxNode, bool, Vec<SyntaxNode>);

/// The branches after `n`: its later clauses, or what its `else` holds (an `if` goes on).
fn following(g: &Grammar, n: &SyntaxNode) -> Vec<Branch> {
    let mut out = Vec::new();
    if g.has_tag(n.kind(), "branch") {
        let mut next = n.next_sibling();
        while let Some(s) = next.filter(|s| g.has_tag(s.kind(), "branch")) {
            out.push((s.clone(), has_condition(g, &s), body(g, &s)));
            next = s.next_sibling();
        }
        return out;
    }
    for e in nodes(g, n, "else") {
        if g.has_tag(e.kind(), "branch") {
            out.push((e.clone(), has_condition(g, &e), body(g, &e)));
        } else if is_statement_conditional(g, &e) {
            out.push((e.clone(), true, body(g, &e)));
            out.extend(following(g, &e));
        } else {
            out.push((e.clone(), false, unblock(g, vec![e])));
        }
    }
    out
}

fn has_more(g: &Grammar, n: &SyntaxNode) -> bool {
    !following(g, n).is_empty()
}

fn collapsible_if(g: &Grammar, n: &SyntaxNode) -> bool {
    if !is_statement_conditional(g, n) || !has_condition(g, n) || has_more(g, n) {
        return false;
    }
    matches!(&body(g, n)[..], [s] if is_statement_conditional(g, s)
        && has_condition(g, s) && !has_more(g, s))
}

fn boolean(g: &Grammar, e: &SyntaxNode) -> Option<bool> {
    let e = un(e);
    if !g.has_tag(e.kind(), "literal") {
        return None;
    }
    match code_text(&e).as_str() {
        "true" | "True" => Some(true),
        "false" | "False" => Some(false),
        _ => None,
    }
}

fn bool_return(g: &Grammar, s: &SyntaxNode) -> Option<bool> {
    if !g.has_tag(s.kind(), "return") {
        return None;
    }
    boolean(g, &node(g, s, "value")?)
}

fn only_bool_return(g: &Grammar, body: &[SyntaxNode]) -> Option<bool> {
    match body {
        [s] => bool_return(g, s),
        _ => None,
    }
}

fn needless_bool(g: &Grammar, n: &SyntaxNode) -> bool {
    if !is_statement_conditional(g, n) || !has_condition(g, n) {
        return false;
    }
    let Some(a) = only_bool_return(g, &body(g, n)) else {
        return false;
    };
    let rest = following(g, n);
    let other = match &rest[..] {
        [] if !is_elif(g, n) => n.next_sibling().and_then(|s| bool_return(g, &s)),
        [(_, false, b)] => only_bool_return(g, b),
        _ => None,
    };
    other.is_some_and(|b| b != a)
}

fn key(b: &[SyntaxNode]) -> String {
    b.iter().map(code_text).collect::<Vec<_>>().join("\n")
}

/// The branches (not the first) that repeat the body of the one before them.
fn same_arms(g: &Grammar, n: &SyntaxNode) -> Vec<SyntaxNode> {
    if !is_statement_conditional(g, n) || is_elif(g, n) {
        return Vec::new();
    }
    let mut previous = key(&body(g, n));
    let mut out = Vec::new();
    for (node, tested, stmts) in following(g, n) {
        if !tested {
            break;
        }
        let k = key(&stmts);
        if k == previous && !k.is_empty() {
            out.push(node);
        }
        previous = k;
    }
    out
}

/// Is the expression a boolean literal, `want` when given?
fn is_boolean(g: &Grammar, e: &SyntaxNode, want: Option<bool>) -> bool {
    boolean(g, e).is_some_and(|b| want.is_none_or(|w| w == b))
}

fn boolean_conditional(
    g: &Grammar,
    n: &SyntaxNode,
    then: Option<bool>,
    other: Option<bool>,
) -> bool {
    g.has_tag(n.kind(), "expression")
        && node(g, n, "then").is_some_and(|t| is_boolean(g, &t, then))
        && node(g, n, "else").is_some_and(|e| is_boolean(g, &e, other))
}

/// The operand of a logical negation (`not x`, `!x`).
fn negation(g: &Grammar, n: &SyntaxNode) -> Option<SyntaxNode> {
    let n = un(n);
    if !g.has_tag(n.kind(), "unary") {
        return None;
    }
    let op = g.slot(&n, "operator")?;
    matches!(op.as_token()?.text(), "not" | "!").then_some(())?;
    node(g, &n, "operand")
}

fn twisted(g: &Grammar, n: &SyntaxNode) -> bool {
    let (Some(t), Some(c), Some(e)) = (
        node(g, n, "then"),
        node(g, n, "condition"),
        node(g, n, "else"),
    ) else {
        return false;
    };
    let (e, tag) = (un(&e), "reference");
    g.has_tag(n.kind(), "expression")
        && boolean(g, &t).is_none()
        && boolean(g, &e).is_none()
        && negation(g, &c).map(|o| un(&o)).is_some_and(|o| {
            g.has_tag(o.kind(), tag) && g.has_tag(e.kind(), tag) && code_text(&o) == code_text(&e)
        })
}

fn function_name(g: &Grammar, n: &SyntaxNode) -> String {
    n.ancestors()
        .skip(1)
        .find(|a| g.has_tag(a.kind(), "function"))
        .and_then(|f| node(g, &f, "name"))
        .map(|name| code_text(&name))
        .unwrap_or_default()
}

/// Is the expression in the test of an `if` whose only statement throws (an argument check)?
fn guard(g: &Grammar, n: &SyntaxNode) -> bool {
    let mut child = n.clone();
    for a in n.ancestors().skip(1) {
        if is_statement_conditional(g, &a)
            && node(g, &a, "condition").as_ref() == Some(&outer(&child))
        {
            return matches!(&body(g, &a)[..], [s] if g.has_tag(s.kind(), "throw"));
        }
        if g.has_tag(a.kind(), "statement") {
            return false;
        }
        child = a;
    }
    false
}

/// The comparison `not` negates, when it is a single `operator` comparison.
fn negated_comparison(
    g: &Grammar,
    n: &SyntaxNode,
    operator: &str,
    except_in: &[String],
    except_guard: bool,
) -> Option<(SyntaxNode, SyntaxNode)> {
    let raw = negation(g, n)?;
    let operand = un(&raw);
    if !g.has_tag(operand.kind(), "binary") {
        return None;
    }
    // a chain (`a == b == c`) has an operator after its right operand too
    let mut ops = operand
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| g.has_tag(t.kind(), "operator"));
    if ops.next().is_none_or(|t| t.text() != operator) || ops.next().is_some() {
        return None;
    }
    if (!except_in.is_empty() && except_in.contains(&function_name(g, n)))
        || (except_guard && guard(g, n))
    {
        return None;
    }
    Some((raw, operand))
}

/// The edits of a fix that drops the negation around `raw` (the operand as written) and, with
/// `swap` (the comparison and its operator), turns the operator into its opposite; none when
/// something but negations and balanced brackets is around the operand.
fn negation_edits(
    text: &str,
    n: &SyntaxNode,
    raw: &SyntaxNode,
    swap: Option<(&SyntaxNode, &str)>,
) -> Option<Vec<Edit>> {
    let (whole, inside) = (n.text_range(), raw.text_range());
    let head = TextRange::new(whole.start(), inside.start());
    let tail = TextRange::new(inside.end(), whole.end());
    let only = |r: TextRange, extra: char| {
        text[r]
            .chars()
            .all(|c| c.is_whitespace() || c == extra || c == '!' || c.is_alphabetic())
    };
    let count = |r: TextRange, c: char| text[r].matches(c).count();
    if !only(head, '(') || !only(tail, ')') || count(head, '(') != count(tail, ')') {
        return None;
    }
    let mut edits = vec![Edit::delete(head), Edit::delete(tail)];
    if let Some((comparison, op)) = swap {
        let token = comparison
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.text() == op)?;
        edits.push(Edit::replace(
            token.text_range(),
            if op == "==" { "!=" } else { "==" },
        ));
    }
    Some(edits)
}

fn in_boolean_context(g: &Grammar, n: &SyntaxNode, calls: &[String]) -> bool {
    let outer = outer(n);
    let Some(parent) = outer.parent() else {
        return false;
    };
    let kind = parent.kind();
    if g.has_tag(kind, "conditional") || g.has_tag(kind, "loop") {
        return node(g, &parent, "condition").as_ref() == Some(&outer);
    }
    if g.has_tag(kind, "unary") {
        return g
            .slot(&parent, "operator")
            .and_then(|o| o.as_token().map(|t| t.text() == "!"))
            == Some(true);
    }
    g.has_tag(kind, "call")
        && is_named(g, &parent, calls)
        && node(g, &parent, "callee").as_ref() != Some(&outer)
}

fn is_named(g: &Grammar, call: &SyntaxNode, names: &[String]) -> bool {
    node(g, call, "callee")
        .is_some_and(|c| u32::from(c.text_range().len()) <= 24 && names.contains(&code_text(&c)))
}

fn redundant_cast(g: &Grammar, n: &SyntaxNode, calls: &[String]) -> Option<String> {
    let found = if g.has_tag(n.kind(), "unary") {
        negation(g, n)
            .and_then(|o| negation(g, &un(&o)))
            .map(|_| "double negation".to_string())
    } else {
        is_named(g, n, calls).then(|| {
            format!(
                "{} call",
                node(g, n, "callee")
                    .map(|c| code_text(&c))
                    .unwrap_or_default()
            )
        })
    }?;
    in_boolean_context(g, n, calls).then_some(found)
}

/// Is a `yield` or `await` outside a function: its nearest scope is a type, a comprehension
/// (not for `await`) or the file?
fn suspend_outside(g: &Grammar, n: &SyntaxNode) -> bool {
    let is_await = g.has_tag(n.kind(), "await");
    for a in n.ancestors().skip(1) {
        let k = a.kind();
        if g.has_tag(k, "function") {
            return false;
        }
        if g.has_tag(k, "type-declaration")
            || (!is_await && g.has_tag(k, "scope") && g.has_tag(k, "display"))
        {
            return true;
        }
    }
    true
}

fn jump_outside(g: &Grammar, n: &SyntaxNode, jump: &[&str], inside: &[String]) -> bool {
    if jump.iter().any(|j| matches!(*j, "yield" | "await")) {
        return suspend_outside(g, n);
    }
    let jump = jump.first().copied().unwrap_or_default();
    if g.slot(n, "label").is_some() {
        return false;
    }
    let mut child = n.clone();
    for a in n.ancestors().skip(1) {
        let k = a.kind();
        if jump != "return" {
            let allowed = g.has_tag(k, "loop") && nodes(g, &a, "body").contains(&child)
                || (jump == "break" && inside.iter().any(|t| g.has_tag(k, t)));
            if allowed {
                return false;
            }
        }
        if g.has_tag(k, "function") {
            return jump != "return";
        }
        if g.has_tag(k, "type-declaration") {
            return true;
        }
        child = a;
    }
    true
}
