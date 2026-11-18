//! The CSS targets of the shared `names` family (`core_rules::listed`): what the names of a kind
//! of thing are, read off the CSS tree.

use super::model::{self, Part, plain, split_dimension};
use crate::generated::kind as k;
use core_rules::listed::{Mode, Targets};
use core_rules::tree::significant_range;
use core_rules::{Family, RuleContext};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt, TextRange};

pub const FAMILY: Family = core_rules::listed::family::<Css>();

pub struct Css;

#[derive(Clone, Copy, PartialEq)]
pub enum Target {
    Property,
    CustomProperty,
    Function,
    Unit,
    AtRule,
    MediaFeature,
    KeyframesName,
    CustomMedia,
    Layer,
    Container,
    Combinator,
    Selector,
    AttributeOperator,
    FontFamily,
    Part(Part),
}

impl Targets for Css {
    type Target = Target;
    const ALL: &'static [(&'static str, Target)] = &[
        ("property", Target::Property),
        ("custom-property", Target::CustomProperty),
        ("function", Target::Function),
        ("unit", Target::Unit),
        ("at-rule", Target::AtRule),
        ("media-feature", Target::MediaFeature),
        ("keyframes-name", Target::KeyframesName),
        ("custom-media", Target::CustomMedia),
        ("layer", Target::Layer),
        ("container", Target::Container),
        ("combinator", Target::Combinator),
        ("selector", Target::Selector),
        ("attribute-operator", Target::AttributeOperator),
        ("font-family", Target::FontFamily),
        ("class", Target::Part(Part::Class)),
        ("id", Target::Part(Part::Id)),
        ("attribute", Target::Part(Part::Attribute)),
        ("pseudo-class", Target::Part(Part::PseudoClass)),
        ("pseudo-element", Target::Part(Part::PseudoElement)),
        ("type", Target::Part(Part::Type)),
    ];

    fn kinds(target: Target) -> &'static [SyntaxKind] {
        use Target::*;
        match target {
            Property | FontFamily => &[k::DECLARATION],
            CustomProperty => &[k::DECLARATION, k::AT_RULE],
            Function => &[k::FUNCTION],
            Unit => &[k::VALUE, k::PRELUDE],
            AtRule | MediaFeature | KeyframesName | CustomMedia | Layer | Container => {
                &[k::AT_RULE]
            }
            Combinator | Selector | AttributeOperator | Part(_) => &[k::RULE],
        }
    }

    fn insensitive(target: Target) -> bool {
        !matches!(
            target,
            Target::Part(Part::Class | Part::Id | Part::Attribute | Part::Type)
                | Target::KeyframesName
                | Target::Selector
                | Target::FontFamily
                | Target::Function
        )
    }

    fn keep_case(target: Target) -> &'static [&'static str] {
        if target == Target::Function {
            CAMEL
        } else {
            &[]
        }
    }

    fn collect(
        target: Target,
        mode: Mode,
        node: &SyntaxNode,
        ctx: &RuleContext<'_>,
        out: &mut Vec<(String, TextRange)>,
    ) {
        collect(target, mode, node, ctx, out)
    }
}

/// The names in `node` with where each is.
fn collect(
    target: Target,
    mode: Mode,
    node: &SyntaxNode,
    ctx: &RuleContext<'_>,
    out: &mut Vec<(String, TextRange)>,
) {
    {
        let range = |n: &SyntaxNode| significant_range(n, ctx.file.grammar);
        match target {
            Target::Property => {
                if let Some((name, at)) = model::property(node)
                    && !model::is_custom(&name)
                {
                    out.push((name, at));
                }
            }
            // The names without `--`: declared, used in `var()` and named by `@property`.
            Target::CustomProperty if node.kind() == k::AT_RULE => {
                if model::at_name(node) == "property"
                    && let Some(p) = node.child(k::PRELUDE)
                {
                    out.push((plain(&p).trim_start_matches("--").to_string(), range(&p)));
                }
            }
            Target::CustomProperty => {
                if let Some((name, at)) = model::property(node)
                    && model::is_custom(&name)
                {
                    out.push((name[2..].to_string(), at));
                }
                for f in node.descendants().filter(|n| n.kind() == k::FUNCTION) {
                    let var = model::function_name(&f)
                        .is_some_and(|t| t.text().eq_ignore_ascii_case("var"));
                    let first = f.child(k::GROUP).and_then(|g| model::tokens(&g).nth(1));
                    if let (true, Some(t)) = (var, first)
                        && t.text().starts_with("--")
                    {
                        out.push((t.text()[2..].to_string(), t.text_range()));
                    }
                }
            }
            Target::Function => {
                if let (false, Some(t)) = (after_colon(node), model::function_name(node)) {
                    out.push((t.text().to_string(), t.text_range()));
                }
            }
            Target::Unit => {
                if node.kind() == k::PRELUDE && node.parent().is_none_or(|p| p.kind() != k::AT_RULE)
                {
                    return;
                }
                for t in model::tokens(node) {
                    match t.kind() {
                        k::DIMENSION => {
                            out.push((split_dimension(t.text()).1.to_string(), t.text_range()))
                        }
                        k::PERCENTAGE => out.push(("%".into(), t.text_range())),
                        _ => {}
                    }
                }
            }
            Target::AtRule => {
                if let Some(t) = node.token(k::AT_KEYWORD) {
                    let name = model::at_name(node);
                    out.push((model::unprefixed(&name).to_string(), t.text_range()));
                }
            }
            Target::MediaFeature => {
                if model::at_name(node) == "media"
                    && let Some(prelude) = node.child(k::PRELUDE)
                {
                    for g in prelude.descendants().filter(|n| n.kind() == k::GROUP) {
                        if let Some(t) = model::tokens(&g).nth(1)
                            && t.kind() == k::IDENT
                            && model::tokens(&g)
                                .next()
                                .is_some_and(|o| o.kind() == k::L_PAREN)
                        {
                            out.push((t.text().to_string(), t.text_range()));
                        }
                    }
                }
            }
            Target::KeyframesName => {
                if model::is_keyframes(node)
                    && let Some(p) = node.child(k::PRELUDE)
                {
                    out.push((
                        model::plain(&p).trim_matches(['"', '\'']).to_string(),
                        range(&p),
                    ));
                }
            }
            Target::CustomMedia => {
                if model::at_name(node) == "custom-media"
                    && let Some(t) = node
                        .child(k::PRELUDE)
                        .and_then(|p| model::tokens(&p).next())
                {
                    out.push((
                        t.text().trim_start_matches("--").to_string(),
                        t.text_range(),
                    ));
                }
            }
            Target::Layer => {
                if model::at_name(node) == "layer"
                    && let Some(p) = node.child(k::PRELUDE)
                {
                    for part in model::tokens(&p)
                        .collect::<Vec<_>>()
                        .split(|t| t.kind() == k::COMMA)
                    {
                        if let (Some(a), Some(b)) = (part.first(), part.last()) {
                            let name = part.iter().map(|t| t.text()).collect();
                            out.push((name, a.text_range().cover(b.text_range())));
                        }
                    }
                }
            }
            Target::Container => {
                if model::at_name(node) == "container"
                    && let Some(t) = node
                        .child(k::PRELUDE)
                        .and_then(|p| model::tokens(&p).next())
                    && t.kind() == k::IDENT
                    && !matches!(t.text().to_ascii_lowercase().as_str(), "not" | "and" | "or")
                {
                    out.push((t.text().to_string(), t.text_range()));
                }
            }
            Target::FontFamily => {
                if let Some(families) = super::font::families(node) {
                    out.extend(families.into_iter().map(|f| (f.name, f.range)));
                }
            }
            Target::Combinator | Target::Selector | Target::AttributeOperator | Target::Part(_) => {
                let Some(prelude) = node.child(k::PRELUDE) else {
                    return;
                };
                // Keyframe selectors are not selectors, except for their case.
                if model::in_keyframes(node) && mode != Mode::Lower {
                    return;
                }
                if target == Target::Selector {
                    // The whole list, as stylelint reads it.
                    let list: Vec<String> = model::selectors(&prelude)
                        .into_iter()
                        .map(|s| s.text)
                        .collect();
                    out.push((list.join(", "), range(&prelude)));
                    return;
                }
                for sel in model::selectors(&prelude) {
                    match target {
                        Target::AttributeOperator => {
                            for p in sel.pieces().filter(|p| p.part == Part::Attribute) {
                                let text = &ctx.file.text
                                    [usize::from(p.range.start())..usize::from(p.range.end())];
                                let after =
                                    &text[text.find(&*p.name).map_or(0, |i| i + p.name.len())..];
                                let op: String = after
                                    .trim_start()
                                    .chars()
                                    .take_while(|c| "~|^$*=".contains(*c))
                                    .collect();
                                if !op.is_empty() {
                                    out.push((op, p.range));
                                }
                            }
                        }
                        Target::Combinator => {
                            out.extend(sel.combinators.iter().map(|(c, r)| (c.clone(), *r)))
                        }
                        Target::Part(part) => out.extend(
                            sel.pieces()
                                .filter(|p| p.part == part)
                                .map(|p| (p.name.clone(), p.range)),
                        ),
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Whether a `:` comes just before `node` (a pseudo-class, not a function).
pub(super) fn after_colon(node: &SyntaxNode) -> bool {
    prev_token(node).is_some_and(|t| t.kind() == k::COLON)
}

fn prev_token(node: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
    let mut e = node.prev_sibling_or_token();
    while let Some(el) = e {
        match el {
            core_syntax::SyntaxElement::Token(t) if model::is_trivia(t.kind()) => {
                e = t.prev_sibling_or_token()
            }
            core_syntax::SyntaxElement::Token(t) => return Some(t),
            core_syntax::SyntaxElement::Node(_) => return None,
        }
    }
    None
}

/// `translateX` and the like keep their capitals.
const CAMEL: &[&str] = &[
    "translateX",
    "translateY",
    "translateZ",
    "scaleX",
    "scaleY",
    "scaleZ",
    "rotateX",
    "rotateY",
    "rotateZ",
    "skewX",
    "skewY",
];
