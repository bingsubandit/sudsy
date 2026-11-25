//! The generic safety check: the output must parse and must have the
//! same tree as the input once trivia are ignored.
//!
//! Tokens tagged `#separator` are ignored as well. A formatter may legitimately add or remove a
//! trailing comma, and a missing separator elsewhere changes the item structure or fails the
//! re-parse, so nothing is lost by not comparing them.
//!
//! A node tagged `#transparent` (JavaScript's parenthesized expression) is not compared, nor are
//! its own tokens (the parentheses): a formatter may add or remove it.
//!
//! Tokens tagged `#optional` are ignored too: the formatter may add or remove them.
//!
//! Tokens and nodes tagged `#restyle` (YAML's scalars, CSS's numbers) may change their text, for
//! example from single to double quotes or by re-indenting continuation lines; they have to stay
//! where they are, and their [`NodeFormatter::value`] has to stay the same.
//! The subtree of a `#restyle` node is not compared token by token.
//!
//! The comment tokens (tagged `#comment` and `#trivia`) are compared as a multiset of their texts
//! (line breaks as LF, the lines of a multi-line comment trimmed, the end of a one-line comment
//! trimmed): a formatter may move a comment, not lose, duplicate or edit one.
//! The provider's [`NodeFormatter::extra_check`] verifies what is not per item.
//!
//! concept: format-safety-check
//! what: Output must parse and keep the tree once trivia are ignored.
//! tools: none (meaning preservation)
//! reads: grammar:language

use crate::FormatError;
use crate::context::NodeFormatter;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode};
use rowan::WalkEvent;
use std::borrow::Cow;

#[derive(Debug, PartialEq, Eq)]
enum Event<'a> {
    Enter(SyntaxKind),
    Leave(SyntaxKind),
    Token(SyntaxKind, &'a str),
    /// A `#restyle` token or node: only its position is compared here.
    Restyled,
}

/// What the comparison needs to know about a token kind, looked up once per kind.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Ignored,
    Comment,
    Restyle,
    Compared,
}

/// The significant events of a tree in document order, each with its byte offset. It walks the
/// green tree directly: building red nodes for millions of tokens would cost more than the
/// comparison itself.
struct Events<'a> {
    g: &'a Grammar,
    /// Open nodes: their kind and the children still to visit.
    stack: Vec<(SyntaxKind, rowan::Children<'a>, bool)>,
    /// Byte offset after the tokens seen so far.
    pos: usize,
    root: Option<&'a rowan::GreenNodeData>,
    /// The texts of the comments seen so far.
    comments: Vec<Cow<'a, str>>,
}

/// The text of a green node.
fn green_text(node: &rowan::GreenNodeData, out: &mut String) {
    for c in node.children() {
        match c {
            rowan::NodeOrToken::Node(n) => green_text(n, out),
            rowan::NodeOrToken::Token(t) => out.push_str(t.text()),
        }
    }
}

impl<'a> Events<'a> {
    fn new(g: &'a Grammar, root: &'a rowan::GreenNodeData) -> Self {
        Events {
            g,
            stack: Vec::new(),
            pos: 0,
            root: Some(root),
            comments: Vec::new(),
        }
    }

    /// The comments inside a node that is not walked (a `#restyle` node).
    fn collect_comments(&mut self, node: &'a rowan::GreenNodeData) {
        for c in node.children() {
            match c {
                rowan::NodeOrToken::Token(t) => {
                    let kind = SyntaxKind(t.kind().0);
                    if self.g.is_trivia(kind) && self.g.has_tag(kind, "comment") {
                        self.comments.push(t.text().into());
                    }
                }
                rowan::NodeOrToken::Node(n) => {
                    let kind = SyntaxKind(n.kind().0);
                    if self.g.is_trivia(kind) && self.g.has_tag(kind, "comment") {
                        let mut text = String::new();
                        green_text(n, &mut text);
                        self.comments.push(text.into());
                    } else {
                        self.collect_comments(n);
                    }
                }
            }
        }
    }

    fn enter(&mut self, node: &'a rowan::GreenNodeData) -> (usize, Event<'a>) {
        let kind = SyntaxKind(node.kind().0);
        let transparent = self.g.has_tag(kind, "transparent");
        self.stack.push((kind, node.children(), transparent));
        (self.pos, Event::Enter(kind))
    }

    fn class(&self, kind: SyntaxKind) -> Class {
        let g = self.g;
        if g.is_trivia(kind) {
            if g.has_tag(kind, "comment") {
                Class::Comment
            } else {
                Class::Ignored
            }
        } else if g.has_tag(kind, "separator") || g.has_tag(kind, "optional") {
            Class::Ignored
        } else if g.has_tag(kind, "restyle") {
            Class::Restyle
        } else {
            Class::Compared
        }
    }
}

impl<'a> Iterator for Events<'a> {
    type Item = (usize, Event<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(root) = self.root.take() {
            return Some(self.enter(root));
        }
        loop {
            let (kind, children, transparent) = self.stack.last_mut()?;
            let transparent = *transparent;
            match children.next() {
                None => {
                    let kind = *kind;
                    self.stack.pop();
                    if transparent {
                        continue;
                    }
                    return Some((self.pos, Event::Leave(kind)));
                }
                Some(rowan::NodeOrToken::Node(n)) => {
                    let kind = SyntaxKind(n.kind().0);
                    // A structured trivia (a C# doc comment, a directive) is trivia as a whole.
                    if self.g.is_trivia(kind) {
                        self.pos += usize::from(n.text_len());
                        if self.g.has_tag(kind, "comment") {
                            let mut text = String::new();
                            green_text(n, &mut text);
                            self.comments.push(text.into());
                        }
                        continue;
                    }
                    if self.g.has_tag(kind, "restyle") {
                        let at = self.pos;
                        self.pos += usize::from(n.text_len());
                        self.collect_comments(n);
                        return Some((at, Event::Restyled));
                    }
                    let entered = self.enter(n);
                    if self.stack.last().is_some_and(|f| f.2) {
                        continue;
                    }
                    return Some(entered);
                }
                Some(rowan::NodeOrToken::Token(t)) => {
                    let kind = SyntaxKind(t.kind().0);
                    let at = self.pos;
                    self.pos += t.text().len();
                    let class = self.class(kind);
                    if class == Class::Comment {
                        self.comments.push(t.text().into());
                    }
                    if transparent {
                        continue;
                    }
                    match class {
                        Class::Ignored | Class::Comment => {}
                        Class::Restyle => return Some((at, Event::Restyled)),
                        Class::Compared => return Some((at, Event::Token(kind, t.text()))),
                    }
                }
            }
        }
    }
}

/// `Ok` when both trees have the same nodes and non-trivia tokens in the same order and the same
/// comments; else a description of the first difference. The value is the number of `#restyle`
/// items.
fn compare(
    g: &Grammar,
    a: &SyntaxNode,
    b: &SyntaxNode,
    words: bool,
    as_line: bool,
    cut_blanks: bool,
) -> Result<usize, String> {
    let (a, b) = (a.green(), b.green());
    let (mut a, mut b) = (Events::new(g, &a), Events::new(g, &b));
    let mut restyled = 0;
    loop {
        match (a.next(), b.next()) {
            (None, None) => break,
            (Some((_, x)), Some((_, y))) if x == y => restyled += usize::from(x == Event::Restyled),
            // The printer writes LF only inside a token that spans lines (a template chunk).
            (Some((_, Event::Token(k, x))), Some((_, Event::Token(l, y))))
                if k == l && x.replace("\r\n", "\n") == y => {}
            (x, y) => {
                return Err(format!(
                    "the trees differ: input has {}, output has {}",
                    describe(g, x),
                    describe(g, y)
                ));
            }
        }
    }
    let sorted = |v: Vec<Cow<'_, str>>| {
        let mut keys: Vec<String> = Vec::new();
        for c in &v {
            match c.strip_prefix("//") {
                // (a reflowed `//` comment: its words, wherever the lines break)
                Some(rest) if words => {
                    keys.extend(rest.split_whitespace().map(|w| format!("//{w}")))
                }
                _ => keys.push(comment_key(g, c, as_line, cut_blanks)),
            }
        }
        keys.sort_unstable();
        keys
    };
    let (c, d) = (sorted(a.comments), sorted(b.comments));
    match (0..c.len().max(d.len())).find(|&i| c.get(i) != d.get(i)) {
        None => Ok(restyled),
        Some(i) => Err(format!(
            "a comment was lost, duplicated or edited: input has {:?}, output has {:?}",
            c.get(i),
            d.get(i)
        )),
    }
}

/// A comment without what the printer changes: it writes LF only and re-indents the lines of a
/// multi-line comment; the end of a one-line comment is trimmed. With `cut_blanks` the printer
/// may cut the runs of blank lines inside a comment (`NodeFormatter::cuts_blank_lines_in_comments`).
fn comment_key(g: &Grammar, text: &str, as_line: bool, cut_blanks: bool) -> String {
    let text = text.replace("\r\n", "\n");
    // (`single_block_comment_as_line`: a one-line block comment may be written as a line comment)
    if as_line && let Some(line) = line_form(g, &text) {
        return line;
    }
    if text.contains('\n') {
        trimmed_lines(&text, cut_blanks)
    } else {
        text.trim_end().to_string()
    }
}

/// The lines of a multi-line comment trimmed; with `cut_blanks` a run of blank lines counts as
/// one.
pub(crate) fn trimmed_lines(text: &str, cut_blanks: bool) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if !(cut_blanks && line.is_empty() && lines.last().is_some_and(|l| l.is_empty())) {
            lines.push(line);
        }
    }
    lines.join("\n")
}

/// The line comment a one-line block comment (not a documentation comment, not empty) is written
/// as under `single_block_comment_as_line`.
pub(crate) fn line_form(g: &Grammar, text: &str) -> Option<String> {
    let (open, close) = g.comments.block.as_ref()?;
    let marker = g.comments.line.as_deref()?;
    let inner = text
        .strip_prefix(open.as_str())?
        .strip_suffix(close.as_str())?;
    let doc = open.chars().last()?;
    (!inner.contains(['\n', '\r']) && !inner.starts_with(doc) && !inner.trim().is_empty())
        .then(|| format!("{marker} {}", inner.trim()))
}

/// [`compare`] without the values of the `#restyle` items.
pub fn equal_ignoring_trivia(g: &Grammar, a: &SyntaxNode, b: &SyntaxNode) -> Result<(), String> {
    equal_with_line_comments(g, a, b, false, false)
}

/// [`equal_ignoring_trivia`]; with `as_line` a one-line block comment may be a line comment
/// (`single_block_comment_as_line`), with `cut_blanks` a comment may lose blank lines.
pub fn equal_with_line_comments(
    g: &Grammar,
    a: &SyntaxNode,
    b: &SyntaxNode,
    as_line: bool,
    cut_blanks: bool,
) -> Result<(), String> {
    compare(g, a, b, false, as_line, cut_blanks).map(drop)
}

/// The `#restyle` items of a tree in document order, as `compare` counts them.
fn restyled(g: &Grammar, root: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut out = Vec::new();
    let mut walk = root.preorder_with_tokens();
    while let Some(event) = walk.next() {
        let WalkEvent::Enter(e) = event else { continue };
        let kind = e.kind();
        if g.is_trivia(kind) {
            walk.skip_subtree();
        } else if g.has_tag(kind, "restyle") {
            let in_transparent = e
                .as_token()
                .and_then(|t| t.parent())
                .is_some_and(|p| g.has_tag(p.kind(), "transparent"));
            if !in_transparent && !g.has_tag(kind, "separator") && !g.has_tag(kind, "optional") {
                out.push(e);
            }
            walk.skip_subtree();
        }
    }
    out
}

fn same_values(
    g: &Grammar,
    f: &dyn NodeFormatter,
    options: &core_config::ResolvedOptions,
    a: &SyntaxNode,
    b: &SyntaxNode,
) -> Result<(), String> {
    let (a, b) = (restyled(g, a), restyled(g, b));
    // (a raw string that a call strips the indentation of may be re-indented: see `raw_string`)
    let value = |e: &SyntaxElement| -> String {
        e.as_node()
            .and_then(|n| crate::shapes::raw_string::value(options, g, n))
            .unwrap_or_else(|| f.value(e).into_owned())
    };
    for (x, y) in a.iter().zip(&b) {
        let (vx, vy) = (value(x), value(y));
        if vx != vy {
            return Err(format!(
                "{} `{vx}` became {} `{vy}`",
                g.name(x.kind()),
                g.name(y.kind())
            ));
        }
    }
    Ok(())
}

fn describe(g: &Grammar, ev: Option<(usize, Event<'_>)>) -> String {
    match ev {
        None => "nothing more".into(),
        Some((at, Event::Enter(k))) => format!("start of {} at byte {at}", g.name(k)),
        Some((at, Event::Leave(k))) => format!("end of {} at byte {at}", g.name(k)),
        Some((at, Event::Token(k, text))) => {
            format!("{} `{text}` at byte {at}", g.name(k))
        }
        Some((at, Event::Restyled)) => format!("a restyled token at byte {at}"),
    }
}

/// The kinds of markup tags, for [`same_tags`].
pub struct TagKinds<'a> {
    /// Nodes that start an element.
    pub start: &'a [SyntaxKind],
    /// Nodes that end one (also an end tag without a start).
    pub end: &'a [SyntaxKind],
    /// The token of a tag node that holds its name.
    pub name: SyntaxKind,
    /// Names compare in lowercase.
    pub ignore_case: bool,
}

/// The start and end tags of both trees are the same, tag by tag with their names. A grammar that
/// tags its end tag node `#transparent` (an element may omit it) leaves it out of the generic
/// comparison; this one refuses an end tag that was added or dropped.
pub fn same_tags(before: &SyntaxNode, after: &SyntaxNode, kinds: &TagKinds) -> Result<(), String> {
    let sequence = |tree: &SyntaxNode| -> Vec<(bool, String)> {
        tree.descendants()
            .filter_map(|n| {
                let start = kinds.start.contains(&n.kind());
                if !start && !kinds.end.contains(&n.kind()) {
                    return None;
                }
                let name = n
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == kinds.name)?;
                let name = if kinds.ignore_case {
                    name.text().to_ascii_lowercase()
                } else {
                    name.text().to_string()
                };
                Some((start, name))
            })
            .collect()
    };
    let (a, b) = (sequence(before), sequence(after));
    match a.iter().zip(&b).position(|(x, y)| x != y) {
        None if a.len() == b.len() => Ok(()),
        at => Err(format!(
            "the start and end tags changed at tag {}",
            at.unwrap_or(a.len().min(b.len())) + 1
        )),
    }
}

/// Runs the generic check and then the provider's own. `reparse` parses the output and returns
/// an error message if it has parse errors.
pub fn verify(
    g: &Grammar,
    formatter: &dyn NodeFormatter,
    options: &core_config::ResolvedOptions,
    before_tree: &SyntaxNode,
    after: &str,
    reparse: impl FnOnce(&str) -> Result<SyntaxNode, String>,
) -> Result<(), FormatError> {
    verify_with(g, formatter, options, before_tree, after, reparse, None)
}

/// [`verify`] for a host whose code of other languages `embedder` compares.
pub fn verify_with(
    g: &Grammar,
    formatter: &dyn NodeFormatter,
    options: &core_config::ResolvedOptions,
    before_tree: &SyntaxNode,
    after: &str,
    reparse: impl FnOnce(&str) -> Result<SyntaxNode, String>,
    embedder: Option<&dyn crate::Embedder>,
) -> Result<(), FormatError> {
    let after_tree = reparse(after)
        .map_err(|e| FormatError::Safety(format!("the output does not parse: {e}")))?;
    let as_line = options.get(core_config::opt::SINGLE_BLOCK_COMMENT_AS_LINE)
        == Some(&core_config::OptionValue::Bool(true));
    if compare(
        g,
        before_tree,
        &after_tree,
        formatter.reflows_comments(),
        as_line,
        formatter.cuts_blank_lines_in_comments(),
    )
    .map_err(FormatError::Safety)?
        > 0
    {
        same_values(g, formatter, options, before_tree, &after_tree)
            .map_err(FormatError::Safety)?;
    }
    if let Some(spec) = formatter.embedded_text() {
        embedded_same(&spec, before_tree, &after_tree, options, embedder)
            .map_err(FormatError::Safety)?;
    }
    formatter
        .extra_check_with(before_tree, &after_tree, options)
        .map_err(FormatError::Safety)
}

/// The tokens of `spec.kind` of both trees, compared as code of `spec.language`: the same count
/// (a lost or added text is a change), and each pair read as the same code when its text differs.
fn embedded_same(
    spec: &crate::context::EmbeddedText,
    before: &SyntaxNode,
    after: &SyntaxNode,
    options: &core_config::ResolvedOptions,
    embedder: Option<&dyn crate::Embedder>,
) -> Result<(), String> {
    let texts = |tree: &SyntaxNode| {
        tree.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == spec.kind)
            .map(|t| (spec.read)(t.text()))
            .collect::<Vec<_>>()
    };
    let (x, y) = (texts(before), texts(after));
    if x.len() != y.len() {
        return Err(format!(
            "{} {} texts became {}",
            x.len(),
            spec.language,
            y.len()
        ));
    }
    for (x, y) in x.iter().zip(&y) {
        if x == y {
            continue;
        }
        let embedder = embedder.ok_or_else(|| {
            format!(
                "the {} text changed without a {} formatter",
                spec.language, spec.language
            )
        })?;
        let forward = crate::Forward {
            host: options,
            ids: spec.forward,
        };
        embedder
            .same_code(spec.language, spec.mode, (x, y), spec.size, forward)
            .map_err(|e| format!("{} text: {e}", spec.language))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::grammars;

    fn key(text: &str, as_line: bool) -> String {
        comment_key(grammars::csharp(), text, as_line, false)
    }

    #[test]
    fn a_cut_run_of_blank_lines_is_an_edit_unless_the_printer_cuts_them() {
        let (a, b) = ("/* a\n\n\n b */", "/* a\n\n b */");
        let key = |t, cut| comment_key(grammars::csharp(), t, false, cut);
        assert_ne!(key(a, false), key(b, false));
        assert_eq!(key(a, true), key(b, true));
        // (a removed text line is an edit either way)
        assert_ne!(key(a, true), key("/* a\n\n */", true));
    }

    #[test]
    fn a_one_line_block_comment_equals_its_line_form_only_under_the_option() {
        assert_eq!(key("/* a b */", true), key("// a b", true));
        assert_eq!(key("/*a*/", true), key("// a", true));
        assert_ne!(key("/* a */", false), key("// a", false));
    }

    #[test]
    fn other_changes_of_a_comment_stay_changes() {
        // (other words, a documentation comment, a multi-line or an empty comment)
        assert_ne!(key("/* a */", true), key("// b", true));
        assert_ne!(key("/** a */", true), key("// a", true));
        assert_ne!(key("/* a\n b */", true), key("// a b", true));
        assert_ne!(key("/**/", true), key("//", true));
        assert_ne!(key("/* a */", true), key("// a // b", true));
        // (a line comment never equals a different block comment)
        assert_ne!(key("// a", true), key("/* b */", true));
    }

    #[test]
    fn a_lost_or_added_embedded_text_is_refused_by_count() {
        use core_syntax::TreeBuilder;
        let g = grammars::json();
        let tree = |n: usize| {
            let mut b = TreeBuilder::new();
            b.start_node(g.kind_by_name("Document").unwrap());
            for _ in 0..n {
                b.token(g.kind_by_name("Number").unwrap(), "1");
            }
            b.token(g.kind_by_name("Eof").unwrap(), "");
            b.finish_node();
            SyntaxNode::new_root(b.finish())
        };
        let spec = crate::context::EmbeddedText {
            kind: g.kind_by_name("Number").unwrap(),
            language: "x",
            mode: crate::EmbedMode::Continued,
            forward: &[],
            read: str::to_string,
            size: (80, false),
        };
        let options = core_config::ResolvedOptions::default();
        assert!(embedded_same(&spec, &tree(2), &tree(2), &options, None).is_ok());
        let lost = embedded_same(&spec, &tree(2), &tree(1), &options, None);
        assert!(lost.unwrap_err().contains("2 x texts became 1"));
        assert!(embedded_same(&spec, &tree(1), &tree(2), &options, None).is_err());
    }
}
