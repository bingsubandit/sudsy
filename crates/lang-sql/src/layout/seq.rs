//! The file as the layout sees it: blocks (code and comments) with the points between them.
//!
//! A point holds what lies between two blocks: line breaks, the blanks after the last one (the
//! indentation of the next line, or the gap on one line) and the indentation structure the
//! parser put there (`Indent` and `Dedent`). `points[i]` precedes `blocks[i]`; the last block is
//! the end of the file. Nodes keep their first and last code block, so a rule can ask whether a
//! block starts or ends one.

use crate::generated::kind as k;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode};
use std::borrow::Cow;

pub const NONE: u32 = u32::MAX;

#[derive(Clone, Default)]
pub struct Point {
    pub newlines: u32,
    /// The blanks after the last line break, or the whole gap when there is none.
    pub ws: Cow<'static, str>,
    /// +1 per `Indent`, -1 per `Dedent`, in order.
    pub metas: Vec<i8>,
    /// An implicit indent is among the metas.
    pub implicit: bool,
}

impl Point {
    /// The net indentation change and the lowest point it reaches (never above 0).
    pub fn stats(&self) -> (i32, i32) {
        let (mut sum, mut trough) = (0, 0);
        for &m in &self.metas {
            sum += i32::from(m);
            trough = trough.min(sum);
        }
        (sum, trough)
    }

    pub fn set_ws(&mut self, ws: &'static str) {
        self.ws = Cow::Borrowed(ws);
    }
}

pub struct Block {
    pub kind: SyntaxKind,
    pub text: String,
    pub node: u32,
}

pub struct Node {
    pub kind: SyntaxKind,
    pub parent: u32,
    /// First and last code block (not comments).
    pub first: u32,
    pub last: u32,
    /// The index after the node's subtree (nodes are in pre-order).
    pub end: u32,
}

pub struct Seq {
    pub points: Vec<Point>,
    pub blocks: Vec<Block>,
    pub nodes: Vec<Node>,
}

pub fn is_comment(kind: SyntaxKind) -> bool {
    kind == k::LINE_COMMENT || kind == k::BLOCK_COMMENT
}

impl Seq {
    pub fn build(root: &SyntaxNode, grammar: &Grammar) -> Seq {
        let mut seq = Seq {
            points: Vec::new(),
            blocks: Vec::new(),
            nodes: Vec::new(),
        };
        let mut point = Point::default();
        let mut stack: Vec<(u32, Option<SyntaxElement>)> = Vec::new();
        let mut last_code = NONE;
        seq.open(root.kind(), &mut stack, root);
        while let Some((node, next)) = stack.last_mut() {
            let len = seq.nodes.len() as u32;
            let Some(child) = next.take() else {
                let n = &mut seq.nodes[*node as usize];
                n.end = len;
                if n.first != NONE {
                    n.last = last_code;
                }
                stack.pop();
                continue;
            };
            *next = child.next_sibling_or_token();
            match child {
                SyntaxElement::Node(n) => seq.open(n.kind(), &mut stack, &n),
                SyntaxElement::Token(t) => {
                    let kind = t.kind();
                    let text = t.text();
                    if kind == k::WHITESPACE {
                        point.ws = if text == " " {
                            Cow::Borrowed(" ")
                        } else {
                            Cow::Owned(text.to_string())
                        };
                    } else if kind == k::NEWLINE {
                        point.newlines += 1;
                        point.ws = Cow::Borrowed("");
                    } else if kind == k::INDENT || kind == k::IMPLICIT_INDENT {
                        point.metas.push(1);
                        point.implicit |= kind == k::IMPLICIT_INDENT;
                    } else if kind == k::DEDENT {
                        point.metas.push(-1);
                    } else {
                        debug_assert!(!grammar.is_trivia(kind) || is_comment(kind));
                        let at = seq.blocks.len() as u32;
                        let owner = stack.last().map_or(NONE, |s| s.0);
                        if !is_comment(kind) {
                            let mut n = owner;
                            while n != NONE && seq.nodes[n as usize].first == NONE {
                                seq.nodes[n as usize].first = at;
                                n = seq.nodes[n as usize].parent;
                            }
                            last_code = at;
                        }
                        seq.points.push(std::mem::take(&mut point));
                        seq.blocks.push(Block {
                            kind,
                            text: text.to_string(),
                            node: owner,
                        });
                    }
                }
            }
        }
        seq
    }

    fn open(
        &mut self,
        kind: SyntaxKind,
        stack: &mut Vec<(u32, Option<SyntaxElement>)>,
        n: &SyntaxNode,
    ) {
        let parent = stack.last().map_or(NONE, |s| s.0);
        let id = self.nodes.len() as u32;
        self.nodes.push(Node {
            kind,
            parent,
            first: NONE,
            last: NONE,
            end: 0,
        });
        stack.push((id, n.first_child_or_token()));
    }

    /// The child nodes of `node`, by index.
    pub fn children(&self, node: u32) -> impl Iterator<Item = u32> + '_ {
        let end = self.nodes[node as usize].end;
        let mut j = node + 1;
        std::iter::from_fn(move || {
            (j < end).then(|| {
                let at = j;
                j = self.nodes[at as usize].end;
                at
            })
        })
    }

    pub fn is_comment(&self, i: usize) -> bool {
        is_comment(self.blocks[i].kind)
    }

    pub fn is_eof(&self, i: usize) -> bool {
        self.blocks[i].kind == k::EOF
    }

    /// The innermost node that holds both blocks.
    pub fn common_node(&self, a: usize, b: usize) -> u32 {
        let (mut x, mut y) = (self.blocks[a].node, self.blocks[b].node);
        let depth = |mut n: u32| {
            let mut d = 0;
            while n != NONE {
                n = self.nodes[n as usize].parent;
                d += 1;
            }
            d
        };
        let (mut dx, mut dy) = (depth(x), depth(y));
        while dx > dy {
            x = self.nodes[x as usize].parent;
            dx -= 1;
        }
        while dy > dx {
            y = self.nodes[y as usize].parent;
            dy -= 1;
        }
        while x != y {
            x = self.nodes[x as usize].parent;
            y = self.nodes[y as usize].parent;
        }
        x
    }

    /// Does `node` have a line break between its first and last code block?
    pub fn spans_lines(&self, node: u32) -> bool {
        let n = &self.nodes[node as usize];
        n.first != NONE && (n.first + 1..=n.last).any(|i| self.points[i as usize].newlines > 0)
    }
}
