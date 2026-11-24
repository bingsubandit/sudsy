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
//!   a prefix; `only` (words, same form): the rule is about nodes whose first word is one of them;
//!   `block`: the tag of the block node.
//! - `token` (instead of `tag` and `kind`): the rule is about the comment tokens of that kind (a token
//!   kind name); `ignore` adds `inline` (code before it on its line) and `directive` (a comment whose
//!   text starts with the param `directive`).
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
    inline: bool,
    skip: Vec<String>,
    only: Vec<String>,
    token: Option<String>,
    directive: Option<String>,
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
        if ![
            "after-comment",
            "inside-single-line-block",
            "inline",
            "directive",
        ]
        .contains(&w.as_str())
        {
            return Err(format!("param `ignore`: unknown word `{w}`"));
        }
    }
    let rule = BlankLineBefore {
        meta,
        message: p.text("message")?,
        tags: [Box::leak(
            p.optional("tag").unwrap_or_default().into_boxed_str(),
        )],
        kind: p.optional("kind").unwrap_or_default(),
        block: p.text("block")?,
        multi_line,
        first_nested: has(&except, "first-nested"),
        after_same_kind: has(&except, "after-same-kind"),
        after_blockless_same_name: has(&except, "after-blockless-same-name"),
        after_comment: has(&ignore, "after-comment"),
        single_line_block: has(&ignore, "inside-single-line-block"),
        inline: has(&ignore, "inline"),
        skip: words(&mut p, "skip"),
        only: words(&mut p, "only"),
        token: p.optional("token"),
        directive: if has(&ignore, "directive") {
            Some(p.text("directive")?)
        } else {
            None
        },
    };
    if rule.token.is_none() && (rule.kind.is_empty() || rule.tags[0].is_empty()) {
        return Err("params `tag` and `kind` (or `token`) are missing".to_string());
    }
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
        let listed = |l: &[String]| {
            l.iter().any(|s| match s.strip_suffix('*') {
                Some(prefix) => word.to_ascii_lowercase().starts_with(prefix),
                None => word.eq_ignore_ascii_case(s),
            })
        };
        listed(&self.skip) || (!self.only.is_empty() && !listed(&self.only))
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

    fn bodies(&self) -> bool {
        false
    }

    fn tags(&self) -> &[&'static str] {
        if self.token.is_some() {
            &[]
        } else {
            &self.tags
        }
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
        self.check_at(&first, Some(node), block.is_some(), parent_block, ctx);
    }

    fn check_file(&self, file: &crate::rule::FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(token) = &self.token else {
            return;
        };
        let g = file.grammar;
        for t in file
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if g.name(t.kind()) != token {
                continue;
            }
            if self.directive.as_ref().is_some_and(|d| {
                g.comment_body(t.text())
                    .trim_start()
                    .starts_with(d.as_str())
            }) {
                continue;
            }
            // A comment before the first statement of a block hangs below that statement.
            let parent_block = t
                .parent_ancestors()
                .find(|p| g.has_tag(p.kind(), &self.block));
            self.check_at(&t, None, false, parent_block, ctx);
        }
    }
}

impl BlankLineBefore {
    /// The part of the check both modes share: `first` is the first word of the node, or the comment
    /// token.
    fn check_at(
        &self,
        first: &SyntaxToken,
        node: Option<&SyntaxNode>,
        has_block: bool,
        parent_block: Option<SyntaxNode>,
        ctx: &mut RuleContext<'_>,
    ) {
        let g = ctx.file.grammar;
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
        if self.inline && newlines == 0 && (previous_code.is_some() || comment) {
            return;
        }
        if previous_code.is_none() && !comment {
            return;
        }
        let first_nested = !comment
            && parent_block.is_some()
            && previous_code.as_ref().is_some_and(|t| t.text() == "{");
        let sibling = node.and_then(SyntaxNode::prev_sibling).filter(|_| !comment);
        let same = |s: &SyntaxNode| {
            kind_is(s, &self.kind, g) && !first_word(s, g).is_some_and(|w| self.skipped(w.text()))
        };
        let flip = (self.first_nested && first_nested)
            || (self.after_same_kind && sibling.as_ref().is_some_and(same))
            || (self.after_blockless_same_name
                && !has_block
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
