//! The HTML targets of the shared `names` family (`core_rules::listed`): the value of `id`, each
//! word of `class`, the value of `name` on form controls, or the name of every attribute.

use crate::dom;
use crate::generated::kind as k;
use core_rules::listed::{Mode, Targets};
use core_rules::{Family, RuleContext};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

pub const FAMILY: Family = core_rules::listed::family::<Html>();

pub struct Html;

#[derive(Clone, Copy, PartialEq)]
pub enum Target {
    Id,
    Class,
    Name,
    AttrName,
}

impl Targets for Html {
    type Target = Target;
    const ALL: &'static [(&'static str, Target)] = &[
        ("id", Target::Id),
        ("class", Target::Class),
        ("name", Target::Name),
        ("attr-name", Target::AttrName),
    ];

    fn kinds(_: Target) -> &'static [SyntaxKind] {
        &[k::START_TAG]
    }

    fn collect(
        target: Target,
        _: Mode,
        tag: &SyntaxNode,
        ctx: &RuleContext<'_>,
        out: &mut Vec<(String, TextRange)>,
    ) {
        let text = ctx.file.text;
        let Some((element, _)) = dom::tag_name(tag, text) else {
            return;
        };
        let controls = [
            "input", "select", "textarea", "button", "fieldset", "output",
        ];
        for a in dom::attrs(tag, text) {
            let wanted = match target {
                Target::AttrName => {
                    out.push((text[a.key].to_string(), a.key));
                    continue;
                }
                Target::Id => "id",
                Target::Class => "class",
                Target::Name => "name",
            };
            if a.name != wanted || (wanted == "name" && !controls.contains(&element.as_str())) {
                continue;
            }
            let (Some(value), Some(range)) = (a.value, a.value_range) else {
                continue;
            };
            if target == Target::Class {
                for word in value.split_ascii_whitespace() {
                    let offset = word.as_ptr() as usize - value.as_ptr() as usize;
                    let at = range.start() + TextSize::try_from(offset).unwrap_or_default();
                    let len = TextSize::try_from(word.len()).unwrap_or_default();
                    out.push((word.to_string(), TextRange::at(at, len)));
                }
            } else {
                out.push((value.to_string(), range));
            }
        }
    }
}
