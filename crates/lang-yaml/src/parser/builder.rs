//! Turns the parser's events into the lossless tree. The parser only reports real tokens; the
//! text between two of them is trivia (blanks, line breaks, comments), split by
//! `provider_api::adapter::Stream` into trailing trivia of the previous token and leading trivia of
//! the next one.

use core_syntax::{SyntaxKind, SyntaxNode, TreeBuilder, grammars};
use provider_api::ParseError;
use provider_api::adapter::{Stream, Tok};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Ev {
    Start(SyntaxKind),
    Tok(SyntaxKind, usize, usize),
    Finish,
}

pub(crate) fn build(
    src: &str,
    events: &[Ev],
    mut errors: Vec<ParseError>,
) -> (SyntaxNode, Vec<ParseError>) {
    let mut starts = events.iter().filter_map(|e| match *e {
        Ev::Tok(_, s, _) => Some(s as u32),
        _ => None,
    });
    let first = starts.next().unwrap_or(src.len() as u32);
    let mut stream = Stream::of(src, grammars::YAML.get(), first).joining_tabs();
    let mut b = TreeBuilder::new();
    for ev in events {
        match *ev {
            Ev::Start(kind) => b.start_node(kind),
            Ev::Finish => b.finish_node(),
            Ev::Tok(kind, start, end) => {
                let tok = Tok {
                    kind,
                    start: start as u32,
                    end: end as u32,
                };
                let next = starts.next().unwrap_or(src.len() as u32);
                stream.token(&mut b, tok, next);
            }
        }
    }
    errors.append(&mut stream.gap.errors);
    (SyntaxNode::new_root(b.finish()), errors)
}
