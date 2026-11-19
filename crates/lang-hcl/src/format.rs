//! HCL formatting, the layout of `terraform fmt` (hashicorp/hcl `hclwrite.Format`): every source
//! line stays a line (blank lines and line breaks inside expressions are kept), and on each line
//! the indentation follows the brackets, the spaces between tokens follow fixed rules, and the `=`
//! of consecutive attributes and their trailing comments line up. The tree only classifies the
//! tokens; the layout works on the token stream, as hclwrite does.
//!
//! concept: key-value-layout, block-and-statements

use crate::generated::kind as k;
use core_config::shared;
use core_format::columns;
use core_format::frames::Frames;
use core_format::rules::LazyRules;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, grammars};
use rowan::WalkEvent;

include!("schema.generated.rs");

static ROWS: LazyRules = LazyRules::new(|| grammars::HCL.get(), include_str!("../format.rules"));

/// A `${` or `%{` and the `}` that ends it: the template text mode stops inside them.
fn in_template_code(t: &SyntaxToken) -> bool {
    t.parent()
        .is_some_and(|p| matches!(p.kind(), k::INTERPOLATION | k::TEMPLATE_DIRECTIVE))
}

pub struct HclFormatter;

impl NodeFormatter for HclFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        if node.kind() != k::DOCUMENT {
            return Ok(None);
        }
        let mut lines = lines(node);
        indent(
            &mut lines,
            cx.options.int(shared::INDENT_SIZE).max(1) as usize,
        );
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            line.print(&mut out);
        }
        // Blank lines at the end stay, as the blanks after a heredoc's closing marker do.
        Ok(Some(doc::verbatim(out)))
    }
}

struct Tok {
    tok: SyntaxToken,
    text: String,
    /// Text written before the token as it stands: the blanks and line breaks of template text.
    raw: String,
    spaces: usize,
    /// A `#` or `//` comment.
    line_comment: bool,
}

impl Tok {
    fn columns(&self) -> usize {
        self.spaces + self.text.chars().count()
    }
}

#[derive(Default)]
struct Line {
    lead: Vec<Tok>,
    assign: Vec<Tok>,
    comment: Option<Tok>,
}

fn columns(toks: &[Tok]) -> usize {
    toks.iter().map(Tok::columns).sum()
}

impl Line {
    fn print(&self, out: &mut String) {
        for t in self.lead.iter().chain(&self.assign).chain(&self.comment) {
            out.extend(std::iter::repeat_n(' ', t.spaces));
            out.push_str(&t.raw);
            out.push_str(&t.text);
        }
    }
}

/// Splits the token stream into lines. Blanks and line breaks inside template text stay as written
/// (`raw`); a line break anywhere else ends the line.
fn lines(root: &SyntaxNode) -> Vec<Line> {
    let mut lines = vec![Line::default()];
    // True for each open template, false for each open `${`/`%{`.
    let mut text_mode: Vec<bool> = Vec::new();
    let mut raw = String::new();
    // Templates entered whose first token has not come yet: the trivia before it is the code's.
    let mut opening = 0;
    for event in root.preorder_with_tokens() {
        match event {
            WalkEvent::Enter(SyntaxElement::Node(n))
                if matches!(n.kind(), k::QUOTED_TEMPLATE | k::HEREDOC) =>
            {
                opening += 1;
            }
            WalkEvent::Leave(SyntaxElement::Node(n))
                if matches!(n.kind(), k::QUOTED_TEMPLATE | k::HEREDOC) =>
            {
                text_mode.pop();
                // What is left is the trivia after the closing token, which belongs to the code.
                for _ in raw.matches('\n') {
                    lines.push(Line::default());
                }
                raw.clear();
            }
            WalkEvent::Enter(SyntaxElement::Token(t)) => {
                let text = t.text().replace("\r\n", "\n");
                match t.kind() {
                    k::EOF => {}
                    k::WHITESPACE | k::NEWLINE if text_mode.last() == Some(&true) => {
                        raw.push_str(&text)
                    }
                    k::WHITESPACE => {}
                    k::NEWLINE => lines.extend(text.matches('\n').map(|_| Line::default())),
                    kind => {
                        if in_template_code(&t) {
                            match kind {
                                k::OPEN => text_mode.push(false),
                                k::CLOSE => {
                                    text_mode.pop();
                                }
                                _ => {}
                            }
                        }
                        let line_comment = kind == k::COMMENT && !text.starts_with("/*");
                        let text = if kind == k::COMMENT {
                            text.trim_end().to_string()
                        } else {
                            text
                        };
                        let line = lines.last_mut().expect("a line");
                        line.lead.push(Tok {
                            tok: t.clone(),
                            text,
                            raw: std::mem::take(&mut raw),
                            spaces: 0,
                            line_comment,
                        });
                        if kind != k::COMMENT {
                            text_mode
                                .extend(std::iter::repeat_n(true, std::mem::take(&mut opening)));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    lines
}

/// The blanks in front of each token of a cell: the `[space]` rows, none inside template text.
fn space_cell(cell: &mut [Tok]) {
    let rows = ROWS.get().space();
    for i in 1..cell.len() {
        let (head, tail) = cell.split_at_mut(i);
        let next = &mut tail[0];
        next.spaces = usize::from(next.raw.is_empty() && rows.spaced(&head[i - 1].tok, &next.tok));
    }
}

fn indent(lines: &mut [Line], size: usize) {
    split_cells(lines);
    let brackets = ROWS.get().brackets();
    let mut frames = Frames::default();
    for line in lines.iter_mut() {
        if line.lead.is_empty() {
            continue;
        }
        let net: i32 = (line.lead.iter().chain(&line.assign))
            .map(|t| brackets.net(t.tok.kind()))
            .sum();
        let depth = frames.line(net);
        space_cell(&mut line.lead);
        if let Some(first) = line.lead.first_mut() {
            first.spaces = depth * size;
        }
        if let Some(eq) = line.assign.first_mut() {
            eq.spaces = 1;
        }
        space_cell(&mut line.assign);
    }
    align(lines);
}

/// The `=` and what follows it become the assign cell of a line that holds a whole expression, a
/// trailing `#` comment the comment cell.
fn split_cells(lines: &mut [Line]) {
    let brackets = ROWS.get().brackets();
    for line in lines {
        if line.lead.len() > 1 && line.lead.last().is_some_and(|t| t.line_comment) {
            line.comment = line.lead.pop();
        }
        let Some(eq) = line
            .lead
            .iter()
            .skip(1)
            .position(|t| t.tok.kind() == k::PUNCT && t.text == "=")
            .map(|i| i + 1)
        else {
            continue;
        };
        if line.lead[eq..]
            .iter()
            .map(|t| brackets.net(t.tok.kind()))
            .sum::<i32>()
            == 0
        {
            line.assign = line.lead.split_off(eq);
        }
    }
}

/// Runs of consecutive lines with an assign cell: the `=` lines up one space after the widest key.
/// Runs of consecutive lines with a comment: the comments line up one space after the longest code.
fn align(lines: &mut [Line]) {
    columns::runs(
        lines,
        |l| !l.assign.is_empty(),
        |run| {
            let widths: Vec<usize> = run.iter().map(|l| columns(&l.lead)).collect();
            for (l, pad) in run.iter_mut().zip(columns::pads(&widths)) {
                l.assign[0].spaces = pad + 1;
            }
        },
    );
    columns::runs(
        lines,
        |l| l.comment.is_some(),
        |run| {
            let widths: Vec<usize> = (run.iter())
                .map(|l| columns(&l.lead) + columns(&l.assign))
                .collect();
            for (l, pad) in run.iter_mut().zip(columns::pads(&widths)) {
                if let Some(c) = &mut l.comment {
                    c.spaces = pad + 1;
                }
            }
        },
    );
}
