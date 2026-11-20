//! The context formatting code runs in, and the trait providers implement.
//!
//! concept: none (infrastructure: formatting context and the NodeFormatter trait)

use crate::comments::{self, Comment, CommentKind, CommentStyle};
use crate::directives::{Directive, DirectiveTable, NodeDirectives};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::ignore::{self, IgnoreMark};
use crate::lines::LineBreaks;
use crate::shapes::{self, BlockStyle, KeyValueStyle, ListStyle};
use core_config::ResolvedOptions;
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange, TextSize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;

/// What an embedded region holds (see [`Embedder`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EmbedMode {
    /// A whole file of the language.
    Program,
    /// One expression (a Vue directive, `{{ }}`, a Svelte `{...}`).
    Expression,
    /// One expression of an Angular template: as `Expression`, and call arguments take no trailing
    /// comma (Angular's expression syntax has none).
    NgExpression,
    /// One expression of an Angular template that is not the whole value (an interpolation, a
    /// statement, a block's parameter): as `NgExpression`, and a binary's operands are indented.
    NgNested,
    /// A list of declarations (`style=""`).
    Declarations,
}

impl EmbedMode {
    /// The name the other language's provider reads in its `embed_mode` option.
    pub fn name(self) -> &'static str {
        match self {
            EmbedMode::Program => "program",
            EmbedMode::Expression => "expression",
            EmbedMode::NgExpression => "ng-expression",
            EmbedMode::NgNested => "ng-nested",
            EmbedMode::Declarations => "declarations",
        }
    }
}

/// Formats code of another language for a host formatter (`<script>` in HTML): the engine
/// parses `code` with the provider of `language`, formats it for `width` columns and returns the
/// text without a final line break, or `None` when the code does not parse or the language is
/// unknown. `single_quotes`: the host's own quote is `"` (an attribute value), so strings prefer `'`.
pub trait Embedder {
    fn format(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        width: usize,
        single_quotes: bool,
    ) -> Option<String>;
}

/// A provider's formatting code. Nodes it does not handle fall back to the shared shape of their
/// grammar tag (`#block`, `#delimited-list`, `#key-value`), and else to their children
/// concatenated.
pub trait NodeFormatter: Send + Sync {
    /// Layout for `node`, or `None` to use the tag default. Per-node code builds a style from
    /// `cx.options` and calls a shape; hooks that try several layouts use `conditional_group`.
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError>;

    /// A provider-specific addition to the generic safety check (YAML compares the loaded data
    /// model here). `before` and `after` are the trees of the source and of the formatted
    /// output, both already parsed without errors.
    fn extra_check(&self, _before: &SyntaxNode, _after: &SyntaxNode) -> Result<(), String> {
        Ok(())
    }

    /// What the safety check compares of a `#restyle` token or node of the input and of the
    /// output: the text without what formatting may change (quotes, case, number spelling, blanks).
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => Cow::Borrowed(t.text()),
            // (the code of a node: formatting moves the comments around it)
            SyntaxElement::Node(n) => {
                let toks: Vec<_> = n
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .collect();
                let code = |t: &&core_syntax::SyntaxToken| {
                    let s = t.text();
                    !s.trim().is_empty() && !s.starts_with("//") && !s.starts_with("/*")
                };
                let from = toks.iter().position(|t| code(&t)).unwrap_or(0);
                let to = toks.iter().rposition(|t| code(&t)).map_or(0, |i| i + 1);
                let text: String = toks[from..to].iter().map(|t| t.text()).collect();
                Cow::Owned(text)
            }
        }
    }

    /// Source text to format instead of the input, for an option that reorders code (C# sorts
    /// `using` directives): the input tree is parsed from it again, and the safety check compares
    /// the output with that tree. `None` (the default) formats the input as it is.
    /// How often [`NodeFormatter::rewrite_source`] may run on its own result: a rewrite that
    /// unlocks the next (`((T))` loses one pair of parentheses per round) needs more than one.
    fn rewrite_rounds(&self) -> usize {
        1
    }

    /// The text of the language's `format.rules`: the rows of the rules that run on the source
    /// before layout (`core_rules::Rule::formats`) are in it. Empty: the language has none.
    fn rows(&self) -> &'static str {
        ""
    }

    /// The `[rewrite]` rows of the language, for a formatter that lets the shared format loop
    /// scan them in its one walk of the nodes ([`NodeFormatter::rewrite_scanned`]).
    fn rewrite_rows(&self) -> Option<&crate::rewrite::RewriteRules> {
        None
    }

    /// [`NodeFormatter::rewrite_source`] with the scan of [`NodeFormatter::rewrite_rows`] already
    /// made (`None` when it has no rows); the default ignores it.
    fn rewrite_scanned(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &Grammar,
        options: &ResolvedOptions,
        _scan: Option<crate::rewrite::RewriteScan<'_>>,
    ) -> Option<String> {
        self.rewrite_source(tree, text, grammar, options)
    }

    fn rewrite_source(
        &self,
        _tree: &SyntaxNode,
        _text: &str,
        _grammar: &Grammar,
        _options: &ResolvedOptions,
    ) -> Option<String> {
        None
    }

    /// Formatted text of the code the input's own parse leaves out (C# `#if` branches that are
    /// inactive under the file's symbols): `output` is the input formatted from `tree`;
    /// `variants` parses and formats the same text under other symbol sets. Returns the output
    /// with those parts replaced by their formatted text, and the symbol sets whose parses it
    /// took text from. The caller checks the result against the tree of every one of them and
    /// falls back to `output` on any doubt. `None` (the default) means there is nothing to merge.
    fn merge_variants(
        &self,
        _tree: &SyntaxNode,
        _output: &str,
        _options: &ResolvedOptions,
        _variants: &mut dyn Variants,
    ) -> Option<(String, Vec<Vec<String>>)> {
        None
    }

    /// Does the formatter re-space and wrap `//` comments (`CommentStyle::reflow_width`)? The safety
    /// check then compares their words, not their texts.
    fn reflows_comments(&self) -> bool {
        false
    }

    /// How comments are printed; the default keeps their text and puts one space before a
    /// comment after code.
    fn comment_style(&self, options: &ResolvedOptions) -> CommentStyle {
        CommentStyle {
            trim_lines: options.get("comment_trim_lines")
                == Some(&core_config::OptionValue::Bool(true)),
            reindent: options.get("comment_reindent_lines")
                == Some(&core_config::OptionValue::Bool(true)),
            ..CommentStyle::default()
        }
    }

    /// Whether the end of the file is text the formatter must not change (PHP: inline HTML after
    /// the last `?>`): the final newline is not added then. The formatter writes that text with
    /// [`doc::verbatim`], which keeps its trailing blanks.
    fn exact_end(&self, _root: &SyntaxNode) -> bool {
        false
    }

    /// Must a blank line stand between these two statements although the source has none (the
    /// language's `BlankRule`)?
    fn blank_between(&self, _cx: &FormatCtx<'_>, _prev: &SyntaxNode, _next: &SyntaxNode) -> bool {
        false
    }
}

/// Other parses of the text being formatted, under other symbol sets (see
/// [`NodeFormatter::merge_variants`]). Parses are memoized by symbol set.
pub trait Variants {
    /// The tree under `symbols`, or `None` when it has syntax errors.
    fn parse(&mut self, symbols: &[String]) -> Option<SyntaxNode>;
    /// `tree` (from [`Variants::parse`]) formatted; `None` when formatting fails.
    fn format(&mut self, tree: &SyntaxNode) -> Option<String>;
}

/// For providers that have no formatting code yet: everything uses tag defaults.
pub struct DefaultFormatter;

impl NodeFormatter for DefaultFormatter {
    fn format_node(&self, _: &SyntaxNode, _: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
        Ok(None)
    }
}

/// Everything a formatting decision may read. Any node of a tree can be formatted with
/// [`FormatCtx::format`]: nothing depends on the root, so a subtree is formatted in its context
/// (its comments and neighbours are read from the tree around it).
pub struct FormatCtx<'a> {
    pub grammar: &'a Grammar,
    pub options: &'a ResolvedOptions,
    pub directives: &'a DirectiveTable,
    formatter: &'a dyn NodeFormatter,
    pub comment_style: CommentStyle,
    /// The line breaks of the tree being formatted, found when first asked for.
    line_breaks: Mutex<Option<LineBreaks>>,
    /// Tokens whose trailing comments a shape prints itself, with the comments once the token
    /// was printed.
    detached: Mutex<Vec<(TextSize, Option<Vec<Comment>>)>>,
    /// Like `detached` for the whole run: the comments are read, not taken, so a document that
    /// was cached while they were held back stays right when it is used again.
    held: Mutex<HashMap<TextSize, Option<Vec<Comment>>>>,
    /// Tokens whose leading comments a shape prints itself (see [`FormatCtx::hold_leading`]).
    held_leading: Mutex<std::collections::HashSet<TextSize>>,
    /// How many leading comments of a token the layouts before it have printed (`placement`).
    pub(crate) consumed: Mutex<HashMap<TextSize, usize>>,
    /// Is any token in `held_leading`? Asked for every token, so without the lock.
    any_held_leading: std::sync::atomic::AtomicBool,
    /// A provider's per-run data derived from the options (see [`FormatCtx::cached`]).
    cache: std::sync::OnceLock<Box<dyn std::any::Any + Send + Sync>>,
    /// The widths for `align_comments` (see [`crate::align`]), measured when first asked for.
    comment_align: std::sync::OnceLock<crate::align::AlignTable>,
    /// Documents of nodes formatted for a layout, by node and flags (see [`FormatCtx::layout`]).
    layouts: Mutex<HashMap<(TextRange, u32, u32), Doc>>,
    embedder: Option<&'a dyn Embedder>,
    /// The option `max_blank_lines`, read once (all the blank lines of the source without it).
    pub(crate) max_blank_lines: usize,
}

impl<'a> FormatCtx<'a> {
    pub fn new(
        grammar: &'a Grammar,
        options: &'a ResolvedOptions,
        directives: &'a DirectiveTable,
        formatter: &'a dyn NodeFormatter,
    ) -> Self {
        FormatCtx {
            grammar,
            options,
            directives,
            formatter,
            comment_style: formatter.comment_style(options),
            max_blank_lines: match options.get("max_blank_lines") {
                Some(core_config::OptionValue::Int(n)) => usize::try_from(*n).unwrap_or(usize::MAX),
                _ => usize::MAX,
            },
            line_breaks: Mutex::new(None),
            detached: Mutex::new(Vec::new()),
            held: Mutex::new(HashMap::new()),
            held_leading: Mutex::new(std::collections::HashSet::new()),
            consumed: Mutex::new(HashMap::new()),
            any_held_leading: std::sync::atomic::AtomicBool::new(false),
            cache: std::sync::OnceLock::new(),
            comment_align: std::sync::OnceLock::new(),
            layouts: Mutex::new(HashMap::new()),
            embedder: None,
        }
    }

    /// A context of the same run that pads no comments: the entries are measured in it.
    pub(crate) fn measuring(&self) -> FormatCtx<'a> {
        let cx = FormatCtx::new(self.grammar, self.options, self.directives, self.formatter);
        let _ = cx.comment_align.set(Default::default());
        cx
    }

    /// Extra blanks in front of the trailing comment of a `#key-value` entry or `#statement`
    /// (`align_comments`); the first one asked for decides which of the two the runs are of.
    pub(crate) fn comment_pad(&self, entry: &SyntaxNode) -> usize {
        let Some(tag) = crate::align::run_tag(self.grammar, entry.kind()) else {
            return 0;
        };
        self.comment_align
            .get_or_init(|| crate::align::AlignTable::build(self, entry, tag))
            .extra(entry)
    }

    /// Pads the trailing comment in `d`, the layout of `entry`, to the column of its run.
    pub fn align_trailing_comment(&self, entry: &SyntaxNode, d: &mut Doc) {
        let extra = self.comment_pad(entry);
        if extra > 0 {
            crate::align::pad_comment(d, extra);
        }
    }

    /// The engine's formatter of other languages' code, when the host runs under one.
    pub fn with_embedder(mut self, embedder: Option<&'a dyn Embedder>) -> Self {
        self.embedder = embedder;
        self
    }

    /// `code` of `language` formatted for `width` columns (see [`Embedder`]).
    pub fn embed(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        width: usize,
        single_quotes: bool,
    ) -> Option<String> {
        self.embedder?
            .format(language, mode, code, width, single_quotes)
    }

    /// Data the provider derives from the options once per run instead of once per node. A
    /// context serves one provider, so `T` is always the same type.
    pub fn cached<T: std::any::Any + Send + Sync>(&self, make: impl FnOnce() -> T) -> &T {
        self.cache
            .get_or_init(|| Box::new(make()))
            .downcast_ref::<T>()
            .expect("one cached type per FormatCtx")
    }

    /// The document `make` builds for `node` under `flags` (a provider's own numbering of the
    /// layouts it tries), built once: a layout that prints a child in two candidates (the last
    /// call argument hugged or not) reuses the child's document instead of formatting it again,
    /// which would take 2^depth runs for nested callbacks. The result is [`doc::shared`], so
    /// clones cost nothing.
    pub fn layout(
        &self,
        node: &SyntaxNode,
        flags: u32,
        make: impl FnOnce() -> Result<Doc, FormatError>,
    ) -> Result<Doc, FormatError> {
        let key = (node.text_range(), node.kind().0, flags);
        if let Some(d) = self
            .layouts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
        {
            return Ok(d.clone());
        }
        let d = doc::shared(make()?);
        self.layouts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, d.clone());
        Ok(d)
    }

    /// Does the source contain a line break (`\n` or `\r`) within `range`, which is given in the
    /// offsets of the tree `node` is in?
    pub fn has_line_break(&self, node: &SyntaxNode, range: TextRange) -> bool {
        let root = node.ancestors().last().unwrap_or_else(|| node.clone());
        let mut cached = self.line_breaks.lock().unwrap_or_else(|e| e.into_inner());
        if !cached.as_ref().is_some_and(|l| l.is_of(&root)) {
            *cached = Some(LineBreaks::of(&root));
        }
        cached.as_ref().is_some_and(|l| l.any_in(range))
    }

    pub fn directives_of(&self, node: &SyntaxNode) -> NodeDirectives<'a> {
        self.directives.of(node)
    }

    /// Formats a child node. Honors `sudsy-ignore` comments before the node and the `Preserve`
    /// directive; use [`FormatCtx::format_unguarded`] for the node a caller was invoked for.
    pub fn format(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        if self.is_ignored(node) || self.directives_of(node).has(&Directive::Preserve) {
            return Ok(self.verbatim_node(node));
        }
        self.format_unguarded(node)
    }

    /// [`FormatCtx::format`] for a caller that already knows whether a comment stands in front of
    /// `node` (without one nothing can ask for it to be kept as written).
    pub fn format_known(&self, node: &SyntaxNode, commented: bool) -> Result<Doc, FormatError> {
        if (commented && self.is_ignored(node))
            || (!self.directives.is_empty() && self.directives_of(node).has(&Directive::Preserve))
        {
            return Ok(self.verbatim_node(node));
        }
        self.format_unguarded(node)
    }

    /// Does a `sudsy-ignore` comment in front of `node` keep it as written?
    pub fn is_ignored(&self, node: &SyntaxNode) -> bool {
        // Only a comment can ask for the node to be kept as written.
        comments::node_leading(self.grammar, node).has_comment
            && self.first_token(node).is_some_and(|first| {
                let leading = comments::leading_comments(self.grammar, &first);
                ignore::mark_in(self.grammar, &leading) == Some(IgnoreMark::Ignore)
            })
    }

    /// Formatting code for `node` itself, ignoring ignore comments and `Preserve`.
    pub fn exact_end(&self, root: &SyntaxNode) -> bool {
        self.formatter.exact_end(root)
    }

    /// See [`NodeFormatter::blank_between`].
    pub fn blank_between(&self, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
        self.formatter.blank_between(self, prev, next)
    }

    pub fn format_unguarded(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        crate::deep(|| match self.formatter.format_node(node, self)? {
            Some(doc) => Ok(doc),
            None => self.format_default(node),
        })
    }

    /// The shared shape for the node's grammar tag.
    pub fn format_default(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        let g = self.grammar;
        let has = |tag| g.def(node.kind()).is_some_and(|d| d.has_tag(tag));
        if has("delimited-list") {
            shapes::delimited_list(self, node, &ListStyle::default())
        } else if has("block") {
            shapes::block(self, node, &BlockStyle::default())
        } else if has("key-value") {
            shapes::key_value(self, node, &KeyValueStyle::default())
        } else {
            self.concat_children(node)
        }
    }

    /// Children printed one after the other, with no spacing.
    pub fn concat_children(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        // The first part waits in `single`: most nodes have just one, and need no list.
        let mut single = None;
        let mut parts = Vec::new();
        let significant = node
            .children_with_tokens()
            .by_kind(|k| !self.grammar.is_trivia(k));
        for e in significant {
            let part = match e {
                SyntaxElement::Node(n) => self.format(&n)?,
                SyntaxElement::Token(t) => self.token(&t),
            };
            match single.take() {
                None if parts.is_empty() => single = Some(part),
                None => parts.push(part),
                Some(first) => parts.extend([first, part]),
            }
        }
        Ok(single.unwrap_or_else(|| doc::concat(parts)))
    }

    pub fn first_token(&self, node: &SyntaxNode) -> Option<SyntaxToken> {
        let mut walk = node.preorder_with_tokens();
        while let Some(event) = walk.next() {
            match event {
                rowan::WalkEvent::Enter(SyntaxElement::Node(n))
                    if self.grammar.is_trivia(n.kind()) =>
                {
                    walk.skip_subtree();
                }
                rowan::WalkEvent::Enter(SyntaxElement::Token(t))
                    if !self.grammar.is_trivia(t.kind()) =>
                {
                    return Some(t);
                }
                _ => {}
            }
        }
        None
    }

    pub fn last_token(&self, node: &SyntaxNode) -> Option<SyntaxToken> {
        // Walk back from the node's last token over trivia, staying inside the node.
        let mut cur = node.last_token();
        while let Some(t) = cur {
            if !self.grammar.is_trivia(t.kind()) && !self.in_trivia_node(&t, node) {
                return Some(t);
            }
            cur = t
                .prev_token()
                .filter(|p| p.parent_ancestors().any(|a| a == *node));
        }
        None
    }

    /// Is `t` inside a structured trivia node (a doc comment) below `node`?
    fn in_trivia_node(&self, t: &SyntaxToken, node: &SyntaxNode) -> bool {
        self.grammar.has_trivia_nodes()
            && t.parent_ancestors()
                .take_while(|a| a != node)
                .any(|a| self.grammar.is_trivia(a.kind()))
    }

    /// A token with its leading and trailing comments.
    pub fn token(&self, tok: &SyntaxToken) -> Doc {
        let leading = self.leading(tok);
        let trailing = self.trailing(tok);
        if leading.is_empty() && trailing.is_empty() {
            return self.token_text(tok);
        }
        let lead = if self.grammar.has_tag(tok.kind(), "close") {
            self.close_leading(tok, &leading)
        } else {
            leading_comments_doc_with(&leading, &self.comment_style)
        };
        doc::concat(vec![
            lead,
            self.token_text(tok),
            trailing_comments_doc(&self.comment_style, &trailing),
        ])
    }

    /// The comments in front of a closing token: inline block comments sit tight against it.
    fn close_leading(&self, tok: &SyntaxToken, leading: &[Comment]) -> Doc {
        if !leading.is_empty()
            && leading
                .iter()
                .all(|c| c.kind == CommentKind::Block && c.breaks_after == 0)
        {
            // (no space when the closing token starts a line of its own)
            let space = if self.follows_comma(tok) {
                doc::if_break(Doc::Nil, doc::text(" "))
            } else {
                Doc::Nil
            };
            doc::concat(vec![
                space,
                doc::concat(leading.iter().map(comment_doc).collect()),
            ])
        } else {
            leading_comments_doc_with(leading, &self.comment_style)
        }
    }

    /// A closing token with the comments in front of it only (those after it belong to the
    /// caller): the tight form of [`FormatCtx::token`].
    pub fn close_token(&self, tok: &SyntaxToken) -> Doc {
        doc::concat(vec![
            self.close_leading(tok, &self.leading(tok)),
            self.token_text(tok),
        ])
    }

    /// Is the last token in front of `tok` (trivia skipped) a comma?
    fn follows_comma(&self, tok: &SyntaxToken) -> bool {
        let mut cur = tok.prev_token();
        while let Some(t) = cur {
            if !self.grammar.is_trivia(t.kind()) {
                return t.text() == ",";
            }
            cur = t.prev_token();
        }
        false
    }

    pub fn token_text(&self, tok: &SyntaxToken) -> Doc {
        verbatim_text(tok.text())
    }

    pub fn leading_comments_doc(&self, tok: &SyntaxToken) -> Doc {
        leading_comments_doc_with(&self.leading(tok), &self.comment_style)
    }

    /// Holds back the leading comments of `tok` for the rest of the run, so that a shape can
    /// print them in front of a layout that the comments' line breaks must not break: print them
    /// with [`FormatCtx::held_leading`].
    pub fn hold_leading(&self, tok: &SyntaxToken) {
        let at = tok.text_range().start();
        let mut held = self.held_leading.lock().expect("held leading lock");
        held.insert(at);
        self.any_held_leading
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Were the leading comments of `tok` held back?
    pub fn is_held_leading(&self, tok: &SyntaxToken) -> bool {
        self.held_leading
            .lock()
            .expect("held leading lock")
            .contains(&tok.text_range().start())
    }

    /// The comments [`FormatCtx::hold_leading`] held back, as printed leading comments.
    pub fn held_leading(&self, tok: &SyntaxToken) -> Doc {
        leading_comments_doc_with(&self.leading_of(tok), &self.comment_style)
    }

    /// The comments in front of `tok`, as the comment style wants them attached to it.
    fn leading(&self, tok: &SyntaxToken) -> Vec<Comment> {
        let cs = self.leading_of(tok);
        if !cs.is_empty()
            && self
                .any_held_leading
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            let held = self.held_leading.lock().expect("held leading lock");
            if held.contains(&tok.text_range().start()) {
                return Vec::new();
            }
        }
        cs
    }

    fn leading_of(&self, tok: &SyntaxToken) -> Vec<Comment> {
        let mut cs = comments::leading_comments(self.grammar, tok);
        let style = &self.comment_style;
        if (style.attach_comments || style.attach_docs)
            && let Some(last) = cs.last_mut()
            && (style.attach_comments
                && matches!(last.kind, CommentKind::Line | CommentKind::Lines)
                || (style.attach_docs && last.text().starts_with("/**")))
            && last.breaks_after >= 2
            && !last.text().starts_with("////")
            && !(last.kind == CommentKind::Lines && last.text().starts_with('#'))
            && !comments::starts_file(self.grammar, tok)
        {
            last.breaks_after = 1;
        }
        cs
    }

    pub fn trailing_comments_doc(&self, tok: &SyntaxToken) -> Doc {
        trailing_comments_doc(&self.comment_style, &self.trailing(tok))
    }

    /// The trailing comments of `tok`, none while a shape has detached them.
    fn trailing(&self, tok: &SyntaxToken) -> Vec<Comment> {
        let cs = comments::trailing_comments(self.grammar, tok);
        if !cs.is_empty() {
            let at = tok.text_range().start();
            let mut held = self.held.lock().expect("held lock");
            if let Some(h) = held.get_mut(&at) {
                *h = Some(cs);
                return Vec::new();
            }
            drop(held);
            let mut detached = self.detached.lock().expect("detached lock");
            if let Some(d) = detached.iter_mut().find(|d| d.0 == at) {
                d.1 = Some(cs);
                return Vec::new();
            }
        }
        cs
    }

    /// Holds back the trailing comments of `tok` from the next printing of the token, so that a
    /// shape can print them itself after what follows the token (a list separator).
    pub fn detach_trailing(&self, tok: &SyntaxToken) {
        let mut detached = self.detached.lock().expect("detached lock");
        detached.push((tok.text_range().start(), None));
    }

    /// [`FormatCtx::detach_trailing`] for the rest of the run, ended by nothing: read the comments
    /// with [`FormatCtx::held_trailing`] as often as needed.
    pub fn hold_trailing(&self, tok: &SyntaxToken) {
        let at = tok.text_range().start();
        let mut held = self.held.lock().expect("held lock");
        held.entry(at).or_insert(None);
    }

    /// Has [`FormatCtx::hold_trailing`] been called for `tok`?
    pub fn is_held(&self, tok: &SyntaxToken) -> bool {
        let held = self.held.lock().expect("held lock");
        held.contains_key(&tok.text_range().start())
    }

    /// The comments [`FormatCtx::hold_trailing`] held back, as the printed trailing comments of the
    /// token; nothing when the token was not printed through the context.
    pub fn held_trailing(&self, tok: &SyntaxToken) -> Doc {
        let at = tok.text_range().start();
        let held = self.held.lock().expect("held lock");
        held.get(&at)
            .and_then(|h| h.as_ref())
            .map_or(Doc::Nil, |cs| {
                trailing_comments_doc(&self.comment_style, cs)
            })
    }

    /// Ends [`FormatCtx::detach_trailing`]: the comments of a token that was printed without them
    /// (the caller prints them), none when the token was not printed through the context (its
    /// comments are part of what was printed).
    pub fn reattach_trailing(&self, tok: &SyntaxToken) -> Doc {
        let at = tok.text_range().start();
        let mut detached = self.detached.lock().expect("detached lock");
        let i = detached.iter().position(|d| d.0 == at);
        i.and_then(|i| detached.swap_remove(i).1)
            .map_or(Doc::Nil, |cs| {
                trailing_comments_doc(&self.comment_style, &cs)
            })
    }

    /// `f` with the trailing comments of `tok` held back from what it prints and put after it, so
    /// that they stay at the end of their line and do not break a group in `f`.
    pub fn trailing_after(
        &self,
        tok: Option<&SyntaxToken>,
        f: impl FnOnce() -> Result<Doc, FormatError>,
    ) -> Result<Doc, FormatError> {
        let Some(tok) = tok else { return f() };
        self.detach_trailing(tok);
        let doc = f()?;
        Ok(doc::concat(vec![doc, self.reattach_trailing(tok)]))
    }

    /// Source text from the node's first to its last token, plus those tokens' outer comments.
    pub fn verbatim_node(&self, node: &SyntaxNode) -> Doc {
        match (self.first_token(node), self.last_token(node)) {
            (Some(first), Some(last)) => self.verbatim_range(&first, &last),
            _ => Doc::Nil,
        }
    }

    /// Source text from the start of `first` to the end of `last`, exactly as written (including
    /// the comments between them), with `first`'s leading and `last`'s trailing comments around it.
    pub fn verbatim_range(&self, first: &SyntaxToken, last: &SyntaxToken) -> Doc {
        // Sliced from the nearest node that holds both tokens: slicing the root walks the file.
        let span = core_syntax::TextRange::new(first.text_range().start(), last.text_range().end());
        let raw = first
            .parent_ancestors()
            .find(|a| a.text_range().contains_range(span))
            .map(|n| {
                let at = n.text_range().start();
                n.text()
                    .slice(core_syntax::TextRange::new(
                        span.start() - at,
                        span.end() - at,
                    ))
                    .to_string()
            })
            .unwrap_or_default();
        doc::concat(vec![
            self.leading_comments_doc(first),
            verbatim_text(&raw),
            self.trailing_comments_doc(last),
        ])
    }
}

/// Text that may span several lines: a multi-line text breaks the groups around it.
pub fn verbatim_text(s: &str) -> Doc {
    if s.contains('\n') || s.contains('\r') {
        doc::concat(vec![doc::text(s), doc::break_parent()])
    } else {
        doc::snippet(s)
    }
}

/// One own-line comment (or kept-as-written text) as a document, without the break after it.
pub fn comment_doc(c: &Comment) -> Doc {
    comment_doc_in(c, &CommentStyle::default())
}

/// [`comment_doc`] in a comment style.
fn comment_doc_in(c: &Comment, st: &CommentStyle) -> Doc {
    let text = c.text();
    match c.kind {
        CommentKind::Lines => lines_doc(&text),
        CommentKind::Directive => doc::concat(vec![doc::line_start(text), doc::break_parent()]),
        CommentKind::Line => match st.reflow_width {
            Some(width) => doc::reflow(st.line_text(c), width),
            None => verbatim_text(&text),
        },
        CommentKind::Block if st.reindent && text.contains('\n') => reindented_doc(c, &text, st),
        CommentKind::Block if st.trim_lines && text.contains('\n') => verbatim_text(
            &text
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        CommentKind::Block => verbatim_text(&text),
    }
}

/// The column the comment starts at in the source, when only blanks stand before it on its line.
fn source_column(c: &Comment) -> Option<usize> {
    let (start, root) = match &c.element {
        SyntaxElement::Token(t) => (t.text_range().start(), t.parent()?.ancestors().last()?),
        SyntaxElement::Node(n) => (n.text_range().start(), n.ancestors().last()?),
    };
    let text = root.text();
    let mut col = 0;
    let mut at = usize::from(start);
    while at > 0 {
        at -= 1;
        match text.char_at(TextSize::from(u32::try_from(at).ok()?))? {
            '\n' => return Some(col),
            ' ' | '\t' => col += 1,
            _ => return None,
        }
    }
    Some(col)
}

/// A block comment that starts in an indented column: a line after the first that starts with
/// `*` goes one column right of the comment's start, the others keep their indentation relative
/// to the comment's own. A comment at column 0 stays at column 0.
fn reindented_doc(c: &Comment, text: &str, st: &CommentStyle) -> Doc {
    let col = source_column(c).unwrap_or(0);
    let lines = text
        .lines()
        .map(|l| if st.trim_lines { l.trim_end() } else { l });
    if col == 0 {
        return doc::concat(vec![
            doc::line_start(lines.collect::<Vec<_>>().join("\n")),
            doc::break_parent(),
        ]);
    }
    let mut parts = Vec::new();
    for (i, line) in lines.enumerate() {
        if i == 0 {
            parts.push(doc::text(line.to_string()));
            continue;
        }
        parts.push(doc::hardline());
        let bare = line.trim_start_matches([' ', '\t']);
        if bare.starts_with('*') {
            parts.push(doc::text(format!(" {bare}")));
        } else {
            parts.push(verbatim_text(&line[(line.len() - bare.len()).min(col)..]));
        }
    }
    doc::concat(parts)
}

/// The lines of `text` with their indentation replaced by the current one.
fn lines_doc(text: &str) -> Doc {
    let mut parts = Vec::with_capacity(2 * text.lines().count());
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_start();
        if i > 0 {
            parts.push(doc::hardline());
            if line.starts_with('*') {
                parts.push(doc::text(format!(" {line}")));
                continue;
            }
        }
        parts.push(doc::snippet(line));
    }
    doc::concat(parts)
}

/// Own-line comments go on their own lines, inline block comments stay inline.
pub fn leading_comments_doc(cs: &[Comment]) -> Doc {
    leading_comments_doc_with(cs, &CommentStyle::default())
}

/// [`leading_comments_doc`] in a comment style.
pub fn leading_comments_doc_with(cs: &[Comment], st: &CommentStyle) -> Doc {
    let max_blank = st.max_blank_after;
    if cs.is_empty() {
        return Doc::Nil;
    }
    let mut parts = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        // A comment that was on its own line starts one, wherever the formatter put the code
        // before it (a later one is on the line of the comment before it, or that comment ended
        // the line already).
        if i == 0 && (st.keep_gap || st.align_lines) && continues_trailing(c) {
            // The next line of a comment that began after code stays under it.
            parts.push(continuing_comment_doc(c, st));
        } else {
            if i == 0 && (c.kind != CommentKind::Block || c.breaks_after > 0) {
                parts.push(doc::newline_if_needed());
            }
            parts.push(comment_doc_in(c, st));
        }
        if c.kind == CommentKind::Line || c.breaks_after > 0 {
            parts.push(doc::hardline());
            for _ in 0..c.breaks_after.saturating_sub(1).min(max_blank) {
                parts.push(doc::hardline());
            }
        } else if (!st.keep_gap || gap_follows(c)) && !st.is_glued(&c.text()) {
            parts.push(doc::lit(" "));
        }
    }
    doc::concat(parts)
}

/// The comment on a line of its own, under the trailing comment that ended the line before.
pub(crate) fn continuing_comment_doc(c: &Comment, st: &CommentStyle) -> Doc {
    if st.align_lines {
        // (the column is known when the comments of the lines are aligned)
        return doc::outdent(
            usize::MAX,
            doc::concat(vec![doc::lit("\u{2}"), comment_doc_in(c, st)]),
        );
    }
    doc::outdent_to_suffix(comment_doc_in(c, st), comments::column(c.token()))
}

/// Is `c` a line comment on its own line right under a trailing `//` comment, in its column?
pub(crate) fn continues_trailing(c: &Comment) -> bool {
    let SyntaxElement::Token(t) = &c.element else {
        return false;
    };
    let ws = |t: &SyntaxToken| {
        t.text()
            .bytes()
            .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
    };
    if c.kind != CommentKind::Line {
        return false;
    }
    // the blanks before the comment hold one line break
    let (mut breaks, mut cur) = (0, t.prev_token());
    while let Some(w) = cur.as_ref().filter(|w| ws(w)) {
        breaks += w.text().matches('\n').count();
        cur = w.prev_token();
    }
    let Some(p) = cur.filter(|p| breaks == 1 && p.text().starts_with("//")) else {
        return false;
    };
    let mut before = p.prev_token();
    while let Some(w) = before
        .as_ref()
        .filter(|w| ws(w) && !w.text().contains('\n'))
    {
        before = w.prev_token();
    }
    let after_code = before.is_some_and(|x| !ws(&x));
    after_code && comments::column(&p) == comments::column(t)
}

/// Is the comment followed by a blank in the source (`/*x=*/0` has none)?
fn gap_follows(c: &Comment) -> bool {
    match &c.element {
        SyntaxElement::Token(t) => t
            .next_token()
            .is_none_or(|n| n.text().starts_with([' ', '\t', '\n', '\r'])),
        SyntaxElement::Node(n) => n
            .last_token()
            .and_then(|t| t.next_token())
            .is_none_or(|n| n.text().starts_with([' ', '\t', '\n', '\r'])),
    }
}

/// Comments after a token on its line: line comments are deferred to the end of the line and
/// force the enclosing group to break.
pub fn trailing_comments_doc(st: &CommentStyle, cs: &[Comment]) -> Doc {
    if cs.is_empty() {
        return Doc::Nil;
    }
    let mut parts = Vec::new();
    for c in cs {
        let text = st.line_text(c);
        let body = doc::concat(vec![st.gap_before(c), verbatim_text(&text)]);
        match c.kind {
            CommentKind::Line | CommentKind::Lines | CommentKind::Directive => {
                parts.push(doc::line_suffix(body));
                if !st.trailing_counts_after.is_empty() {
                    parts.push(doc::reserve(1 + text.chars().count()));
                }
                parts.push(doc::break_parent());
            }
            CommentKind::Block => parts.push(body),
        }
    }
    doc::concat(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::{TreeBuilder, grammars};
    use std::cell::Cell;

    #[test]
    fn layout_builds_once_per_node_and_flags() {
        let g = grammars::json();
        let mut b = TreeBuilder::new();
        b.start_node(g.kind_by_name("Document").unwrap());
        b.token(g.kind_by_name("Number").unwrap(), "1");
        b.finish_node();
        let root = SyntaxNode::new_root(b.finish());
        let (options, table) = (ResolvedOptions::default(), DirectiveTable::default());
        let cx = FormatCtx::new(g, &options, &table, &DefaultFormatter);
        let runs = Cell::new(0);
        let make = || {
            runs.set(runs.get() + 1);
            Ok(doc::concat(vec![doc::lit("a"), doc::line()]))
        };
        let first = cx.layout(&root, 0, make).unwrap();
        assert_eq!(cx.layout(&root, 0, make).unwrap(), first);
        assert_eq!(runs.get(), 1);
        cx.layout(&root, 1, make).unwrap();
        assert_eq!(runs.get(), 2);
    }
}
