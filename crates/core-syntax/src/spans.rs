//! Span algorithms of the hand-written parsers that put nodes over a token list: nodes in opening
//! order, the bracket groups of the tokens, the preprocessor-like directives that split a token
//! range, and the extents of the labels of a block.
//!
//! concept: hand-written-parser-kit

use std::cmp::Reverse;

/// Sorts nodes into opening order: by start, the longer (outer) first, then by `rank`. The sort is
/// stable, so equal keys keep their order.
pub fn sort_nested<T, K: Ord>(nodes: &mut [T], key: impl Fn(&T) -> (u32, u32, K)) {
    nodes.sort_by_key(|n| {
        let (start, end, rank) = key(n);
        (start, Reverse(end), rank)
    });
}

/// "No group", "no token": the sentinel of the index tables.
pub const NONE: u32 = u32::MAX;

/// The bracket groups over a token list. A group is a node whose span starts at an opening token
/// and ends at the last token it covers; an unclosed group ends at a closer of another group.
pub struct Groups {
    /// For an opening token: the index of the last token of its group; else `NONE`.
    last: Vec<u32>,
    /// For an opening token: the id of the node of its group.
    group: Vec<u32>,
    /// For a token that ends a group: the innermost group it ends, by its opening token.
    ender: Vec<u32>,
}

impl Groups {
    /// `tokens`: the `(start, end)` of each token; `groups`: the `(id, start, end)` of each group
    /// node, in any order (`id` is the caller's number of the node, returned by `node`). A group
    /// whose start is no token start, or that covers no token, is ignored.
    pub fn new(tokens: &[(u32, u32)], groups: impl Iterator<Item = (usize, u32, u32)>) -> Groups {
        let mut last = vec![NONE; tokens.len()];
        let mut group = vec![NONE; tokens.len()];
        for (i, start, end) in groups {
            let open = tokens.partition_point(|t| t.0 < start);
            let after = tokens.partition_point(|t| t.1 <= end);
            if open < tokens.len() && tokens[open].0 == start && after > open {
                last[open] = (after - 1) as u32;
                group[open] = i as u32;
            }
        }
        let mut ender = vec![NONE; tokens.len()];
        for (open, &l) in last.iter().enumerate() {
            if l != NONE {
                ender[l as usize] = open as u32;
            }
        }
        Groups { last, group, ender }
    }

    pub fn is_open(&self, i: usize) -> bool {
        self.last.get(i).is_some_and(|&l| l != NONE)
    }

    /// The last token of the group opened at `i`.
    pub fn last(&self, i: usize) -> Option<usize> {
        self.last
            .get(i)
            .filter(|&&l| l != NONE)
            .map(|&l| l as usize)
    }

    /// The node of the group opened at `i`.
    pub fn node(&self, i: usize) -> Option<usize> {
        self.group
            .get(i)
            .filter(|&&g| g != NONE)
            .map(|&g| g as usize)
    }

    /// The opening token of the innermost group that ends at token `i`.
    pub fn ender(&self, i: usize) -> Option<usize> {
        self.ender
            .get(i)
            .filter(|&&o| o != NONE)
            .map(|&o| o as usize)
    }

    /// The token after the one at `i`, or after its whole group.
    pub fn skip(&self, i: usize) -> usize {
        self.last(i).map_or(i + 1, |l| l + 1)
    }

    /// The group opened at `i` ends at a token of its own: `is_closer(last)` says that token is
    /// a closing bracket.
    pub fn closed(&self, i: usize, is_closer: impl Fn(usize) -> bool) -> bool {
        self.last(i)
            .is_some_and(|l| l > i && self.ender(l) == Some(i) && is_closer(l))
    }

    /// The tokens between the brackets of the group opened at `i`.
    pub fn inner(&self, i: usize, is_closer: impl Fn(usize) -> bool) -> (usize, usize) {
        let l = self.last(i).unwrap_or(i);
        (i + 1, if self.closed(i, is_closer) { l } else { l + 1 })
    }
}

/// Where the directives are among the code tokens: before which one, and in which group.
pub struct Directives {
    at: Vec<(u32, i64)>,
}

impl Directives {
    /// `marks`: for every token of the file in order, 0 for a code token, else the number of the
    /// directive it belongs to (equal numbers are one directive).
    pub fn new(marks: impl Iterator<Item = u32>, groups: &Groups) -> Directives {
        let mut at = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        let (mut ci, mut seen) = (0usize, 0u32);
        let pop = |stack: &mut Vec<usize>, ci: usize| {
            while stack
                .last()
                .is_some_and(|&o| groups.last(o).is_some_and(|l| l < ci))
            {
                stack.pop();
            }
        };
        for m in marks {
            if m == 0 {
                pop(&mut stack, ci);
                if groups.is_open(ci) {
                    stack.push(ci);
                }
                ci += 1;
            } else if m != seen {
                seen = m;
                pop(&mut stack, ci);
                at.push((ci as u32, stack.last().map_or(-1, |&o| o as i64)));
            }
        }
        Directives { at }
    }

    /// A directive lies between two parts of the code tokens `a..=z`, not inside a group of them.
    pub fn crosses(&self, a: usize, z: usize) -> bool {
        let from = self.at.partition_point(|d| d.0 as usize <= a);
        (self.at[from..].iter())
            .take_while(|d| d.0 as usize <= z)
            .any(|d| d.1 < a as i64)
    }

    /// A directive lies between two parts of the group opened at token `i`.
    pub fn crosses_in(&self, i: usize, groups: &Groups) -> bool {
        let Some(last) = groups.last(i) else {
            return false;
        };
        let from = self.at.partition_point(|d| d.0 as usize <= i);
        (self.at[from..].iter())
            .take_while(|d| d.0 as usize <= last)
            .any(|d| d.1 == i as i64)
    }
}

/// The blocks around the position being read, each with the label (`case`) open in it: a label
/// covers what follows it up to the next label or the end of the block.
pub struct Extents<P> {
    /// The end, the payload and the open label of each enclosing block.
    stack: Vec<(u32, P, Option<u32>)>,
}

impl<P> Default for Extents<P> {
    fn default() -> Self {
        Extents { stack: Vec::new() }
    }
}

impl<P> Extents<P> {
    /// Leaves the blocks that end at or before `start`; `close(label, end)` is called for the
    /// label open in each.
    pub fn leave(&mut self, start: u32, mut close: impl FnMut(u32, u32)) {
        while let Some(&(end, _, label)) = self.stack.last()
            && end <= start
        {
            if let Some(label) = label {
                close(label, end);
            }
            self.stack.pop();
        }
    }

    /// Enters a block that ends at `end`.
    pub fn enter(&mut self, end: u32, payload: P) {
        self.stack.push((end, payload, None));
    }

    /// The payload of the innermost block.
    pub fn top(&self) -> Option<&P> {
        self.stack.last().map(|s| &s.1)
    }

    /// A label (node `label`) starts at `start` in the innermost block: the open one ends there.
    pub fn label(&mut self, label: u32, start: u32, mut close: impl FnMut(u32, u32)) {
        if let Some(top) = self.stack.last_mut() {
            if let Some(open) = top.2 {
                close(open, start);
            }
            top.2 = Some(label);
        }
    }

    /// The end of the file: leaves every block.
    pub fn finish(&mut self, mut close: impl FnMut(u32, u32)) {
        while let Some((end, _, label)) = self.stack.pop() {
            if let Some(label) = label {
                close(label, end);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_open_outer_first_and_keep_order_on_ties() {
        let mut v = vec![(5, 6, 'a'), (0, 4, 'b'), (0, 9, 'c'), (0, 9, 'd')];
        sort_nested(&mut v, |n| (n.0, n.1, 0u8));
        assert_eq!(v.iter().map(|n| n.2).collect::<String>(), "cdba");
    }

    #[test]
    fn rank_breaks_ties() {
        let mut v = vec![(0, 3, 1), (0, 3, 0)];
        sort_nested(&mut v, |n| (n.0, n.1, n.2));
        assert_eq!(v, vec![(0, 3, 0), (0, 3, 1)]);
    }

    // Tokens: `( a ( b ) c`: the outer group is unclosed and ends at the inner closer.
    fn toks() -> Vec<(u32, u32)> {
        (0..5).map(|i| (i * 2, i * 2 + 1)).collect()
    }

    #[test]
    fn groups_pair_and_report_unclosed() {
        let g = Groups::new(&toks(), [(0, 0, 9), (1, 4, 7)].into_iter());
        assert_eq!(g.last(0), Some(4));
        assert_eq!(g.last(2), Some(3));
        assert_eq!(g.node(2), Some(1));
        assert_eq!(g.ender(3), Some(2));
        assert_eq!(g.skip(2), 4);
        assert_eq!(g.skip(1), 2);
        let closer = |i: usize| i == 3 || i == 4;
        assert!(g.closed(2, closer));
        assert!(g.closed(0, closer));
        assert_eq!(g.inner(2, closer), (3, 3));
        // The outer group's last token is also the ender of itself here.
        let g = Groups::new(&toks(), [(0, 0, 7)].into_iter());
        assert!(!g.closed(0, |_| false));
        assert_eq!(g.inner(0, |_| false), (1, 4));
    }

    #[test]
    fn a_group_over_no_token_is_ignored() {
        let g = Groups::new(&toks(), [(0, 1, 1), (1, 40, 50)].into_iter());
        assert!(!g.is_open(0) && !g.is_open(1));
    }

    #[test]
    fn directives_cross_ranges_but_not_whole_groups() {
        // tokens: 0 `(`  1 a  2 `)`  3 b ; directive A before token 1 (in the group at 0),
        // directive B before token 3 (outside any group).
        let tokens: Vec<(u32, u32)> = (0..4).map(|i| (i * 2, i * 2 + 1)).collect();
        let g = Groups::new(&tokens, [(0, 0, 5)].into_iter());
        let marks = [0, 1, 0, 0, 2, 0];
        let d = Directives::new(marks.into_iter(), &g);
        assert!(d.crosses_in(0, &g));
        // A node over `(` .. `)` is cut by directive A (it lies between two parts of the group).
        assert!(!d.crosses(0, 2));
        // A node over `a`, `)`, `b`: directive A is outside it (before `a`), directive B inside.
        assert!(d.crosses(1, 3));
        // `b` alone: nothing crosses.
        assert!(!d.crosses(3, 3));
    }

    #[test]
    fn labels_end_at_the_next_label_or_the_block_end() {
        let mut closed = Vec::new();
        let mut x: Extents<u8> = Extents::default();
        x.enter(100, 7);
        assert_eq!(x.top(), Some(&7));
        x.label(1, 10, |l, e| closed.push((l, e)));
        x.label(2, 20, |l, e| closed.push((l, e)));
        x.leave(50, |l, e| closed.push((l, e)));
        assert_eq!(closed, vec![(1, 20)]);
        x.leave(100, |l, e| closed.push((l, e)));
        assert_eq!(closed, vec![(1, 20), (2, 100)]);
        assert!(x.top().is_none());
        x.enter(5, 0);
        x.label(3, 1, |_, _| unreachable!());
        x.finish(|l, e| closed.push((l, e)));
        assert_eq!(closed.last(), Some(&(3, 5)));
    }
}
