//! Svelte: `{expression}`, `{@html x}` and the tags of blocks (`{#if x}`, `{:else}`, `{/if}`).

use super::{Open, P, is_ws};
use crate::generated::kind as k;

impl P<'_> {
    /// The end (after the `}`) of the braces opened at `i`: nested braces and strings are skipped.
    pub(super) fn brace_end(&self, i: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut j = i;
        while j < self.b.len() {
            match self.b[j] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j + 1);
                    }
                }
                q @ (b'"' | b'\'' | b'`') => {
                    j += 1;
                    while j < self.b.len() && self.b[j] != q {
                        j += usize::from(self.b[j] == b'\\');
                        j += 1;
                    }
                }
                b'/' if self.at(j + 1) == b'/' => {
                    j = self.src[j..].find('\n').map_or(self.b.len(), |n| j + n);
                }
                b'/' if self.at(j + 1) == b'*' => {
                    j = self.src[j + 2..]
                        .find("*/")
                        .map_or(self.b.len(), |n| j + n + 3);
                }
                _ => {}
            }
            j += 1;
        }
        None
    }

    /// A tag in braces: `{expression}` (and `{@html x}`) is a `Mustache`, `{#if x}`, `{:else}` and
    /// `{/if}` are `BlockTag`s in a `Block`.
    pub(super) fn svelte_brace(&mut self) {
        let start = self.pos;
        let Some(end) = self.brace_end(start) else {
            self.error(start, start + 1, "unclosed {");
            return self.text(true);
        };
        let mut i = start + 1;
        while i < end && is_ws(self.b[i]) {
            i += 1;
        }
        let sigil = self.at(i);
        let word = self.src[i + 1..end]
            .split(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("");
        self.pos = end;
        match sigil {
            b'#' if matches!(word, "if" | "each" | "await" | "key" | "snippet") => {
                let block = self.open(k::BLOCK, start);
                self.stack.push((Open::Block, block));
                self.block_tag(start, end);
            }
            b':' | b'/' => {
                let block = self.stack.iter().rposition(|&(o, _)| o == Open::Block);
                let Some(at) = block else {
                    self.error(start, end, "no block is open");
                    return self.block_tag(start, end);
                };
                while self.stack.len() > at + 1 {
                    let (_, node) = self.stack.pop().unwrap();
                    self.close(node);
                }
                self.block_tag(start, end);
                if sigil == b'/' {
                    let (_, node) = self.stack.pop().unwrap();
                    self.close(node);
                }
            }
            _ => self.mustache(start, end),
        }
    }

    fn block_tag(&mut self, start: usize, end: usize) {
        self.leaf(k::BLOCK_TAG, k::BLOCK_TAG_TEXT, start, end);
    }

    pub(super) fn mustache(&mut self, start: usize, end: usize) {
        self.leaf(k::MUSTACHE, k::MUSTACHE_TEXT, start, end);
    }
}
