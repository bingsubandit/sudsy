//! Format strings that do not fit their arguments, and the ones an interpolated string would
//! say better. The grammar's `format` rows say which strings are format strings (`percent`,
//! `brace`: [`flavor`]) and where their arguments are (`operator`: the right operand, `method`:
//! the arguments of the call); the tags of [`shape`] say what an argument is. A format string is
//! read as the language's library reads it; which right-hand sides are one value, a sequence or a
//! mapping follows ruff. One analysis of a node serves every rule: each reports its own findings.

mod flavor;
mod shape;

use crate::rule::{Level, RuleContext, RuleMeta};
use crate::tree::significant_range;
use crate::{Category, Rule};
use core_syntax::roles::code_text;
use core_syntax::{FormatRow, FormatVia, Grammar, SyntaxNode};
use flavor::{Failure, Flavor, Spec};
use shape::{Reader, Rhs};
use std::cell::RefCell;
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    PercentInvalid,
    PercentCharacter,
    PercentCount,
    PercentMixed,
    PercentMapping,
    PercentExtraNames,
    PercentMissingName,
    PercentSequence,
    PercentStar,
    PrintfToInterpolation,
    BraceInvalid,
    BraceMixing,
    BraceExtraPositional,
    BraceExtraNames,
    BraceMissing,
    BraceToInterpolation,
}

type Finding = (Kind, String);

/// Does the grammar describe format strings?
pub fn applies(g: &Grammar) -> bool {
    !g.formats.is_empty()
}

/// One rule per finding: the id, the metadata and the message (`{name}` is what the finding
/// names).
pub struct FormatRule {
    meta: &'static RuleMeta,
    kind: Kind,
    message: &'static str,
}

macro_rules! rules {
    ($($kind:ident $id:literal $category:ident $level:ident $message:literal
        $description:literal $example:literal;)*) => {
        pub static RULES: &[FormatRule] = &[$(FormatRule {
            meta: &RuleMeta {
                id: $id,
                category: Category::$category,
                description: $description,
                example: $example,
                default_level: Level::$level,
                fix: None,
                requires: &[],
                uses: &[],
                options: &[],
            },
            kind: Kind::$kind,
            message: $message,
        }),*];
    };
}

rules! {
    PercentInvalid "percent-format-invalid-format" Correctness Warn
        "`%`-format string is invalid: {name}"
        "A `\"...\" % x` whose format string does not parse." "\"%\" % x";
    PercentCharacter "percent-format-unsupported-format-character" Correctness Warn
        "`%`-format string has an unsupported format character `{name}`"
        "A `%` format with a conversion character the language does not know." "\"%z\" % x";
    PercentMixed "percent-format-mixed-positional-and-named" Correctness Warn
        "`%`-format string mixes positional and named placeholders"
        "A `%` format with both `%s` and `%(name)s`." "\"%s %(a)s\" % x";
    PercentMapping "percent-format-expected-mapping" Correctness Warn
        "`%`-format string has named placeholders but the right side is a sequence"
        "A `%` format with `%(name)s` placeholders applied to a tuple, list or set."
        "\"%(a)s\" % (1,)";
    PercentSequence "percent-format-expected-sequence" Correctness Warn
        "`%`-format string has several placeholders but the right side is a mapping"
        "A `%` format with several positional placeholders applied to a dict."
        "\"%s %s\" % {\"a\": 1}";
    PercentExtraNames "percent-format-extra-named-arguments" Correctness Warn
        "`%`-format string has unused named argument(s): {name}"
        "A `%` format applied to a dict with keys no placeholder names."
        "\"%(a)s\" % {\"a\": 1, \"b\": 2}";
    PercentMissingName "percent-format-missing-argument" Correctness Warn
        "`%`-format string is missing argument(s) for placeholder(s): {name}"
        "A `%` format with named placeholders the dict does not have."
        "\"%(a)s\" % {\"b\": 1}";
    PercentCount "percent-format-positional-count-mismatch" Correctness Warn
        "`%`-format string has {name}"
        "A `%` format whose placeholders and values differ in number." "\"%s %s\" % (1,)";
    PercentStar "percent-format-star-requires-sequence" Correctness Warn
        "`%`-format string `*` specifier requires a sequence"
        "A `%` format with a `*` width or precision applied to a dict." "\"%*s\" % {\"a\": 1}";
    BraceInvalid "string-dot-format-invalid-format" Correctness Warn
        "`.format` call has an invalid format string: {name}"
        "A `\"...\".format(...)` whose string does not parse." "\"{\".format(1)";
    BraceExtraNames "string-dot-format-extra-named-arguments" Correctness Warn
        "`.format` call has unused named argument(s): {name}"
        "A `.format` call with keywords no field names." "\"{}\".format(1, a=2)";
    BraceExtraPositional "string-dot-format-extra-positional-arguments" Correctness Warn
        "`.format` call has unused arguments at position(s): {name}"
        "A `.format` call with positional arguments no field uses." "\"{}\".format(1, 2)";
    BraceMissing "string-dot-format-missing-arguments" Correctness Warn
        "`.format` call is missing argument(s) for placeholder(s): {name}"
        "A `.format` call without an argument for a field." "\"{} {}\".format(1)";
    BraceMixing "string-dot-format-mixing-automatic" Correctness Warn
        "`.format` string mixes automatic and manual numbering"
        "A `.format` string with both `{}` and `{0}`." "\"{} {0}\".format(1)";
    PrintfToInterpolation "printf-string-formatting" Style Off
        "use format specifiers or an f-string instead of `%` formatting"
        "A `\"...\" % x` whose format string and values pair up (a tuple of as many values, a dict for named fields, or one value for one field)."
        "x = \"%s\" % y";
    BraceToInterpolation "f-string" Style Off
        "use an f-string instead of a `.format` call"
        "A `\"...\".format(...)` on a string literal whose fields all name an argument."
        "x = \"{}\".format(y)";
}

impl Rule for FormatRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn tags(&self) -> &[&'static str] {
        &["binary", "call"]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let names = MEMO.with(|m| m.names(node, self.kind, || findings(&Reader { g }, node)));
        if names.is_empty() {
            return;
        }
        let range = significant_range(node, g);
        for name in names {
            ctx.report(range, self.message.replace("{name}", &name));
        }
    }
}

/// The findings of the node the rules ran on last: each rule asks for them, the first one works
/// them out.
struct Memo(RefCell<Option<(SyntaxNode, Vec<Finding>)>>);

impl Memo {
    /// What the findings of `kind` name.
    fn names(&self, n: &SyntaxNode, kind: Kind, f: impl FnOnce() -> Vec<Finding>) -> Vec<String> {
        let mut slot = self.0.borrow_mut();
        if slot.as_ref().is_none_or(|(node, _)| node != n) {
            *slot = Some((n.clone(), f()));
        }
        let found = slot.iter().flat_map(|(_, found)| found);
        found
            .filter(|(k, _)| *k == kind)
            .map(|(_, name)| name.clone())
            .collect()
    }
}

thread_local! {
    static MEMO: Memo = const { Memo(RefCell::new(None)) };
}

fn join<T: ToString>(items: impl IntoIterator<Item = T>) -> String {
    items
        .into_iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn findings(r: &Reader<'_>, n: &SyntaxNode) -> Vec<Finding> {
    for row in &r.g.formats {
        let Some(flavor) = Flavor::named(&row.flavor) else {
            continue;
        };
        let found = match row.via {
            // The cheap rejects first: the operator's own token, the callee's kind.
            FormatVia::Operator
                if r.tag(n, "binary")
                    && n.children_with_tokens()
                        .filter_map(|e| e.into_token())
                        .any(|t| t.text() == row.name) =>
            {
                operator(r, row, flavor, n)
            }
            FormatVia::Method
                if r.tag(n, "call")
                    && n.children()
                        .next()
                        .is_some_and(|c| r.tag(&c, "member-access")) =>
            {
                method(r, row, flavor, n)
            }
            _ => None,
        };
        if let Some(found) = found {
            return found;
        }
    }
    Vec::new()
}

/// `"..." % right`.
fn operator(
    r: &Reader<'_>,
    row: &FormatRow,
    flavor: Flavor,
    b: &SyntaxNode,
) -> Option<Vec<Finding>> {
    if r.operator(b)? != row.name {
        return None;
    }
    let left = r.field(b, "left").filter(|l| r.is_string(l))?;
    let right = r.field(b, "right")?;
    let spec = match flavor.parse(&r.string_value(&left)) {
        Ok(spec) => spec,
        Err(Failure::Invalid(why)) => return Some(vec![(Kind::PercentInvalid, why.to_owned())]),
        Err(Failure::Char(c)) => return Some(vec![(Kind::PercentCharacter, c.to_string())]),
    };
    let mut out: Vec<Finding> = Vec::new();
    let (named, pos) = (!spec.names.is_empty(), spec.auto);
    let count = |out: &mut Vec<Finding>, got: usize| {
        if pos != got {
            let why = format!("{pos} placeholder(s) but {got} substitution(s)");
            out.push((Kind::PercentCount, why));
        }
    };
    let starred = |e: &SyntaxNode| r.tag(&core_syntax::roles::un(e), "rest");
    if named && pos > 0 {
        out.push((Kind::PercentMixed, String::new()));
    }
    let rhs = r.classify(&right);
    match &rhs {
        Rhs::Tuple(_) | Rhs::Sequence if named => {
            out.push((Kind::PercentMapping, String::new()));
        }
        Rhs::Tuple(t) => {
            if !t.iter().any(starred) {
                count(&mut out, t.len());
            }
        }
        Rhs::Sequence => count(&mut out, 1),
        Rhs::Dict(keys) => {
            if named || pos == 0 {
                if let Some(keys) = keys {
                    let unused: Vec<_> = keys.iter().filter(|k| !spec.names.contains(k)).collect();
                    let mut missing: Vec<_> =
                        spec.names.iter().filter(|k| !keys.contains(k)).collect();
                    missing.sort();
                    if !unused.is_empty() {
                        out.push((Kind::PercentExtraNames, join(unused)));
                    }
                    if !missing.is_empty() {
                        out.push((Kind::PercentMissingName, join(missing)));
                    }
                }
                if !named {
                    count(&mut out, 1);
                }
            } else {
                if pos > 1 {
                    out.push((Kind::PercentSequence, String::new()));
                }
                count(&mut out, 1);
            }
            if spec.star {
                out.push((Kind::PercentStar, String::new()));
            }
        }
        Rhs::Single if !named => count(&mut out, 1),
        Rhs::Unknown if !named && pos == 0 => count(&mut out, 1),
        Rhs::Single | Rhs::Unknown => {}
    }
    if out.is_empty() && interpolates_percent(r, &rhs, &right, &spec) {
        out.push((Kind::PrintfToInterpolation, String::new()));
    }
    Some(out)
}

/// Would an interpolated string say the same: the values pair up with the placeholders.
fn interpolates_percent(r: &Reader<'_>, rhs: &Rhs, right: &SyntaxNode, spec: &Spec) -> bool {
    let (named, pos) = (!spec.names.is_empty(), spec.auto);
    let right = core_syntax::roles::un(right);
    match rhs {
        Rhs::Tuple(t) => !named && !t.iter().any(|e| r.tag(&core_syntax::roles::un(e), "rest")),
        Rhs::Dict(keys) => named && keys.is_some(),
        Rhs::Unknown => ["reference", "member-access", "index", "call"]
            .iter()
            .any(|t| r.tag(&right, t)),
        Rhs::Single => {
            let logical = r.tag(&right, "binary")
                && r.operator(&right)
                    .and_then(|o| r.g.operator_kind(&o).map(|k| k == "logical"))
                    .unwrap_or(false);
            !named && pos == 1 && !logical && !r.tag(&right, "conditional")
        }
        Rhs::Sequence => false,
    }
}

/// `"...".method(arguments)`.
fn method(
    r: &Reader<'_>,
    row: &FormatRow,
    flavor: Flavor,
    call: &SyntaxNode,
) -> Option<Vec<Finding>> {
    let callee = r
        .field(call, "callee")
        .filter(|f| r.tag(f, "member-access"))?;
    let string = r.field(&callee, "object").filter(|s| r.is_string(s))?;
    if code_text(&r.g.slot(&callee, "member")?.into_node()?) != row.name {
        return None;
    }
    let spec = match flavor.parse(&r.string_value(&string)) {
        Ok(spec) => spec,
        Err(Failure::Invalid(why)) => return Some(vec![(Kind::BraceInvalid, why.to_owned())]),
        Err(Failure::Char(c)) => return Some(vec![(Kind::BraceInvalid, c.to_string())]),
    };
    let mut out: Vec<Finding> = Vec::new();
    if spec.auto > 0 && !spec.index.is_empty() {
        out.push((Kind::BraceMixing, String::new()));
    }
    let (mut args, mut keywords) = (Vec::new(), Vec::new());
    if let Some(list) = r.g.slot(call, "arguments").and_then(|a| a.into_node()) {
        for c in list.children() {
            if r.tag(&c, "key-value") {
                keywords.push(c);
            } else {
                args.push(c);
            }
        }
    }
    let starred = |a: &SyntaxNode| r.tag(a, "rest");
    let used = |i: usize| i < spec.auto || spec.index.contains(&i);
    let unused: Vec<usize> = (0..args.len())
        .filter(|&i| !starred(&args[i]) && !used(i))
        .collect();
    if !unused.is_empty() {
        out.push((Kind::BraceExtraPositional, join(unused)));
    }
    let named: Vec<String> = keywords
        .iter()
        .filter_map(|kw| r.g.slot(kw, "key")?.into_node().map(|k| code_text(&k)))
        .collect();
    let splat = named.len() < keywords.len();
    if !splat {
        let extra: Vec<_> = named.iter().filter(|n| !spec.names.contains(n)).collect();
        if !extra.is_empty() {
            out.push((Kind::BraceExtraNames, join(extra)));
        }
    }
    if !splat && !args.iter().any(starred) {
        let positions = (0..spec.auto)
            .chain(spec.index.iter().copied())
            .filter(|&i| i >= args.len())
            .collect::<BTreeSet<_>>();
        let names = spec.names.iter().filter(|k| !named.contains(k));
        let missing = join(
            positions
                .into_iter()
                .map(|i| i.to_string())
                .chain(names.cloned()),
        );
        if !missing.is_empty() {
            out.push((Kind::BraceMissing, missing));
        }
    }
    // The arguments move into the string: they must not hold its quote character.
    let quote = string
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| r.g.has_tag(t.kind(), "string"))
        .and_then(|t| t.text().chars().find(|c| matches!(c, '\'' | '"')));
    let clash = |a: &SyntaxNode| quote.is_none_or(|q| code_text(a).contains(q));
    if out.is_empty()
        && !splat
        && !args.iter().any(starred)
        && !args.iter().any(clash)
        && !keywords.iter().any(clash)
    {
        out.push((Kind::BraceToInterpolation, String::new()));
    }
    Some(out)
}
