//! Indentation by open frames for formatters that keep every source line: a line is indented by
//! the frames open when it starts, a bracket opens a frame, its closer ends it, and the lines that
//! continue a pipeline or a statement (a `|` or a backtick at the end of a line) are a frame of
//! their own that the next line without a continuation ends. Which kinds open and close a frame
//! is the `[lines]` section of `format.rules`: `open = A|B` and `close = C|D`.
//!
//! concept: indent-frames
//! what: Line indentation from a stack of bracket and continuation frames.
//! tools: terraform fmt (hclwrite indentation), PSScriptAnalyzer PSUseConsistentIndentation
//! reads: rules:lines

use core_syntax::{Grammar, SyntaxKind};

/// The kinds of tokens that open and close a frame.
#[derive(Default)]
pub struct Brackets {
    open: Vec<SyntaxKind>,
    close: Vec<SyntaxKind>,
}

impl Brackets {
    /// The `open` and `close` kind lists of the `[lines]` section of `text`.
    pub fn parse(g: &Grammar, text: &str) -> Brackets {
        let settings = crate::rules::settings(text, "lines");
        let kinds = |key: &str| -> Vec<SyntaxKind> {
            settings
                .iter()
                .filter(|(k, _)| *k == key)
                .flat_map(|(_, v)| v.split('|'))
                .map(|n| {
                    g.kind_by_name(n.trim())
                        .unwrap_or_else(|| panic!("lines: unknown kind `{n}`"))
                })
                .collect()
        };
        Brackets {
            open: kinds("open"),
            close: kinds("close"),
        }
    }

    pub fn is_open(&self, kind: SyntaxKind) -> bool {
        self.open.contains(&kind)
    }

    pub fn is_close(&self, kind: SyntaxKind) -> bool {
        self.close.contains(&kind)
    }

    /// 1 for an opener, -1 for a closer, 0 for any other kind.
    pub fn net(&self, kind: SyntaxKind) -> i32 {
        i32::from(self.is_open(kind)) - i32::from(self.is_close(kind))
    }
}

enum Frame {
    /// Brackets opened on one line (they add one level together).
    Open(u32),
    /// The lines that continue a pipeline or a statement; a line that does both is indented twice.
    Continuation { pipe: bool, tick: bool },
}

/// The stack of open frames.
#[derive(Default)]
pub struct Frames(Vec<Frame>);

impl Frames {
    /// The levels of indentation the open frames add up to.
    pub fn depth(&self) -> usize {
        (self.0.iter())
            .map(|f| match f {
                Frame::Open(_) => 1,
                Frame::Continuation { pipe, tick } => usize::from(*pipe) + usize::from(*tick),
            })
            .sum()
    }

    /// Opens a frame of `n` brackets.
    pub fn open(&mut self, n: u32) {
        self.0.push(Frame::Open(n));
    }

    /// Closes `n` brackets, innermost first; the continuation lines of a frame end with it.
    pub fn close(&mut self, mut n: u32) {
        while n > 0 {
            while matches!(self.0.last(), Some(Frame::Continuation { .. })) {
                self.0.pop();
            }
            match self.0.last_mut() {
                Some(Frame::Open(top)) if n < *top => {
                    *top -= n;
                    n = 0;
                }
                Some(Frame::Open(top)) => {
                    n -= *top;
                    self.0.pop();
                }
                _ => break,
            }
        }
    }

    /// A line of a language whose lines are indented as a whole (`net` brackets left open, or
    /// closed when negative): the levels the line is indented by.
    pub fn line(&mut self, net: i32) -> usize {
        if net > 0 {
            let depth = self.depth();
            self.open(net.unsigned_abs());
            depth
        } else {
            self.close(net.unsigned_abs());
            self.depth()
        }
    }

    /// A line starts: it continues the previous one by a pipe or a backtick, or ends the
    /// continuation.
    pub fn continue_line(&mut self, pipe: bool, tick: bool) {
        match self.0.last_mut() {
            Some(Frame::Continuation { pipe: p, tick: t }) if pipe || tick => {
                *p |= pipe;
                *t |= tick;
            }
            Some(Frame::Continuation { .. }) => {
                self.0.pop();
            }
            _ if pipe || tick => self.0.push(Frame::Continuation { pipe, tick }),
            _ => {}
        }
    }
}
