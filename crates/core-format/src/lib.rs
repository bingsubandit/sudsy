//! The shared formatting engine.
//!
//! * [`doc`]: the layout document; [`printer`]: the greedy, language-unaware printer.
//! * [`shapes`]: delimited list, block, key-value, statement sequence and verbatim.
//! * [`context`]: [`FormatCtx`] and the [`NodeFormatter`] trait providers implement.
//! * [`comments`], [`ignore`], [`directives`]: comment placement, `sudsy-ignore`, layout directives.
//! * [`safety`]: the re-parse and tree comparison check; [`testing`]: the shared test helper.
//!
//! A provider's formatter turns a node into a [`Doc`]; [`format_tree`] prints it.
//!
//! concept: none (glue: crate root and format entry)

pub mod adjacent;
mod align;
pub mod ancestry;
pub mod blank;
pub mod block_frames;
pub mod calls;
pub mod case;
pub mod columns;
pub mod comments;
pub mod content_lines;
pub mod context;
pub mod directives;
pub mod doc;
pub mod embed;
mod error;
pub mod frames;
pub mod generated;
mod header_break;
pub mod ignore;
pub mod join;
mod layout;
mod lines;
pub mod literals;
mod payload;
mod placement;
pub mod printer;
mod relay;
pub mod requote;
pub mod rewrite;
pub mod rules;
pub mod safety;
pub mod seq;
pub mod shapes;
pub mod spacing;
pub mod stream;
pub mod stream_blank;
pub mod stream_indent;
pub mod tabs;
pub mod tag_layout;
pub mod testing;
pub mod verbatim;

pub use context::{
    DefaultFormatter, EmbedMode, Embedder, FormatCtx, Forward, NodeFormatter, Variants,
};
pub use directives::{DIRECTIVES_VERSION, Directive, DirectiveConflict, DirectiveTable, NodeKey};
pub use doc::Doc;
pub use error::FormatError;
pub use placement::{comment_lines, comment_lines_max};
pub use printer::PrintOptions;
pub use verbatim::VerbatimFormatter;

use core_config::ResolvedOptions;
use core_syntax::SyntaxNode;

/// Files this process formatted that the engine mostly copied through ([`FormatCtx::mostly_copied`]).
static COPIED_THROUGH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Counts the file just formatted with `cx` when the engine mostly copied it through.
pub fn count_copied_through(cx: &FormatCtx<'_>, root: &SyntaxNode) {
    if cx.mostly_copied(root) {
        COPIED_THROUGH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The files this process formatted that the engine mostly copied through: the parity runner's
/// stability score counts such a file as unstable (docs/ai/design/parity-target.md).
pub fn copied_through_files() -> usize {
    COPIED_THROUGH.load(std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    /// Levels of [`deep`] entered on this thread, and the most since the last [`take_peak`].
    static LEVEL: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static PEAK: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The deepest [`deep`] nesting since the previous call.
fn take_peak() -> u32 {
    PEAK.replace(0)
}

/// Runs `f` on a fresh stack segment when the current one is nearly used up. The recursion over
/// a syntax tree or a document is as deep as the source nests, which debug frames cannot hold on
/// a 2 MB thread stack; every recursive entry of the engine goes through here.
pub fn deep<R>(f: impl FnOnce() -> R) -> R {
    // Measuring the stack is far dearer than a counter, so only every 32nd level measures; 32
    // debug-build levels use well under the red zone.
    let level = LEVEL.get();
    LEVEL.set(level.wrapping_add(1));
    if level > PEAK.get() {
        PEAK.set(level);
    }
    let result = if level % 32 == 31 {
        stacker::maybe_grow(512 * 1024, 4 * 1024 * 1024, f)
    } else {
        f()
    };
    LEVEL.set(level);
    result
}

/// Printer settings from the shared options (`line_width`, `indent_size`, `indent_style`); an
/// option the provider's schema does not declare keeps the printer default.
pub fn print_options(options: &ResolvedOptions) -> PrintOptions {
    use core_config::shared;
    let default = PrintOptions::default();
    let int = |name, fallback: usize| options.uint(name, fallback);
    PrintOptions {
        line_width: int(shared::LINE_WIDTH, default.line_width),
        indent_width: int(shared::INDENT_SIZE, default.indent_width),
        use_tabs: options.is(shared::INDENT_STYLE, "tab"),
        align_with_tabs: matches!(
            options.get(core_config::opt::ALIGN_WITH_TABS),
            Some(core_config::OptionValue::Bool(true))
        ),
        continuation: None,
    }
}

/// The printer settings for the final print of a tree: the code of a host whose line breaks are
/// line continuations (`EmbedMode::Continued`) is printed with them.
fn tree_print_options(cx: &FormatCtx<'_>) -> PrintOptions {
    let mut opts = print_options(cx.options);
    if EmbedMode::of(cx.options) == Some(EmbedMode::Continued) {
        opts.continuation = cx
            .grammar
            .comments
            .line
            .as_deref()
            .and_then(|m| m.chars().next())
            .map(|comment| printer::Continuation {
                escape: printer::CONTINUATION_ESCAPE,
                comment,
            });
    }
    opts
}

/// Line break style of a file: the first line break found decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn detect(text: &str) -> Self {
        match text.find('\n') {
            Some(i) if text[..i].ends_with('\r') => LineEnding::CrLf,
            _ => LineEnding::Lf,
        }
    }
}

impl LineEnding {
    /// Like [`LineEnding::detect`] for the text of a tree, without making the text.
    pub fn of_tree(root: &SyntaxNode) -> Self {
        let mut after_cr = false;
        for token in root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            let text = token.text();
            if let Some(i) = text.find('\n') {
                let cr = text[..i].ends_with('\r') || i == 0 && after_cr;
                return if cr { LineEnding::CrLf } else { LineEnding::Lf };
            }
            after_cr = text.ends_with('\r');
        }
        LineEnding::Lf
    }
}

/// Formats `root` (and everything below it) to text. The `end_of_line` option decides the line
/// ending (`auto`, the default and for providers without the option: the file's first one) and
/// the `final_newline` option, when the provider declares it, decides how the text ends.
pub fn format_tree(root: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<String, FormatError> {
    take_peak();
    let mut doc = cx.format_unguarded(root)?;
    let (mut printed, mut keep) = printer::print_protected(&mut doc, &tree_print_options(cx));
    if cx.comment_style.block_as_line {
        comments::settle_block_comments(cx.grammar, &mut printed, &mut keep);
    }
    if cx.comment_style.align_lines {
        comments::align_marked_gaps(
            &mut printed,
            &mut keep,
            if cx.comment_style.align_within_width {
                print_options(cx.options).line_width
            } else {
                usize::MAX
            },
        );
    }
    // A document of ordinary depth drops the usual way; only a pathological one needs this.
    if take_peak() > 64 {
        doc.dispose();
    }
    // Copies of the whole output only when verbatim text brought `\r\n` into it.
    let (mut out, keep) = if printed.contains("\r\n") {
        let keep = printed[..keep].replace("\r\n", "\n").len();
        (printed.replace("\r\n", "\n"), keep)
    } else {
        (printed, keep)
    };
    out.truncate(out.trim_end_matches(['\n', ' ', '\t']).len().max(keep));
    let final_newline = cx.options.flag(core_config::shared::FINAL_NEWLINE, true);
    // `keep_final_newline`: a file that has no final newline gets none.
    let kept = cx.options.flag(core_config::opt::KEEP_FINAL_NEWLINE, false) && {
        let text = root.text();
        let len = text.len();
        len > rowan::TextSize::from(0)
            && text
                .char_at(len - rowan::TextSize::from(1))
                .is_some_and(|c| c != '\n')
    };
    if final_newline && !kept && !cx.exact_end(root) && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    let ending = match cx.options.text(core_config::shared::END_OF_LINE) {
        "lf" => LineEnding::Lf,
        "crlf" => LineEnding::CrLf,
        _ => LineEnding::of_tree(root),
    };
    Ok(match ending {
        LineEnding::Lf => out,
        LineEnding::CrLf => out.replace('\n', "\r\n"),
    })
}
