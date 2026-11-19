//! The `blank-line-before` family: whether a blank line stands before a statement. Params:
//! - `tag`: the tag of the statement nodes the rule looks at; `kind`: the node kind it is about
//!   (a kind name of the language);
//! - `always`: `true` wants a blank line before every such node, `multi-line` only before one whose
//!   block spans lines (`block` is the tag of its block node);
//! - `except` (words): `first-nested` (the first node of a block wants none instead),
//!   `after-same-kind` (one right after a node of its kind wants none), `after-blockless-same-name`
//!   (a node without block right after one without block and with the same first word wants none);
//! - `ignore` (words): `after-comment` (a comment right before), `inside-single-line-block`;
//! - `skip` (words): nodes whose first word is one of them are left out, an entry ending in `*` is
//!   a prefix; `block`: the tag of the block node.
//!
//! The first node of a file is never checked. The message has `{blank}`: `an empty line` or `no
//! empty line`.
//!
//! concept: blank-line-before-family
//! what: Whether a blank line stands before a statement of a kind, with the usual exceptions.
//! tools: stylelint rule-empty-line-before, at-rule-empty-line-before, declaration-empty-line-before
//! reads: family:blank-line-before, tag:comment, tag:newline

use crate::tree::significant_range;
use crate::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

pub const FAMILY: Family = Family {
    name: "blank-line-before",
    build,
};

struct BlankLineBefore {
    meta: &'static RuleMeta,
    message: String,
    tags: [&'static str; 1],
    kind: String,
    block: String,
    multi_line: bool,
    first_nested: bool,
    after_same_kind: bool,
    after_blockless_same_name: bool,
    after_comment: bool,
    single_line_block: bool,
    skip: Vec<String>,
}

fn words(p: &mut Params, key: &str) -> Vec<String> {
    p.optional(key)
        .map(|s| s.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let multi_line = p.choice("always", true, &[("true", false), ("multi-line", true)])?;
    let except = words(&mut p, "except");
    let ignore = words(&mut p, "ignore");
    let has = |l: &[String], w: &str| l.iter().any(|x| x == w);
    for w in &except {
        if ![
            "first-nested",
            "after-same-kind",
            "after-blockless-same-name",
        ]
        .contains(&w.as_str())
        {
            return Err(format!("param `except`: unknown word `{w}`"));
        }
    }
    for w in &ignore {
        if !["after-comment", "inside-single-line-block"].contains(&w.as_str()) {
            return Err(format!("param `ignore`: unknown word `{w}`"));
        }
    }
    let rule = BlankLineBefore {
        meta,
        message: p.text("message")?,
        tags: [Box::leak(p.text("tag")?.into_boxed_str())],
        kind: p.text("kind")?,
        block: p.text("block")?,
        multi_line,
        first_nested: has(&except, "first-nested"),
        after_same_kind: has(&except, "after-same-kind"),
        after_blockless_same_name: has(&except, "after-blockless-same-name"),
        after_comment: has(&ignore, "after-comment"),
        single_line_block: has(&ignore, "inside-single-line-block"),
        skip: words(&mut p, "skip"),
    };
    p.done()?;
    Ok(Box::new(rule))
}

fn is_trivia(t: &SyntaxToken, g: &Grammar) -> bool {
    g.has_tag(t.kind(), "trivia")
}

fn kind_is(node: &SyntaxNode, name: &str, g: &Grammar) -> bool {
    g.def(node.kind()).is_some_and(|d| d.name == name)
}

/// The text of the first token of `node` that is not trivia.
fn first_word(node: &SyntaxNode, g: &Grammar) -> Option<SyntaxToken> {
    let mut t = node.first_token();
    while let Some(x) = t {
        if !is_trivia(&x, g) {
            return Some(x);
        }
        t = x.next_token();
    }
    None
}

impl BlankLineBefore {
    fn skipped(&self, word: &str) -> bool {
        self.skip.iter().any(|s| match s.strip_suffix('*') {
            Some(prefix) => word.to_ascii_lowercase().starts_with(prefix),
            None => word.eq_ignore_ascii_case(s),
        })
    }

    fn has_block(&self, node: &SyntaxNode, g: &Grammar) -> Option<SyntaxNode> {
        node.children().find(|c| g.has_tag(c.kind(), &self.block))
    }

    fn multi_line_text(node: &SyntaxNode, g: &Grammar, text: &str) -> bool {
        text[significant_range(node, g)].contains(['\n', '\r'])
    }
}

impl Rule for BlankLineBefore {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn tags(&self) -> &[&'static str] {
        &self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        if !kind_is(node, &self.kind, g) {
            return;
        }
        let Some(first) = first_word(node, g) else {
            return;
        };
        if self.skipped(first.text()) {
            return;
        }
        let block = self.has_block(node, g);
        if self.multi_line
            && !block
                .as_ref()
                .is_some_and(|b| Self::multi_line_text(b, g, ctx.file.text))
        {
            return;
        }
        let parent_block = node.parent().filter(|p| g.has_tag(p.kind(), &self.block));
        if self.single_line_block
            && parent_block
                .as_ref()
                .is_some_and(|p| !Self::multi_line_text(p, g, ctx.file.text))
        {
            return;
        }
        // What stands between the node and the code before it.
        let mut newlines = 0;
        let mut comment = false;
        let mut before = first.prev_token();
        let mut previous_code = None;
        while let Some(t) = before {
            if g.has_tag(t.kind(), "comment") {
                comment = true;
                break;
            }
            if g.has_tag(t.kind(), "newline") {
                newlines += 1;
            } else if !is_trivia(&t, g) {
                previous_code = Some(t);
                break;
            }
            before = t.prev_token();
        }
        if comment && self.after_comment {
            return;
        }
        if previous_code.is_none() && !comment {
            return;
        }
        let first_nested = !comment
            && parent_block.is_some()
            && previous_code.as_ref().is_some_and(|t| t.text() == "{");
        let sibling = node.prev_sibling().filter(|_| !comment);
        let same = |s: &SyntaxNode| {
            kind_is(s, &self.kind, g) && !first_word(s, g).is_some_and(|w| self.skipped(w.text()))
        };
        let flip = (self.first_nested && first_nested)
            || (self.after_same_kind && sibling.as_ref().is_some_and(same))
            || (self.after_blockless_same_name
                && block.is_none()
                && sibling.as_ref().is_some_and(|s| {
                    same(s)
                        && self.has_block(s, g).is_none()
                        && first_word(s, g)
                            .is_some_and(|w| w.text().eq_ignore_ascii_case(first.text()))
                }));
        let wants_blank = !flip;
        if (newlines >= 2) != wants_blank {
            let blank = if wants_blank {
                "an empty line"
            } else {
                "no empty line"
            };
            ctx.report(first.text_range(), self.message.replace("{blank}", blank));
        }
    }
}
