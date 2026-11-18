//! Text and inline children packed to the line width: words and separators in a `fill`. Two forms.
//! [`fill_words`] is a text run (words joined by a line, a prefix on the first word and a suffix on
//! the last). [`fill_children`] is JSX's rule for the children of an element (Prettier's
//! `printJsxElement`): text split into words, nodes between them, the separators between a word
//! and a node chosen by the whitespace the source has, the doubled separators removed, and the
//! space that a line break would lose written as a marker (`{" "}`) before the break.
//!
//! concept: text-fill
//! what: Words and separators laid out in a fill: JSX children, inline markup text.
//! tools: Prettier fill, Prettier printJsxElement

use crate::doc::{self, Doc};

/// Whitespace of the children's text: blank, newline, carriage return and tab.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\r' | '\t')
}

/// The words of `text` (split where `split` holds) as a fill, a line between them; `prefix`
/// goes before the first word and `suffix` after the last. No words, no fill.
pub fn fill_words(
    text: &str,
    split: impl Fn(char) -> bool,
    prefix: &str,
    suffix: &str,
) -> Doc {
    let words: Vec<&str> = text.split(split).filter(|w| !w.is_empty()).collect();
    let last = words.len().saturating_sub(1);
    let mut parts = Vec::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            parts.push(doc::line());
        }
        let mut word = String::new();
        if i == 0 {
            word.push_str(prefix);
        }
        word.push_str(w);
        if i == last {
            word.push_str(suffix);
        }
        parts.push(doc::text(word));
    }
    doc::fill(parts)
}

/// One piece of an element's children: the source text between nodes, or a printed node.
pub enum Child {
    Text(String),
    /// A printed child. `self_closing`: an element with no content, next to which a word of more
    /// than one character goes on a line of its own.
    Node { doc: Doc, self_closing: bool },
}

/// A part of the children list: a word or printed node (content), or what separates them.
#[derive(Clone)]
enum Part {
    Empty,
    Hard,
    Soft,
    Line,
    /// Whitespace that counts: a space, or the marker and a line when the line breaks.
    Space,
    Word(String),
    Doc(Doc),
}

/// What [`fill_children`] makes of the children.
pub struct Filled {
    /// The children one level in: a fill when there is text, else all on lines of their own.
    pub broken: Doc,
    /// The same parts for the flat candidate (a fit on one line).
    pub flat: Vec<Doc>,
    /// A hard line or a part that breaks is in `broken`.
    pub forced_break: bool,
}

/// Text with a word or a newline-free space in it: whitespace between lines is not.
pub fn meaningful(t: &str) -> bool {
    t.contains(|c| !is_space(c)) || !t.contains('\n')
}

/// The text split at runs of whitespace, the runs kept: words at even places.
fn split_words(t: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_space = false;
    for c in t.chars() {
        if is_space(c) != in_space {
            in_space = !in_space;
            out.push(String::new());
        }
        out.last_mut().expect("never empty").push(c);
    }
    if in_space {
        out.push(String::new());
    }
    out
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Between a word or node and the next: a softline, except next to a self-closing element.
fn sep_no_ws(word: &str, this: &Child, next: Option<&Child>) -> Part {
    let closes = |c: &Child| matches!(c, Child::Node { self_closing: true, .. });
    if closes(this) || next.is_some_and(closes) {
        return if utf16_len(word) == 1 {
            Part::Soft
        } else {
            Part::Hard
        };
    }
    Part::Soft
}

fn sep_with_ws(word: &str, this: &Child) -> Part {
    if utf16_len(word) == 1 {
        return match this {
            Child::Text(t) if !t.contains('\n') => Part::Hard,
            _ => Part::Soft,
        };
    }
    Part::Hard
}

fn children_parts(children: Vec<Child>) -> Vec<Part> {
    let mut parts = Vec::new();
    for i in 0..children.len() {
        let (child, next) = (&children[i], children.get(i + 1));
        match child {
            Child::Text(t) if meaningful(t) => {
                let mut words = split_words(t);
                if words[0].is_empty() {
                    parts.push(Part::Empty);
                    words.remove(0);
                    if words[0].contains('\n') {
                        parts.push(sep_with_ws(words.get(1).map_or("", |w| w), child));
                    } else {
                        parts.push(Part::Space);
                    }
                    words.remove(0);
                }
                let mut end_ws = None;
                if words.last().is_some_and(|w| w.is_empty()) {
                    words.pop();
                    end_ws = words.pop();
                }
                if words.is_empty() {
                    continue;
                }
                let last = words.last().cloned().unwrap_or_default();
                for (j, w) in words.into_iter().enumerate() {
                    parts.push(if j % 2 == 1 {
                        Part::Line
                    } else {
                        Part::Word(w)
                    });
                }
                parts.push(match end_ws {
                    Some(e) if e.contains('\n') => sep_with_ws(&last, child),
                    Some(_) => Part::Space,
                    None => sep_no_ws(&last, child, next),
                });
            }
            Child::Text(t) if t.contains('\n') => {
                if t.matches('\n').count() > 1 {
                    parts.extend([Part::Empty, Part::Hard]);
                }
            }
            Child::Text(_) => parts.extend([Part::Empty, Part::Space]),
            Child::Node { doc, .. } => {
                parts.push(Part::Doc(doc.clone()));
                match next {
                    Some(Child::Text(t)) if meaningful(t) => {
                        let first = t
                            .trim_matches(is_space)
                            .split(is_space)
                            .next()
                            .unwrap_or("");
                        parts.push(sep_no_ws(first, child, next));
                    }
                    _ => parts.push(Part::Hard),
                }
            }
        }
    }
    parts
}

/// Removes the doubled and misplaced separators `children_parts` leaves between text and nodes.
fn clean(parts: &mut Vec<Part>, contains_text: bool) {
    use Part::{Empty as E, Hard as H, Soft as S, Space as W};
    let mut i = parts.len() as isize - 2;
    while i >= 0 {
        let u = i as usize;
        let at = |j: usize| parts.get(j);
        let (a, b, c) = (at(u), at(u + 1), at(u + 2));
        let b_empty = matches!(b, Some(E));
        let pair_hard = matches!((a, c), (Some(H), Some(H))) && b_empty;
        let pair_empty = matches!(a, Some(E)) && b_empty;
        let line_ws = matches!(a, Some(H | S)) && b_empty && matches!(c, Some(W));
        let ws_line = matches!(a, Some(W)) && b_empty && matches!(c, Some(H | S));
        let double_ws = matches!(a, Some(W)) && b_empty && matches!(c, Some(W));
        // The hard line wins over a soft one.
        let soft_hard = b_empty && matches!((a, c), (Some(S), Some(H)));
        let hard_soft = b_empty && matches!((a, c), (Some(H), Some(S)));
        if (pair_hard && contains_text) || pair_empty || line_ws || double_ws || soft_hard {
            parts.drain(u..u + 2);
        } else if ws_line || hard_soft {
            parts.drain(u + 1..u + 3);
        }
        i -= 1;
    }
}

fn is_blank(p: &Part) -> bool {
    matches!(p, Part::Empty | Part::Hard | Part::Soft | Part::Line)
}

/// The children of an element as a fill. `marker` is what is written where a space would be lost
/// at a line break (JSX: `{" "}`).
pub fn fill_children(children: Vec<Child>, marker: &str) -> Filled {
    let contains_text = children
        .iter()
        .any(|c| matches!(c, Child::Text(t) if meaningful(t)));
    let mut parts = children_parts(children);
    clean(&mut parts, contains_text);
    while parts.last().is_some_and(is_blank) {
        parts.pop();
    }
    while parts.len() > 1 && is_blank(&parts[0]) && is_blank(&parts[1]) {
        parts.drain(..2);
    }
    let space = || {
        doc::if_break(
            doc::concat(vec![doc::lit(marker), doc::softline()]),
            doc::lit(" "),
        )
    };
    let to_doc = |p: &Part| match p {
        Part::Empty => Doc::Nil,
        Part::Hard => doc::hardline(),
        Part::Soft => doc::softline(),
        Part::Line => doc::line(),
        Part::Space => space(),
        Part::Word(w) => doc::text(w.clone()),
        Part::Doc(d) => d.clone(),
    };
    let mut forced_break = false;
    let mut multiline = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if matches!(p, Part::Space) {
            let before_empty = i == 1 && matches!(parts[0], Part::Empty);
            if before_empty && parts.len() == 2 {
                multiline.push(doc::lit(marker));
                continue;
            }
            if before_empty {
                multiline.push(doc::concat(vec![doc::lit(marker), doc::hardline()]));
                continue;
            }
            if i + 1 == parts.len() {
                // Trailing: measured with the content before it.
                let last = multiline.pop().unwrap_or(Doc::Nil);
                multiline.push(doc::concat(vec![last, doc::lit(marker)]));
                continue;
            }
            if i + 2 == parts.len()
                && matches!(parts[i + 1], Part::Empty)
                && i > 0
                && matches!(parts[i - 1], Part::Hard)
            {
                multiline.push(doc::lit(marker));
                continue;
            }
        }
        multiline.push(to_doc(p));
        forced_break |= match p {
            Part::Hard => true,
            Part::Doc(d) => d.will_break(),
            _ => false,
        };
    }
    let broken = if contains_text {
        doc::fill(multiline)
    } else {
        doc::group_broken(doc::concat(multiline))
    };
    Filled {
        broken,
        flat: parts.iter().map(to_doc).collect(),
        forced_break,
    }
}
