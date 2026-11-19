//! The check that a formatted item means what the source did: both are parsed and must have the
//! same nodes, tokens and comments in the same order. An item that fails is kept as written.
//!
//! concept: none (meaning check by re-parse; local until a shared checker takes a language parse)

use crate::generated::kind as k;
use crate::{Dialect, parse};
use rowan::WalkEvent;

/// The nodes and non-blank tokens of `text`, in order.
fn signature(text: &str, dialect: Dialect) -> Vec<String> {
    let tree = parse(text, dialect).tree;
    let mut out = Vec::new();
    for ev in tree.preorder_with_tokens() {
        match ev {
            WalkEvent::Enter(rowan::NodeOrToken::Node(n)) => {
                out.push(format!("<{}", n.kind().0));
            }
            WalkEvent::Leave(rowan::NodeOrToken::Node(_)) => {
                out.push(">".into());
            }
            WalkEvent::Enter(rowan::NodeOrToken::Token(t)) => {
                let kind = t.kind();
                if kind == k::WHITESPACE || kind == k::NEWLINE {
                    continue;
                }
                let lines: Vec<&str> = t.text().lines().map(str::trim).collect();
                out.push(format!("{}:{}", kind.0, lines.join("\n")));
            }
            _ => {}
        }
    }
    out
}

pub fn same(source: &str, output: &str, dialect: Dialect) -> bool {
    let (a, b) = (signature(source, dialect), signature(output, dialect));
    a == b
}
