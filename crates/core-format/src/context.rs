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
    /// One expression as `Expression`, and the operands of a binary that continues on following
    /// lines are indented (`vue/html-indent`).
    ExpressionIndented,
    /// One expression of an Angular template: as `Expression`, and call arguments take no trailing
    /// comma (Angular's expression syntax has none).
    NgExpression,
    /// One expression of an Angular template that is not the whole value (an interpolation, a
    /// statement, a block's parameter): as `NgExpression`, and a binary's operands are indented.
    NgNested,
    /// A list of declarations (`style=""`).
    Declarations,
    /// A whole file whose line breaks are line continuations (the shell code of a Dockerfile `RUN`):
    /// every statement separator stays, a break between statements is a continuation.
    Continued,
}

impl EmbedMode {
    const ALL: [EmbedMode; 7] = [
        EmbedMode::Program,
        EmbedMode::Expression,
        EmbedMode::ExpressionIndented,
        EmbedMode::NgExpression,
        EmbedMode::NgNested,
        EmbedMode::Declarations,
        EmbedMode::Continued,
    ];

    /// The name the other language's provider reads in its `embed_mode` option.
    pub fn name(self) -> &'static str {
        match self {
            EmbedMode::Program => "program",
            EmbedMode::Expression => "expression",
            EmbedMode::ExpressionIndented => "expression-indented",
            EmbedMode::NgExpression => "ng-expression",
            EmbedMode::NgNested => "ng-nested",
            EmbedMode::Declarations => "declarations",
            EmbedMode::Continued => "continued",
        }
    }

    /// The mode of the expressions a host embeds: `ExpressionIndented` when its
    /// `indent_expression_operands` option is on.
    pub fn expression_of(options: &core_config::ResolvedOptions) -> EmbedMode {
        if options.bool(core_config::opt::INDENT_EXPRESSION_OPERANDS) {
            EmbedMode::ExpressionIndented
        } else {
            EmbedMode::Expression
        }
    }

    /// The mode of an `embed_mode` option value.
    pub fn from_name(name: &str) -> Option<EmbedMode> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    /// The mode the host's engine set in the `embed_mode` option; `None` for a whole file.
    pub fn of(options: &core_config::ResolvedOptions) -> Option<EmbedMode> {
        Self::from_name(options.text(core_config::opt::EMBED_MODE))
    }

    /// The text a provider parses for code of the mode `version` names: an expression is the
    /// only element of an array (`[\n expression \n]`), anything else is the text itself.
    pub fn source<'a>(version: Option<&str>, text: &'a str) -> std::borrow::Cow<'a, str> {
        if version
            .and_then(Self::from_name)
            .is_some_and(Self::is_expression)
        {
            std::borrow::Cow::Owned(format!("[\n{text}\n]"))
        } else {
            std::borrow::Cow::Borrowed(text)
        }
    }

    /// The trailing comma style to use (0 none, 1 es5, 2 all) when the option says `commas`:
    /// an Angular expression takes one trailing comma at most (its syntax has none).
    pub fn trailing_commas(mode: Option<EmbedMode>, commas: u8) -> u8 {
        if mode.is_some_and(Self::is_angular) {
            1
        } else {
            commas
        }
    }

    /// Is the code one expression (not a program or declarations)?
    pub fn is_expression(self) -> bool {
        !matches!(
            self,
            EmbedMode::Program | EmbedMode::Declarations | EmbedMode::Continued
        )
    }

    /// Does a binary's continuation indent its operands?
    pub fn indents_operands(self) -> bool {
        matches!(self, EmbedMode::NgNested | EmbedMode::ExpressionIndented)
    }

    /// An Angular template expression: its syntax has no trailing commas.
    pub fn is_angular(self) -> bool {
        matches!(self, EmbedMode::NgExpression | EmbedMode::NgNested)
    }
}

/// Formats code of another language for a host formatter (`<script>` in HTML): the engine
/// parses `code` with the provider of `language`, formats it for `width` columns and returns the
/// text without a final line break, or `None` when the code does not parse or the language is
/// unknown. `single_quotes`: the host's own quote is `"` (an attribute value), so strings prefer `'`.
pub trait Embedder {
    /// `forward` replaces options of the other language's resolved set with the host's values
    /// (the host's indentation, for code that is laid out in the host's lines).
    fn format(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        size: (usize, bool),
        forward: Forward<'_>,
    ) -> Option<String>;

    /// Does `after` read as `before` to the provider of `language` (its tree and its own value
    /// comparison, comments included)? `Err` says what differs, or that the language is unknown
    /// or a text does not parse.
    fn same_code(
        &self,
        language: &str,
        mode: EmbedMode,
        texts: (&str, &str),
        size: (usize, bool),
        forward: Forward<'_>,
    ) -> Result<(), String>;
}

/// Host options that an [`Embedder`] copies into the options of the embedded language.
#[derive(Clone, Copy)]
pub struct Forward<'a> {
    pub host: &'a core_config::ResolvedOptions,
    pub ids: &'a [core_config::OptId],
}

/// Tokens of one kind that hold code of another language, for the safety check
/// ([`NodeFormatter::embedded_text`]).
#[derive(Clone, Copy)]
pub struct EmbeddedText {
    pub kind: core_syntax::SyntaxKind,
    pub language: &'static str,
    pub mode: EmbedMode,
    /// The host options the embedded language takes over.
    pub forward: &'static [core_config::OptId],
    /// The code of a token's text as the embedded language reads it, and the width it is given.
    pub read: fn(&str) -> String,
    pub size: (usize, bool),
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

    /// [`NodeFormatter::extra_check`] for a check that reads an option (HTML's `dialect`); the
    /// safety check calls this one.
    fn extra_check_with(
        &self,
        before: &SyntaxNode,
        after: &SyntaxNode,
        _options: &core_config::ResolvedOptions,
    ) -> Result<(), String> {
        self.extra_check(before, after)
    }

    /// The tokens of this kind hold code of another language (shell text of a Dockerfile): the
    /// safety check compares them as that language reads them, through the embedder.
    fn embedded_text(&self) -> Option<EmbeddedText> {
        None
    }

    /// What the safety check compares of a `#restyle` token or node of the input and of the
    /// output: the text without what formatting may change (quotes, case, number spelling, blanks).
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        default_value(item)
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

    /// Does the formatter cut the runs of blank lines inside a comment (`cut_blank_runs`)? The
    /// safety check then counts such a run as one.
    fn cuts_blank_lines_in_comments(&self) -> bool {
        false
    }

    /// How comments are printed; the default keeps their text and puts one space before a
    /// comment after code.
    fn comment_style(&self, options: &ResolvedOptions) -> CommentStyle {
        CommentStyle {
            trim_lines: options.get(core_config::opt::COMMENT_TRIM_LINES)
                == Some(&core_config::OptionValue::Bool(true)),
            keep_unicode_blanks: options.get(core_config::opt::COMMENT_KEEP_UNICODE_BLANKS)
                == Some(&core_config::OptionValue::Bool(true)),
            reindent: options.get(core_config::opt::COMMENT_REINDENT_LINES)
                == Some(&core_config::OptionValue::Bool(true)),
            block_as_line: options.get(core_config::opt::SINGLE_BLOCK_COMMENT_AS_LINE)
                == Some(&core_config::OptionValue::Bool(true)),
            keep_tight: options.get(core_config::opt::BLOCK_COMMENT_KEEP_TIGHT)
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
/// The policies of the shared shapes, each built from the options the first time a shape asks.
#[derive(Default)]
struct Policies {
    chain: std::sync::OnceLock<crate::shapes::ChainPolicy>,
    binary: std::sync::OnceLock<crate::shapes::BinaryPolicy>,
    assign: std::sync::OnceLock<crate::shapes::AssignPolicy>,
    conditional: std::sync::OnceLock<crate::shapes::ConditionalPolicy>,
}

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
    /// The pointer evidence of the file (`derive_pointer_alignment`), counted when first asked for.
    pointer_net: std::sync::OnceLock<i32>,
    /// The shared shapes' policies, built from the options on first use ([`FormatCtx::chain_policy`]).
    policies: Policies,
    /// Documents of nodes formatted for a layout, by node and flags (see [`FormatCtx::layout`]).
    layouts: Mutex<HashMap<(TextRange, u32, u32), Doc>>,
    embedder: Option<&'a dyn Embedder>,
    /// The source ranges copied through as written (start to end), see [`FormatCtx::mostly_copied`].
    copied: Mutex<Vec<TextRange>>,
    /// The option `max_blank_lines`, read once (all the blank lines of the source without it).
    pub(crate) max_blank_lines: usize,
}

impl<'a> FormatCtx<'a> {
    /// The pointer evidence of the file: `count` runs once.
    pub fn pointer_net(&self, count: impl FnOnce() -> i32) -> i32 {
        *self.pointer_net.get_or_init(count)
    }

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
            max_blank_lines: match options.get(core_config::opt::MAX_BLANK_LINES) {
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
            pointer_net: std::sync::OnceLock::new(),
            policies: Policies::default(),
            layouts: Mutex::new(HashMap::new()),
            embedder: None,
            copied: Mutex::new(Vec::new()),
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

    /// Does the host run under an engine that formats other languages' code?
    pub fn has_embedder(&self) -> bool {
        self.embedder.is_some()
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
        self.embed_with(language, mode, code, (width, single_quotes), &[])
    }

    /// [`FormatCtx::embed`] with the host options `forward` copied into the other language's.
    pub fn embed_with(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        (width, single_quotes): (usize, bool),
        forward: &[core_config::OptId],
    ) -> Option<String> {
        let forward = Forward {
            host: self.options,
            ids: forward,
        };
        self.embedder?
            .format(language, mode, code, (width, single_quotes), forward)
    }

    /// The member chain policy of the options, built once per file.
    pub fn chain_policy(&self) -> &crate::shapes::ChainPolicy {
        self.policies
            .chain
            .get_or_init(|| crate::shapes::ChainPolicy::of(self.options))
    }

    /// The operator chain policy of the options, built once per file.
    pub fn binary_policy(&self) -> &crate::shapes::BinaryPolicy {
        self.policies
            .binary
            .get_or_init(|| crate::shapes::BinaryPolicy::of(self.options))
    }

    /// The assignment policy of the options, built once per file.
    pub fn assign_policy(&self) -> &crate::shapes::AssignPolicy {
        self.policies
            .assign
            .get_or_init(|| crate::shapes::AssignPolicy::of(self.options))
    }

    /// The conditional policy of the options, built once per file.
    pub fn conditional_policy(&self) -> &crate::shapes::ConditionalPolicy {
        self.policies
            .conditional
            .get_or_init(|| crate::shapes::ConditionalPolicy::of(self.options))
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
        // The first token of the node, after the trivia in front of it, is nearly always the answer.
        let range = node.text_range();
        let mut cur = node.first_token();
        let found = cur.is_some();
        while let Some(t) = cur {
            if !self.grammar.is_trivia(t.kind()) && !self.in_trivia_node(&t, node) {
                return Some(t);
            }
            cur = t
                .next_token()
                .filter(|n| range.contains_range(n.text_range()));
        }
        if found {
            return None;
        }
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

    /// [`FormatCtx::leading_comments_doc`] for the comments in front of a directive (`tok` is
    /// its first token): the lines of comments right above it that start at column 0 in the
    /// source stay at column 0, whatever the code around is indented.
    pub fn leading_comments_doc_before_directive(&self, tok: &SyntaxToken) -> Doc {
        let cs = self.leading(tok);
        let st = &self.comment_style;
        let flush = |c: &Comment| {
            matches!(c.kind, CommentKind::Line | CommentKind::Block)
                && matches!(&c.element, SyntaxElement::Token(t) if comments::column(t) == 0)
        };
        // the run of column-0 comments that touches the directive, no blank line in between
        let mut at = cs.len();
        while at > 0 && flush(&cs[at - 1]) && cs[at - 1].breaks_after == 1 {
            at -= 1;
        }
        if at == cs.len() || cs[at..].iter().any(|c| c.kind != CommentKind::Line) {
            return leading_comments_doc_with(&cs, st);
        }
        let mut parts = vec![leading_comments_doc_with(&cs[..at], st)];
        for c in &cs[at..] {
            parts.push(doc::outdent(usize::MAX, comment_doc_in(c, st)));
            parts.push(doc::hardline());
        }
        doc::concat(parts)
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

    /// Records that the text of `range` is copied through as written.
    pub fn note_copied(&self, range: TextRange) {
        self.copied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(range);
    }

    /// Is more than half of the text of `root` copied through as written (a file the engine did not
    /// lay out)? Counted: `verbatim_node`, `verbatim_range`, a file kept as written. Ranges copied
    /// twice (a retried layout) count once.
    pub fn mostly_copied(&self, root: &SyntaxNode) -> bool {
        let mut ranges = self.copied.lock().unwrap_or_else(|e| e.into_inner());
        if ranges.is_empty() {
            return false;
        }
        ranges.sort_by_key(|r| r.start());
        let (mut bytes, mut end) = (0, TextSize::from(0));
        for r in ranges.iter() {
            let start = r.start().max(end);
            if r.end() > start {
                bytes += usize::from(r.end() - start);
                end = r.end();
            }
        }
        bytes * 2 > usize::from(root.text_range().len())
    }

    /// Source text from the node's first to its last token, plus those tokens' outer comments.
    pub fn verbatim_node(&self, node: &SyntaxNode) -> Doc {
        match (self.first_token(node), self.last_token(node)) {
            (Some(first), Some(last)) => self.verbatim_range(&first, &last),
            _ => Doc::Nil,
        }
    }

    /// The source text from `from` to the end of `last`, as written.
    fn raw_text(&self, from: &SyntaxToken, last: &SyntaxToken) -> String {
        // Sliced from the nearest node that holds both tokens: slicing the root walks the file.
        let span = core_syntax::TextRange::new(from.text_range().start(), last.text_range().end());
        self.note_copied(span);
        from.parent_ancestors()
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
            .unwrap_or_default()
    }

    /// Source text from the start of `first` to the end of `last`, exactly as written (including
    /// the comments between them), with `first`'s leading and `last`'s trailing comments around it.
    pub fn verbatim_range(&self, first: &SyntaxToken, last: &SyntaxToken) -> Doc {
        doc::concat(vec![
            self.leading_comments_doc(first),
            verbatim_text(&self.raw_text(first, last)),
            self.trailing_comments_doc(last),
        ])
    }

    /// [`FormatCtx::verbatim_range`] for the region a `Start` mark opens: what follows the mark
    /// (the comments after it too) stands as written, with the indentation of its first line.
    pub fn verbatim_region(&self, first: &SyntaxToken, last: &SyntaxToken) -> Doc {
        let g = self.grammar;
        let cs = comments::leading_comments(g, first);
        let Some(mark) = cs
            .iter()
            .rposition(|c| ignore::mark_of(g, c) == Some(IgnoreMark::Start))
        else {
            return self.verbatim_range(first, last);
        };
        let from = match cs.get(mark + 1).map(|c| &c.element) {
            Some(SyntaxElement::Token(t)) => t.clone(),
            Some(SyntaxElement::Node(_)) => return self.verbatim_range(first, last),
            None => first.clone(),
        };
        // (the thing after the mark starts a line of its own; a line comment holds its line break)
        let blank = |t: &SyntaxToken| t.text().trim().is_empty();
        let own = |t: Option<SyntaxToken>| t.is_none_or(|t| t.text().ends_with('\n'));
        let (indent, own_line) = match from.prev_token() {
            Some(p) if blank(&p) => (
                p.text().rsplit('\n').next().unwrap_or("").to_string(),
                p.text().contains('\n') || own(p.prev_token()),
            ),
            p => (String::new(), own(p)),
        };
        if !own_line {
            return self.verbatim_range(first, last);
        }
        let raw = format!("{indent}{}", self.raw_text(&from, last));
        let (head, rest) = raw.split_once('\n').unwrap_or((&raw, ""));
        let rest = if rest.is_empty() {
            Doc::Nil
        } else {
            doc::verbatim(format!("\n{rest}"))
        };
        doc::concat(vec![
            leading_comments_doc_with(&cs[..=mark], &self.comment_style),
            doc::line_start(head.trim_end()),
            rest,
            self.trailing_comments_doc(last),
        ])
    }
}

/// [`NodeFormatter::value`] as it is unless a formatter says otherwise.
pub fn default_value(item: &SyntaxElement) -> Cow<'_, str> {
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

/// Text that may span several lines: a multi-line text breaks the groups around it.
pub fn verbatim_text(s: &str) -> Doc {
    if s.contains('\n') || s.contains('\r') {
        doc::concat(vec![doc::text(s), doc::break_parent()])
    } else {
        doc::snippet(s)
    }
}

/// A multi-line comment text with the blanks at the end of each line removed.
pub fn trimmed_lines_doc(text: &str) -> Doc {
    verbatim_text(
        &text
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// The characters of the first line of `text` and, when it spans lines, of its last line.
pub fn line_ends(text: &str) -> (usize, Option<usize>) {
    let mut lines = text.split('\n');
    let first = lines.next().map_or(0, |l| l.chars().count());
    (first, lines.next_back().map(|l| l.chars().count()))
}

/// The width of the widest physical line of pieces `(white space before it, a line break
/// precedes it, text)` that holds more than one piece (the pieces are one logical line, so the
/// breaks are the ones inside a piece); a piece that spans lines ends the line it starts on and
/// starts the line it ends on. A tab counts `tab` columns. A single piece counts when no line
/// holds more.
pub fn widest_shared_line<'a>(
    pieces: impl Iterator<Item = (&'a str, bool, &'a str)>,
    tab: usize,
) -> usize {
    let (mut col, mut widest, mut pieces_on_line) = (0, 0, 0);
    for (n, (ws, breaks, text)) in pieces.enumerate() {
        if n == 0 || !breaks {
            col += ws
                .chars()
                .map(|c| if c == '\t' { tab } else { 1 })
                .sum::<usize>();
        } else {
            col = 0;
        }
        pieces_on_line += 1;
        let (first, last) = line_ends(text);
        col += first;
        if let Some(last) = last {
            if pieces_on_line > 1 {
                widest = widest.max(col);
            }
            (col, pieces_on_line) = (last, 1);
        }
    }
    if pieces_on_line > 1 || widest == 0 {
        widest = widest.max(col);
    }
    widest
}

/// `text` with every run of blank lines cut to `most` lines; `None` when none is longer.
pub fn cut_blank_runs(text: &str, most: u32) -> Option<String> {
    let mut run = 0;
    let mut changed = false;
    let mut lines = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            run += 1;
            if run > most {
                changed = true;
                continue;
            }
        } else {
            run = 0;
        }
        lines.push(line);
    }
    changed.then(|| lines.join("\n"))
}

/// One own-line comment (or kept-as-written text) as a document, without the break after it.
pub fn comment_doc(c: &Comment) -> Doc {
    comment_doc_in(c, &CommentStyle::default())
}

/// [`comment_doc`] in a comment style.
pub fn comment_doc_in(c: &Comment, st: &CommentStyle) -> Doc {
    let text = c.text_in(st);
    match c.kind {
        CommentKind::Lines => lines_doc(&text),
        CommentKind::Directive => doc::concat(vec![doc::line_start(text), doc::break_parent()]),
        CommentKind::Line => match st.reflow_width {
            Some(width) => doc::reflow(st.line_text(c), width),
            None => verbatim_text(&text),
        },
        CommentKind::Block if st.reindent && text.contains('\n') => reindented_doc(c, &text, st),
        CommentKind::Block if st.trim_lines && text.contains('\n') => trimmed_lines_doc(&text),
        CommentKind::Block => verbatim_text(&st.line_text(c)),
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

/// A comment `text` as written, or with `indented` (a multi-line one) every line after the first one level deeper than the
/// comment and re-indented to it, a line that holds only the closing `closer` at the comment's
/// own level; blank lines stay empty.
pub fn comment_text(text: &str, indented: bool, closer: &str) -> Doc {
    if !indented || !text.contains('\n') {
        return verbatim_text(text);
    }
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default();
    let rest: Vec<&str> = lines.collect();
    let (body, last) = match rest.split_last() {
        Some((l, b)) if l.trim() == closer => (b, Some(closer)),
        _ => (&rest[..], None),
    };
    let mut inner = Vec::new();
    for line in body {
        inner.push(doc::hardline());
        inner.push(doc::text(line.trim()));
    }
    let mut parts = vec![doc::text(first.trim_end()), doc::indent(doc::concat(inner))];
    if let Some(l) = last {
        parts.push(doc::hardline());
        parts.push(doc::text(l));
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
        } else if (!st.keep_gap || gap_follows(c))
            && !st.is_glued(&c.text())
            && !(st.keep_tight && !gap_follows(c))
        {
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
pub(crate) fn gap_follows(c: &Comment) -> bool {
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
    #[test]
    fn line_ends_of_a_text_that_spans_lines() {
        assert_eq!(super::line_ends("ab"), (2, None));
        assert_eq!(super::line_ends("ab\nc\ndef"), (2, Some(3)));
    }

    #[test]
    fn the_widest_line_shared_by_pieces() {
        let w = |p: &[(&'static str, bool, &'static str)], tab| {
            super::widest_shared_line(p.iter().copied(), tab)
        };
        // a single piece, and tabs
        assert_eq!(w(&[("", false, "abc")], 4), 3);
        assert_eq!(w(&[("\t", false, "ab"), (" ", false, "c")], 4), 8);
        // a break before a piece resets the column
        assert_eq!(w(&[("", false, "abcd"), (" ", true, "ef")], 4), 2);
        // a multi-line piece that ends on a long line: the line it ends on counts with what follows
        assert_eq!(
            w(&[("", false, "/* a\nb */"), (" ", false, "cdefg")], 4),
            10
        );
        // one that starts on a long line: the line it starts on counts with what precedes it
        assert_eq!(
            w(&[("", false, "abcdefg"), (" ", false, "/* x\ny */")], 4),
            12
        );
        // a lone multi-line piece does not make a line long
        assert_eq!(
            w(&[("", false, "$$ aaaaaaaaaa\nb $$"), ("", true, "c")], 4),
            1
        );
    }

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

#[cfg(test)]
mod embed_mode_tests {
    use super::EmbedMode;

    #[test]
    fn source_wraps_only_expressions() {
        for (version, wrapped) in [
            (Some("expression"), true),
            (Some("expression-indented"), true),
            (Some("ng-nested"), true),
            (Some("program"), false),
            (Some("declarations"), false),
            (Some("es2020"), false),
            (None, false),
        ] {
            let out = EmbedMode::source(version, "a");
            assert_eq!(out, if wrapped { "[\na\n]" } else { "a" }, "{version:?}");
        }
    }

    #[test]
    fn angular_modes_cap_the_comma_style() {
        for mode in [EmbedMode::NgExpression, EmbedMode::NgNested] {
            assert_eq!(EmbedMode::trailing_commas(Some(mode), 2), 1);
            assert_eq!(EmbedMode::trailing_commas(Some(mode), 0), 1);
        }
        for mode in [None, Some(EmbedMode::Expression), Some(EmbedMode::Program)] {
            assert_eq!(EmbedMode::trailing_commas(mode, 2), 2);
            assert_eq!(EmbedMode::trailing_commas(mode, 0), 0);
        }
    }
}
