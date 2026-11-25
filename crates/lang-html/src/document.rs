//! Rules that read the whole document: ids and references, classes, headings, `<main>`, comments.
//!
//! concept: new no-duplicate-id, new no-duplicate-class, new no-multiple-main, new heading-level, new no-missing-references, new no-redundant-for, new no-conditional-comment, new input-missing-label, new multiple-labeled-controls, new unique-landmark, new map-dup-name, new unrecognized-char-ref, unique-names
//! what: no-duplicate-id: An `id` that an earlier element of the document already has
//! what: no-duplicate-class: A class name written twice in one `class` attribute
//! what: no-multiple-main: A second `<main>` element that is not `hidden`: a page has one main landmark
//! what: heading-level: Headings do not follow the document outline: the first heading is not an `<h1>`, a level is skipped going down the outline (`<h1>` then `<h3>`), or the page has a second
//! what: no-missing-references: An attribute that refers to an `id` (`for`, `aria-labelledby`, `list`, `headers`, `form`, ...) names an id that no element of the document has
//! what: no-redundant-for: A `<label>` has a `for` that names a control inside it, which the label already labels
//! what: no-conditional-comment: An Internet Explorer conditional comment (`<!--[if IE]>`)
//! what: input-missing-label: An `<input>` (not hidden or a button), `<select>` or `<textarea>` that no `<label>` names: it is not inside one, no `<label for>` points at its `id`, and it has no `aria-
//! what: multiple-labeled-controls: A `<label>` without `for` that holds more than one control: it labels only the first
//! what: unique-landmark: Landmarks of one kind (`<nav>`, `<aside>`, or an element with the same landmark `role`) that a screen reader user cannot tell apart: with several of them, each needs an `
//! what: map-dup-name: A `<map>` whose `name` an earlier `<map>` already has: `usemap` finds the first
//! what: unrecognized-char-ref: A named character reference in text (`&name;`) that HTML does not define

use crate::dom;
use crate::generated::kind as k;
use crate::meta;
use core_rules::heading_outline::{self, Outline, Several};
use core_rules::unique::Repeats;
use core_rules::{FileContext, Rule, RuleContext, rule};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange, TextSize};
use std::collections::HashSet;

pub static RULES: &[&dyn Rule] = &[
    &NoDuplicateId,
    &NoDuplicateClass,
    &NoMultipleMain,
    &HeadingLevel,
    &NoMissingReferences,
    &NoRedundantFor,
    &NoConditionalComment,
    &InputMissingLabel,
    &MultipleLabeledControls,
    &UniqueLandmark,
    &MapDupName,
    &UnrecognizedCharRef,
    &crate::content::PERMITTED_CONTENT,
    &crate::content::PERMITTED_ORDER,
    &crate::content::PERMITTED_OCCURRENCES,
    &crate::content::REQUIRED_CONTENT,
    &crate::access::LINK_TEXT,
    &crate::access::FORM_SUBMIT,
    &crate::access::HEADER_SCOPE,
    &crate::access::HIDDEN_FOCUSABLE,
    &crate::access::IMG_ALT,
    &crate::access::CLICK_KEY_EVENTS,
    &crate::access::INTERACTIVE_FOCUS,
    &crate::access::POINTER_KEY_EVENTS,
    &crate::access::LABEL_CONTROL,
    &crate::access::EMPTY_CONTENT,
    &crate::access::NO_AUTOFOCUS,
];

rule! {
    NoDuplicateId {
        id: "no-duplicate-id",
        category: Correctness,
        default: Warn,
        description: "Reports an `id` that an earlier element of the document already has.",
        why: "Ids must be unique. Scripts, links, labels and styles find the first element only, \
              so the second one is silently unreachable.\n\
              To fix, give the element another id.",
        example: "<p id=\"a\"></p><p id=\"a\"></p>   // reported at the second",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut seen = Repeats::default();
        for tag in dom::start_tags(file.root) {
            for a in dom::attrs(&tag, file.text).into_iter().filter(|a| a.name == "id") {
                let (Some(id), Some(range)) = (a.value, a.value_range) else {
                    continue;
                };
                seen.check(ctx, id, range, || format!("duplicate id `{id}`"), "first used here");
            }
        }
    }
}

rule! {
    NoDuplicateClass {
        id: "no-duplicate-class",
        category: Suspicious,
        default: Off,
        description: "Reports a class name written twice in one `class` attribute.",
        why: "The second one does nothing, and it is usually left over from a merge or from a \
              class that should have had another name.\n\
              To fix, remove the repeat.",
        example: "<p class=\"a b a\">   // reported at the second `a`",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for tag in dom::start_tags(file.root) {
            for a in dom::attrs(&tag, file.text).into_iter().filter(|a| a.name == "class") {
                let (Some(value), Some(range)) = (a.value, a.value_range) else {
                    continue;
                };
                let mut seen = Vec::new();
                for word in value.split_ascii_whitespace() {
                    let offset = word.as_ptr() as usize - value.as_ptr() as usize;
                    if word.starts_with(['{', '}', '%']) || crate::hooks::in_template(&value[..offset]) {
                        continue;
                    }
                    if seen.contains(&word) {
                        let at = TextRange::at(
                            range.start() + TextSize::try_from(offset).unwrap_or_default(),
                            TextSize::try_from(word.len()).unwrap_or_default(),
                        );
                        ctx.report(at, format!("class `{word}` is repeated"));
                    }
                    seen.push(word);
                }
            }
        }
    }
}

rule! {
    NoMultipleMain {
        id: "no-multiple-main",
        category: Correctness,
        default: Warn,
        description: "Reports a second `<main>` element that is not `hidden`.",
        why: "A page has one main landmark; a second one makes the shortcut of a screen reader \
              to the main content ambiguous.\n\
              To fix, use one `<main>`, or mark the other `hidden`.",
        example: "<main></main><main></main>   // reported at the second",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut first = None;
        for tag in dom::start_tags(file.root) {
            let Some((name, range)) = dom::tag_name(&tag, file.text) else {
                continue;
            };
            if name != "main" || dom::attrs(&tag, file.text).iter().any(|a| a.name == "hidden") {
                continue;
            }
            match first {
                Some(at) => {
                    ctx.report(range, "the page has more than one `<main>`").related(at, "first `<main>`");
                }
                None => first = Some(range),
            }
        }
    }
}

/// What `heading-level` checks, and how HTML names a heading.
const OUTLINE: Outline = Outline {
    first_is_top: true,
    skips: true,
    several: Several::All,
    label: |n| format!("<h{n}>"),
    many: "more than one `<h1></h1>` on the page",
};

rule! {
    HeadingLevel {
        id: "heading-level",
        category: Correctness,
        default: Off,
        description: "Reports a heading that breaks the outline of the document: the first heading is \
              not an `<h1>`, a level is skipped going down (`<h1>` then `<h3>`), or the page has \
              more than one `<h1>` (each one is reported).",
        why: "Headings make the table of contents that screen reader users and search engines \
              build. A skipped level suggests a section that is missing, and several `<h1>` \
              leave the main topic unclear.\n\
              To fix, use the level that follows the one above, and style the size with CSS.",
        example: "<h1>A</h1><h3>B</h3>   // reported at the `<h3>`",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let headings = dom::start_tags(file.root)
            .filter_map(|tag| dom::tag_name(&tag, file.text))
            .filter_map(|(name, range)| match name.as_bytes() {
                [b'h', d @ b'1'..=b'6'] => Some(((d - b'0') as usize, range)),
                _ => None,
            });
        heading_outline::check(headings, &OUTLINE, ctx);
    }
}

rule! {
    NoMissingReferences {
        id: "no-missing-references",
        category: Correctness,
        default: Warn,
        description: "Reports an attribute that refers to an `id` (`for`, `aria-labelledby`, `list`, \
              `headers`, `form`, ...) when no element of the document has that id.",
        why: "A reference to nothing silently does nothing: a label that labels no control, a \
              description that is never read, a list that offers no choices.\n\
              To fix, correct the id or add the element that has it.",
        example: "<label for=\"x\">   // no element has id=\"x\"",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let tags: Vec<_> = dom::start_tags(file.root)
            .filter_map(|t| Some((dom::tag_name(&t, file.text)?.0, dom::attrs(&t, file.text))))
            .collect();
        let ids: HashSet<_> = tags
            .iter()
            .flat_map(|(_, attrs)| attrs.iter().filter(|a| a.name == "id").filter_map(|a| a.value))
            .collect();
        for (name, attrs) in &tags {
            for a in attrs.iter().filter(|a| meta::flagged("reference", name, &a.name)) {
                let (Some(value), Some(range)) = (a.value, a.value_range) else {
                    continue;
                };
                for id in value.split_ascii_whitespace().filter(|id| !ids.contains(id)) {
                    let offset = id.as_ptr() as usize - value.as_ptr() as usize;
                    let at = TextRange::at(
                        range.start() + TextSize::try_from(offset).unwrap_or_default(),
                        TextSize::try_from(id.len()).unwrap_or_default(),
                    );
                    ctx.report(at, format!("no element has the id `{id}`"));
                }
            }
        }
    }
}

rule! {
    NoRedundantFor {
        id: "no-redundant-for",
        category: Style,
        default: Off,
        description: "Reports a `<label>` whose `for` names a control that is inside the label.",
        why: "The label already names the control it holds, so the `for` and the `id` it needs \
              say the same thing twice, and they can drift apart when one is renamed.\n\
              To fix, remove the `for` (and the `id` if nothing else uses it).",
        example: "<label for=\"x\"><input id=\"x\"></label>   <label><input></label>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for label in dom::start_tags(file.root) {
            if dom::tag_name(&label, file.text).is_none_or(|(n, _)| n != "label") {
                continue;
            }
            let attrs = dom::attrs(&label, file.text);
            let Some((for_attr, target)) = attrs.iter().find_map(|a| Some((a, a.value.filter(|_| a.name == "for")?))) else {
                continue;
            };
            let Some(element) = label.parent() else {
                continue;
            };
            let holds = dom::start_tags(&element).skip(1).any(|t| {
                dom::attrs(&t, file.text).iter().any(|a| a.name == "id" && a.value == Some(target))
            });
            if holds {
                ctx.report(for_attr.key, "the `for` is redundant: the label holds the control");
            }
        }
    }
}

rule! {
    NoConditionalComment {
        id: "no-conditional-comment",
        category: Suspicious,
        default: Off,
        description: "Reports an Internet Explorer conditional comment (`<!--[if IE]>`).",
        why: "Only Internet Explorer 9 and older read the content; every other browser sees a \
              comment, so it is code that no current browser runs.\n\
              To fix, remove the comment, or replace it with feature detection or CSS.",
        example: "<!--[if IE]><p>old</p><![endif]-->",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for t in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if t.kind() == k::COMMENT
                && (file.text[t.text_range()].starts_with("<!--[if")
                    || file.text[t.text_range()].starts_with("<!--<![endif]"))
            {
                ctx.report(t.text_range(), "conditional comments are only read by old Internet Explorer");
            }
        }
    }
}

/// The lowercase name and attributes of every start tag, in document order, with the tag.
type Tag<'a> = (SyntaxNode, String, TextRange, Vec<dom::Attr<'a>>);

fn tags<'a>(file: &FileContext<'a>) -> Vec<Tag<'a>> {
    dom::start_tags(file.root)
        .filter_map(|t| {
            let (name, range) = dom::tag_name(&t, file.text)?;
            let attrs = dom::attrs(&t, file.text);
            Some((t, name, range, attrs))
        })
        .collect()
}

fn attr<'a>(attrs: &[dom::Attr<'a>], name: &str) -> Option<&'a str> {
    attrs.iter().find(|a| a.name == name).and_then(|a| a.value)
}

/// Does the element hold text (a label with none names nothing)?
fn has_text(element: &SyntaxNode) -> bool {
    element.descendants().any(|n| n.kind() == k::CHUNK)
}

/// A control a `<label>` can name: not a hidden or button-like `<input>`.
fn labelable(name: &str, attrs: &[dom::Attr<'_>]) -> bool {
    match name {
        "input" => !matches!(
            attr(attrs, "type").map(str::to_ascii_lowercase).as_deref(),
            Some("hidden" | "submit" | "reset" | "button" | "image")
        ),
        "select" | "textarea" | "meter" | "output" | "progress" => true,
        _ => false,
    }
}

rule! {
    InputMissingLabel {
        id: "input-missing-label",
        category: Correctness,
        default: Off,
        description: "Reports an `<input>` (not hidden and not a button), `<select>` or `<textarea>` \
              that no label names: it is not inside a `<label>`, no `<label for>` points at its \
              `id`, and it has no `aria-label` or `aria-labelledby`.",
        why: "A control without a name is read by a screen reader as just \"edit text\", and the \
              click target is only the control itself, not its text.\n\
              To fix, wrap the control in a `<label>`, or give it an `id` and a `<label for>`, \
              or add `aria-label`.",
        example: "<input type=\"text\">   <label>Name <input type=\"text\"></label>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let tags = tags(file);
        let targets: HashSet<_> = tags
            .iter()
            .filter(|t| t.1 == "label" && t.0.parent().is_some_and(|e| has_text(&e)))
            .filter_map(|t| attr(&t.3, "for"))
            .collect();
        for (tag, name, range, attrs) in &tags {
            if !matches!(name.as_str(), "input" | "select" | "textarea") || !labelable(name, attrs) {
                continue;
            }
            let named = attr(attrs, "aria-label").is_some()
                || attr(attrs, "aria-labelledby").is_some()
                || attr(attrs, "id").is_some_and(|id| targets.contains(id))
                || tag.ancestors().skip(2).filter(|a| a.kind() == k::ELEMENT).any(|e| {
                    e.child(k::START_TAG)
                        .and_then(|t| dom::tag_name(&t, file.text))
                        .is_some_and(|(n, _)| n == "label")
                        && has_text(&e)
                });
            if !named {
                ctx.report(*range, format!("`<{name}>` has no label"));
            }
        }
    }
}

rule! {
    MultipleLabeledControls {
        id: "multiple-labeled-controls",
        category: Correctness,
        default: Off,
        description: "Reports a `<label>` with no `for` that holds more than one control.",
        why: "A label names only the first control it holds, so the second control has no name \
              and a click on the label text goes to the wrong one.\n\
              To fix, give each control its own label, with `for` and `id`.",
        example: "<label>A <input> <input></label>   <label>A <input></label>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (tag, name, range, attrs) in tags(file) {
            if name != "label" || attr(&attrs, "for").is_some() {
                continue;
            }
            let Some(element) = tag.parent() else {
                continue;
            };
            let controls = dom::start_tags(&element)
                .skip(1)
                .filter(|t| {
                    dom::tag_name(t, file.text)
                        .is_some_and(|(n, _)| n == "button" || labelable(&n, &dom::attrs(t, file.text)))
                })
                .count();
            if controls > 1 {
                ctx.report(range, "the label holds more than one control");
            }
        }
    }
}

rule! {
    UniqueLandmark {
        id: "unique-landmark",
        category: Correctness,
        default: Off,
        description: "Reports landmarks of the same kind (`<nav>`, `<aside>`, or elements with the same \
              landmark `role`) that a screen reader user cannot tell apart: with several of \
              them, each needs an `aria-label` or `aria-labelledby` that no other has.",
        why: "A list of landmarks that holds \"navigation, navigation, navigation\" gives no way \
              to pick the right one.\n\
              To fix, give each landmark a label that tells it from the others.",
        example: "<nav></nav><nav></nav>   <nav aria-label=\"Main\"></nav><nav aria-label=\"Footer\"></nav>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut groups: std::collections::BTreeMap<String, Vec<(&str, TextRange)>> = Default::default();
        for (_, name, range, attrs) in tags(file) {
            let role = match (attr(&attrs, "role"), name.as_str()) {
                (Some(r), _) => r.trim().to_ascii_lowercase(),
                (None, "nav") => "navigation".into(),
                (None, "aside") => "complementary".into(),
                (None, "main") => "main".into(),
                _ => continue,
            };
            if matches!(
                role.as_str(),
                "navigation" | "complementary" | "main" | "banner" | "contentinfo" | "region" | "search" | "form"
            ) {
                let label = attr(&attrs, "aria-label").or_else(|| attr(&attrs, "aria-labelledby"));
                groups.entry(role).or_default().push((label.unwrap_or("").trim(), range));
            }
        }
        for landmarks in groups.values().filter(|g| g.len() > 1) {
            for (label, range) in landmarks {
                if label.is_empty() || landmarks.iter().filter(|(l, _)| l == label).count() > 1 {
                    ctx.report(*range, "landmarks of one kind need a unique, non-empty accessible name");
                }
            }
        }
    }
}

rule! {
    MapDupName {
        id: "map-dup-name",
        category: Correctness,
        default: Warn,
        description: "Reports a `<map>` whose `name` an earlier `<map>` already has.",
        why: "An image refers to its map by `usemap=\"#name\"`, and the browser finds the first \
              one, so the second map is unreachable.\n\
              To fix, give each map its own name.",
        example: "<map name=\"a\"></map><map name=\"a\"></map>   // reported at the second",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut seen = Repeats::default();
        for (_, name, _, attrs) in tags(file) {
            for a in attrs.iter().filter(|a| name == "map" && a.name == "name") {
                if let (Some(v), Some(r)) = (a.value, a.value_range) {
                    seen.check(ctx, v, r, || format!("duplicate map name `{v}`"), "first used here");
                }
            }
        }
    }
}

rule! {
    UnrecognizedCharRef {
        id: "unrecognized-char-ref",
        category: Correctness,
        default: Warn,
        description: "Reports a named character reference in text (`&name;`) that HTML does not define. \
              Numeric references and references in attribute values are not checked.",
        why: "The browser shows the text as written, `&foo;`, which was not meant; it is usually \
              a typo of an entity.\n\
              To fix, correct the name, or write `&amp;` before it if the text is meant.",
        example: "<p>&foo;</p>   <p>&amp;</p>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for t in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if t.kind() != k::ENTITY {
                continue;
            }
            let text = &file.text[t.text_range()];
            if let Some(name) = text.strip_prefix('&').and_then(|r| r.strip_suffix(';'))
                && !name.starts_with('#')
                && !meta::has("entities", name)
            {
                ctx.report(t.text_range(), format!("unknown character reference `{text}`"));
            }
        }
    }
}
