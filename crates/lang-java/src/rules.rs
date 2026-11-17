//! The Rust rules of Java (Checkstyle): the checks that read several nodes. The checks on one
//! node's own text are pattern files in `rules/`.

use crate::generated::kind as k;
use core_rules::options::{Check, Opt, OptionDecl};
use core_rules::{Re, Rule, RuleContext, rule};
use core_syntax::{SyntaxNode, SyntaxToken, TextRange};

pub static RULES: &[&dyn Rule] = &[
    &NamingConvention,
    &ModifierOrder,
    &crate::scope::UNUSED_IMPORT,
    &crate::scope::UNUSED_LOCAL,
    &crate::scope::PARAMETER_ASSIGNMENT,
];

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn ident(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    tokens(node).filter(|t| t.kind() == k::IDENTIFIER)
}

/// The nodes below `node` that are not in a nested declaration of the same kind of body, in
/// document order: the declarators of a declaration, whether or not a `*Core` node wraps them.
fn declarators(node: &SyntaxNode) -> Vec<SyntaxNode> {
    node.children()
        .flat_map(|c| match c.kind() {
            k::VARIABLE_DECLARATOR => vec![c],
            k::LOCAL_VARIABLE_DECLARATION_CORE => c
                .children()
                .filter(|d| d.kind() == k::VARIABLE_DECLARATOR)
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn has_modifier(node: &SyntaxNode, words: &[&str]) -> bool {
    node.children()
        .filter(|c| c.kind() == k::MODIFIERS)
        .flat_map(|m| tokens(&m).collect::<Vec<_>>())
        .filter(|t| t.kind() == k::KEYWORD)
        .filter(|t| words.contains(&t.text()))
        .count()
        == words.len()
}

macro_rules! pattern_option {
    ($name:literal, $what:literal) => {
        OptionDecl {
            name: $name,
            default: Opt::Str(""),
            check: Check::Regex,
            description: concat!(
                "Regular expression a ",
                $what,
                " name must match; empty: not checked."
            ),
        }
    };
}

/// The option that checks the name of `node` and the name's token(s).
fn names(node: &SyntaxNode) -> Vec<(&'static str, SyntaxToken)> {
    let first =
        |opt: &'static str, t: Option<SyntaxToken>| t.map(|t| (opt, t)).into_iter().collect();
    match node.kind() {
        k::CLASS_DECLARATION_CORE
        | k::INTERFACE_DECLARATION_CORE
        | k::ENUM_DECLARATION_CORE
        | k::RECORD_DECLARATION_CORE
        | k::ANNOTATION_TYPE_DECLARATION_CORE => first("types", ident(node).next()),
        k::METHOD_DECLARATION => {
            let before_params: Vec<_> = node
                .children_with_tokens()
                .take_while(|e| e.kind() != k::FORMAL_PARAMETERS)
                .filter_map(|e| e.into_token())
                .filter(|t| t.kind() == k::IDENTIFIER)
                .collect();
            first("methods", before_params.last().cloned())
        }
        k::FORMAL_PARAMETER | k::SPREAD_PARAMETER => first(
            "parameters",
            ident(node)
                .last()
                .or_else(|| declarators(node).first().and_then(|d| ident(d).next())),
        ),
        k::CATCH_FORMAL_PARAMETER => first("catch_parameters", ident(node).last()),
        k::FIELD_DECLARATION | k::CONSTANT_DECLARATION | k::LOCAL_VARIABLE_DECLARATION => {
            let option = match node.kind() {
                k::LOCAL_VARIABLE_DECLARATION => "locals",
                k::CONSTANT_DECLARATION => "constants",
                _ if has_modifier(node, &["static", "final"]) => "constants",
                _ => "members",
            };
            declarators(node)
                .iter()
                .filter_map(|d| ident(d).next())
                .map(|t| (option, t))
                .collect()
        }
        _ => Vec::new(),
    }
}

rule! {
    pub NamingConvention {
        id: "naming-convention",
        category: Style,
        default: Off,
        description: "A name does not match the regular expression set for what it names (Checkstyle \
                  PackageName, TypeName, MethodName, MemberName, ConstantName, ParameterName, \
                  CatchParameterName and LocalVariableName). A `static final` field is a constant, \
                  as is a field of an interface; the other fields are members. No fix: renaming \
                  changes every use.",
        example: "class my_class {   // types: ^[A-Z][a-zA-Z0-9]*$\n}",
        options: [
            pattern_option!("packages", "package"),
            pattern_option!("types", "class, interface, enum, record or annotation type"),
            pattern_option!("methods", "method"),
            pattern_option!("members", "field"),
            pattern_option!("constants", "constant"),
            pattern_option!("parameters", "method parameter"),
            pattern_option!("catch_parameters", "catch parameter"),
            pattern_option!("locals", "local variable"),
        ],
        kinds: &[k::PROGRAM],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let pattern = |ctx: &RuleContext<'_>, option: &'static str| -> Option<(String, Re)> {
            let text = ctx.options.string(option);
            if text.is_empty() {
                return None;
            }
            Re::lazy(text).ok().map(|re| (text.to_string(), re))
        };
        let mut found: Vec<(TextRange, String)> = Vec::new();
        let mut cache: Vec<(&'static str, Option<(String, Re)>)> = Vec::new();
        for def in node.descendants() {
            if def.kind() == k::PACKAGE_DECLARATION {
                let Some((text, re)) = pattern(ctx, "packages") else { continue };
                let words: Vec<_> = tokens_deep(&def);
                let (Some(a), Some(b)) = (words.first(), words.last()) else { continue };
                let name: String = words.iter().map(|t| t.text()).collect();
                if !re.is_match(&name) {
                    found.push((
                        TextRange::new(a.text_range().start(), b.text_range().end()),
                        format!("package name `{name}` must match `{text}`"),
                    ));
                }
                continue;
            }
            for (option, token) in names(&def) {
                if !cache.iter().any(|(o, _)| *o == option) {
                    cache.push((option, pattern(ctx, option)));
                }
                let Some((text, re)) = cache.iter().find(|(o, _)| *o == option).and_then(|(_, p)| p.as_ref()) else {
                    continue;
                };
                if !re.is_match(token.text()) {
                    found.push((token.text_range(), format!("name `{}` must match `{text}`", token.text())));
                }
            }
        }
        for (range, message) in found {
            ctx.report(range, message);
        }
    }
}

/// The name tokens of a package declaration: what is between `package` and `;`.
fn tokens_deep(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::IDENTIFIER || t.kind() == k::DOT)
        .collect()
}

const MODIFIER_ORDER: [&str; 14] = [
    "public",
    "protected",
    "private",
    "abstract",
    "default",
    "static",
    "sealed",
    "non-sealed",
    "final",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
];

rule! {
    pub ModifierOrder {
        id: "modifier-order",
        category: Style,
        default: Off,
        description: "The modifiers of a declaration are not in the order of the Java Language \
                  Specification: `public protected private abstract default static sealed non-sealed \
                  final transient volatile synchronized native strictfp` (Checkstyle ModifierOrder). \
                  No fix: the check does not rewrite code.",
        example: "static public final int X = 1;   // public static final",
        kinds: &[k::MODIFIERS],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut top = 0;
        for t in tokens(node).filter(|t| t.kind() == k::KEYWORD) {
            let Some(rank) = MODIFIER_ORDER.iter().position(|m| *m == t.text()) else { continue };
            if rank < top {
                ctx.report(t.text_range(), format!("`{}` is out of order", t.text()));
                return;
            }
            top = rank;
        }
    }
}
