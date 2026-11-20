//! Declaration rows: which declarations a rule of a family looks at. A row is a grammar tag,
//! optionally `<parent-tag` (the parent has that tag: the node with the modifiers), then `&word`
//! for each modifier keyword the declaration (the parent, with a parent tag) must have and
//! `&@name` for each annotation it must carry (`*` matches any run of characters, so `&@*Test`
//! is `@Test` and `@ParameterizedTest`), then `=path` (a [`SlotPath`] to the name; the node's
//! `name` slot, or its `key` slot, by default). Rows are `|`-separated.
//!
//! concept: declaration-rows
//! what: Rows naming declarations by grammar tag, modifier keywords and annotations.
//! tools: Checkstyle name checks, graphql-eslint naming-convention
//! reads: tag:modifiers, tag:annotation

use crate::slot_path::SlotPath;
use core_syntax::{Grammar, SyntaxNode};

pub struct Row {
    pub option: String,
    pub tag: String,
    pub parent: Option<String>,
    pub modifiers: Vec<String>,
    pub annotations: Vec<String>,
    pub path: Option<SlotPath>,
}

impl Row {
    pub fn parse(option: &str, alt: &str) -> Result<Row, String> {
        let (alt, path) = match alt.split_once('=') {
            Some((a, p)) => (a, Some(SlotPath::parse(p)?)),
            None => (alt, None),
        };
        let mut parts = alt.split('&');
        let head = parts.next().unwrap_or_default();
        let (tag, parent) = match head.split_once('<') {
            Some((t, par)) => (t, Some(par.to_string())),
            None => (head, None),
        };
        let (annotations, modifiers): (Vec<&str>, Vec<&str>) =
            parts.partition(|p| p.starts_with('@'));
        Ok(Row {
            option: option.to_string(),
            tag: tag.to_string(),
            parent,
            modifiers: modifiers.into_iter().map(str::to_string).collect(),
            annotations: annotations
                .into_iter()
                .map(|a| a[1..].to_string())
                .collect(),
            path,
        })
    }

    /// How specific the row is: rows with more conditions are tried first.
    pub fn conditions(&self) -> usize {
        usize::from(self.parent.is_some()) + self.modifiers.len() + self.annotations.len()
    }

    pub fn matches(&self, g: &Grammar, node: &SyntaxNode) -> bool {
        if !g.has_tag(node.kind(), &self.tag) {
            return false;
        }
        let holder = match &self.parent {
            Some(tag) => match node.parent().filter(|p| g.has_tag(p.kind(), tag)) {
                Some(p) => p,
                None => return false,
            },
            None => node.clone(),
        };
        self.modifiers.iter().all(|m| has_modifier(g, &holder, m))
            && self
                .annotations
                .iter()
                .all(|a| annotation_names(g, &holder).iter().any(|n| glob(a, n)))
    }
}

/// Whether `node` has a `modifiers` child with the keyword `word`.
pub fn has_modifier(g: &Grammar, node: &SyntaxNode, word: &str) -> bool {
    node.children()
        .filter(|c| g.has_tag(c.kind(), "modifiers"))
        .any(|m| {
            m.children_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| t.text() == word)
        })
}

/// The names of the annotations of `node`'s `modifiers` (`Test` for `@Test`, `org.junit.Test`
/// for `@org.junit.Test(x)`): the dotted name after the `@`.
pub fn annotation_names(g: &Grammar, node: &SyntaxNode) -> Vec<String> {
    let mut names = Vec::new();
    for m in node.children().filter(|c| g.has_tag(c.kind(), "modifiers")) {
        for a in m.children().filter(|c| g.has_tag(c.kind(), "annotation")) {
            let mut name = String::new();
            let tokens = a
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| !g.is_trivia(t.kind()));
            for t in tokens.skip_while(|t| t.text() != "@").skip(1) {
                if t.text() == "(" {
                    break;
                }
                name.push_str(t.text());
            }
            names.push(name);
        }
    }
    names
}

/// Whether `text` matches `pattern`, in which `*` stands for any run of characters.
pub fn glob(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let mut parts: Vec<&str> = parts.collect();
    let Some(last) = parts.pop() else {
        return rest.is_empty();
    };
    for part in parts {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}
