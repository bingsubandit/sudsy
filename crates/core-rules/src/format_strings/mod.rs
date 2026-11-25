//! Format strings that do not fit their arguments, and the ones an interpolated string would
//! say better. The grammar's `format` rows say which strings are format strings (`percent`,
//! `brace`: [`flavor`]) and where their arguments are (`operator`: the right operand, `method`:
//! the arguments of the call); the tags of [`shape`] say what an argument is. A format string is
//! read as the language's library reads it; which right-hand sides are one value, a sequence or a
//! mapping follows ruff. One analysis of a node serves every rule: each reports its own findings.
//!
//! concept: format-string-checks
//! what: Format strings that do not fit their arguments, by flavor (percent, brace).
//! tools: ruff F501-F509 F521-F525, pyupgrade UP031 UP032
//! reads: grammar:format

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
        $description:literal $why:literal $example:literal;)*) => {
        pub static RULES: &[FormatRule] = &[$(FormatRule {
            meta: &RuleMeta {
                id: $id,
                category: Category::$category,
                why: $why,
                source: "",
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
        "Reports a `\"...\" % x` whose format string does not parse, such as a `%` at the \
         end of the string."
        "Python raises `ValueError: incomplete format` when the line runs, so the failure \
         is at run time and only on the path that reaches the line.\n\
         To fix, escape a literal percent sign as `%%`, or complete the conversion."
        "\"%\" % x";
    PercentCharacter "percent-format-unsupported-format-character" Correctness Warn
        "`%`-format string has an unsupported format character `{name}`"
        "Reports a `%` format with a conversion character that Python does not know, as in \
         `\"%z\" % x`."
        "Python raises `ValueError: unsupported format character` when the line runs. It \
         is usually a literal `%` that should have been written `%%`.\n\
         To fix, write `%%` for a percent sign, or a conversion Python has (`s`, `r`, `a`, \
         `d`, `i`, `f`, `x` and so on)."
        "\"%z\" % x";
    PercentMixed "percent-format-mixed-positional-and-named" Correctness Warn
        "`%`-format string mixes positional and named placeholders"
        "Reports a `%` format that has both positional placeholders (`%s`) and named ones \
         (`%(a)s`)."
        "A format takes its values from a tuple or from a mapping, not both. Python raises \
         a `TypeError` at run time.\n\
         To fix, use only named placeholders with a dict, or only positional ones with a \
         tuple."
        "\"%s %(a)s\" % x";
    PercentMapping "percent-format-expected-mapping" Correctness Warn
        "`%`-format string has named placeholders but the right side is a sequence"
        "Reports a `%` format that has named placeholders (`%(a)s`) applied to a tuple, \
         list or set."
        "Named placeholders look their value up in a mapping. With a tuple, Python raises \
         `TypeError: format requires a mapping` when the line runs.\n\
         To fix, pass a dict, or use positional placeholders."
        "\"%(a)s\" % (1,)";
    PercentSequence "percent-format-expected-sequence" Correctness Warn
        "`%`-format string has several placeholders but the right side is a mapping"
        "Reports a `%` format that has several positional placeholders applied to a dict."
        "Positional placeholders take one value each from a tuple. A dict is taken as one \
         value, so with two or more placeholders Python raises `TypeError: not enough \
         arguments for format string`.\n\
         To fix, pass a tuple, or name the placeholders (`%(a)s`)."
        "\"%s %s\" % {\"a\": 1}";
    PercentExtraNames "percent-format-extra-named-arguments" Correctness Warn
        "`%`-format string has unused named argument(s): {name}"
        "Reports a `%` format applied to a dict that has keys that no placeholder names."
        "Python ignores the extra keys without an error. The key is a leftover after the \
         format string was edited, or a placeholder was misspelled and the value is never \
         shown.\n\
         To fix, remove the key, or add the placeholder."
        "\"%(a)s\" % {\"a\": 1, \"b\": 2}";
    PercentMissingName "percent-format-missing-argument" Correctness Warn
        "`%`-format string is missing argument(s) for placeholder(s): {name}"
        "Reports a `%` format with named placeholders that the dict does not have."
        "Python raises a `KeyError` for the first missing key when the line runs. A key \
         that is misspelled in the string or in the dict is the usual cause.\n\
         To fix, add the key, or correct the name."
        "\"%(a)s\" % {\"b\": 1}";
    PercentCount "percent-format-positional-count-mismatch" Correctness Warn
        "`%`-format string has {name}"
        "Reports a `%` format whose number of placeholders differs from the number of \
         values in the tuple."
        "Python raises a `TypeError` when the line runs: `not enough arguments for format \
         string` with too few values, `not all arguments converted during string \
         formatting` with too many.\n\
         To fix, make the two counts the same."
        "\"%s %s\" % (1,)";
    PercentStar "percent-format-star-requires-sequence" Correctness Warn
        "`%`-format string `*` specifier requires a sequence"
        "Reports a `%` format with a `*` width or precision applied to a dict."
        "A `*` takes its number from a tuple of values. \
         Python raises `TypeError: * wants int`.\n\
         To fix, pass a tuple with the width before the value, or write the number in the \
         format."
        "\"%*s\" % {\"a\": 1}";
    BraceInvalid "string-dot-format-invalid-format" Correctness Warn
        "`.format` call has an invalid format string: {name}"
        "Reports a `\"...\".format(...)` call whose string does not parse, such as an \
         unmatched `{` or `}`."
        "Python raises a `ValueError` (`Single '{' encountered in format string`) when the \
         line runs. It is usually a literal brace that should have been doubled.\n\
         To fix, write `{{` and `}}` for literal braces."
        "\"{\".format(1)";
    BraceExtraNames "string-dot-format-extra-named-arguments" Correctness Warn
        "`.format` call has unused named argument(s): {name}"
        "Reports a `.format` call with keyword arguments that no field names."
        "Python ignores the extra keywords without an error, so a leftover after an edit, \
         or a field that was misspelled, goes unnoticed.\n\
         To fix, remove the keyword, or add the field."
        "\"{}\".format(1, a=2)";
    BraceExtraPositional "string-dot-format-extra-positional-arguments" Correctness Warn
        "`.format` call has unused arguments at position(s): {name}"
        "Reports a `.format` call with positional arguments that no field uses."
        "Python ignores the extra arguments without an error. It is usually a field that \
         was removed from the string, or one that was never added.\n\
         To fix, remove the argument, or add the field."
        "\"{}\".format(1, 2)";
    BraceMissing "string-dot-format-missing-arguments" Correctness Warn
        "`.format` call is missing argument(s) for placeholder(s): {name}"
        "Reports a `.format` call without an argument for a field of its string."
        "Python raises an `IndexError` for a positional field or a `KeyError` for a named \
         one when the line runs.\n\
         To fix, pass the argument, or correct the field."
        "\"{} {}\".format(1)";
    BraceMixing "string-dot-format-mixing-automatic" Correctness Warn
        "`.format` string mixes automatic and manual numbering"
        "Reports a `.format` string that has both automatic fields (`{}`) and numbered \
         ones (`{0}`)."
        "Python raises `ValueError: cannot switch from automatic field numbering to manual \
         field specification` when the line runs.\n\
         To fix, use one kind of field in a string."
        "\"{} {0}\".format(1)";
    PrintfToInterpolation "printf-string-formatting" Style Off
        "use format specifiers or an f-string instead of `%` formatting"
        "Reports a `\"...\" % x` whose format string and values pair up: a tuple with as \
         many values as placeholders, a dict for named fields, or one value for one \
         placeholder."
        "The `%` operator is the oldest of Python's three ways to format a string. An \
         f-string or `.format` puts the value where it is used, and a single value or a \
         tuple that is mistaken for the arguments (`\"%s\" % some_tuple`) is a classic bug \
         of `%`.\n\
         To fix, write an f-string, or use `.format`."
        "x = \"%s\" % y";
    BraceToInterpolation "f-string" Style Off
        "use an f-string instead of a `.format` call"
        "Reports a `\"...\".format(...)` call on a string literal whose fields all name an \
         argument."
        "An f-string puts the expression where it is shown, so the reader does not look \
         from the field to the argument list and count positions. It is also the faster \
         form.\n\
         To fix, write an f-string."
        "x = \"{}\".format(y)";
}

impl Rule for FormatRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
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
