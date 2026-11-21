//! Indentation of statement blocks that a parser leaves as flat keyword runs: the indent and
//! dedent points of blocks that open at a keyword (`BEGIN`, `LOOP`, `WHILE .. DO`, `REPEAT`),
//! of statements with arms (`CASE .. WHEN .. THEN .. ELSE .. END CASE`) and of a statement whose
//! actions hang under its head (`ALTER TABLE name`). The language gives the words as a [`Spec`];
//! the nesting, the depth and the points are computed here.
//!
//! concept: keyword-block-frames
//! what: Indent and dedent points of flat keyword blocks, arm statements and hanging action lists.
//! tools: sqlfluff LT02
//! reads: path:block_frames::Spec

pub use core_syntax::block_parse::{Arm, Frame};

/// A token of the file as the frames see it.
#[derive(Clone, Copy)]
pub struct Tok<'a> {
    /// The text of the token.
    pub text: &'a str,
    /// A word of the language's own keywords.
    pub keyword: bool,
    /// A token that ends a statement or labels one (`;`, `:`).
    pub separator: bool,
    /// A name or a part of a dotted name.
    pub name: NameKind,
}

/// What a token is inside a name.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    None,
    Part,
    Dot,
}

/// A statement whose actions hang under its head: the words of the head, the optional words
/// after it and then one dotted name; the actions up to the end of the statement are indented.
pub struct Hanging {
    pub head: &'static [&'static str],
    pub optional: &'static [&'static str],
}

pub struct Spec {
    /// Words after which a statement starts (a separator token starts one too).
    pub statement_after: &'static [&'static str],
    pub frames: &'static [Frame],
    /// The word that ends a block.
    pub end: &'static str,
    /// Hanging statements, used when the caller asks for them.
    pub hanging: &'static [Hanging],
}

struct Open {
    frame: &'static Frame,
    /// An arm indent is open.
    arms: bool,
    /// The body of an arm is open.
    body: bool,
    /// The word that opens the body of the arm just started.
    waiting: Option<&'static str>,
}

struct Walk<'a, 'b> {
    toks: &'a [Tok<'b>],
    spec: &'static Spec,
    out: Vec<(usize, i8)>,
}

impl Walk<'_, '_> {
    fn is(&self, i: usize, word: &str) -> bool {
        self.toks
            .get(i)
            .is_some_and(|t| t.text.eq_ignore_ascii_case(word))
    }

    fn word(&self, i: usize) -> &str {
        self.toks.get(i).map_or("", |t| t.text)
    }

    fn starts_statement(&self, i: usize) -> bool {
        i == 0
            || self.toks[i - 1].separator
            || self.spec.statement_after.iter().any(|w| self.is(i - 1, w))
    }

    fn opens(&self, f: &Frame, i: usize, stmt: bool) -> bool {
        (!f.statement_start || stmt)
            && !(i > 0 && f.not_after.iter().any(|w| self.is(i - 1, w)))
            && !f
                .not_before
                .iter()
                .any(|w| self.word(i + 1).eq_ignore_ascii_case(w))
    }

    /// The arm word of the innermost open block that starts at `i`, if any.
    fn arm(&mut self, stack: &mut [Open], i: usize, stmt: bool) -> bool {
        let Some(top) = stack.last_mut() else {
            return false;
        };
        if top.frame.arms.is_empty() {
            return false;
        }
        if let Some(w) = top.waiting
            && !top.body
            && self.toks[i].text.eq_ignore_ascii_case(w)
        {
            self.out.push((i + 1, 1));
            top.body = true;
            top.waiting = None;
            return true;
        }
        let first = top.frame.arm_indent
            && !top.arms
            && self.toks[i]
                .text
                .eq_ignore_ascii_case(top.frame.arms[0].word);
        let Some(arm) = top
            .frame
            .arms
            .iter()
            .find(|a| self.toks[i].text.eq_ignore_ascii_case(a.word))
        else {
            return false;
        };
        if !(stmt || first) {
            return false;
        }
        if top.body {
            self.out.push((i, -1));
            top.body = false;
        }
        if top.frame.arm_indent && !top.arms {
            self.out.push((i, 1));
            top.arms = true;
        }
        match arm.body_after {
            None => {
                self.out.push((i + 1, 1));
                top.body = true;
            }
            Some(w) => top.waiting = Some(w),
        }
        true
    }

    fn close(&mut self, open: &Open, at: usize) {
        if open.body {
            self.out.push((at, -1));
        }
        if open.arms {
            self.out.push((at, -1));
        }
    }

    /// The end of the name that follows the head of a hanging statement at `i`, if there is one.
    fn hanging(&self, h: &Hanging, i: usize) -> Option<usize> {
        let mut j = i;
        for w in h.head {
            if !self.is(j, w) {
                return None;
            }
            j += 1;
        }
        while h.optional.iter().any(|w| self.is(j, w)) {
            j += 1;
        }
        if self.toks.get(j)?.name != NameKind::Part {
            return None;
        }
        j += 1;
        while self.toks.get(j).is_some_and(|t| t.name == NameKind::Dot)
            && self
                .toks
                .get(j + 1)
                .is_some_and(|t| t.name == NameKind::Part)
        {
            j += 2;
        }
        Some(j)
    }

    fn run(&mut self, blocks: bool, hanging: bool) {
        let mut stack: Vec<Open> = Vec::new();
        let mut pending: Option<&'static Frame> = None;
        let mut after_until: Option<&'static str> = None;
        let mut hang = false;
        for i in 0..self.toks.len() {
            let t = self.toks[i];
            if hang && t.separator && t.text == ";" || hang && i + 1 == self.toks.len() {
                self.out.push((i, -1));
                hang = false;
            }
            if !t.keyword {
                continue;
            }
            let stmt = self.starts_statement(i);
            if !blocks && !hanging {
                return;
            }
            if blocks && self.arm(&mut stack, i, stmt) {
                continue;
            }
            if hanging && stmt && !hang {
                let spec = self.spec;
                for h in spec.hanging {
                    if let Some(j) = self.hanging(h, i)
                        && j < self.toks.len()
                        && !self.toks[j].separator
                        && j + 1 < self.toks.len()
                    {
                        self.out.push((j, 1));
                        hang = true;
                    }
                }
            }
            if !blocks {
                continue;
            }
            let spec = self.spec;
            if t.text.eq_ignore_ascii_case(spec.end) && stmt {
                if after_until.is_some_and(|w| self.is(i + 1, w)) {
                    after_until = None;
                } else if let Some(open) = stack.pop() {
                    self.close(&open, i);
                }
                continue;
            }
            if let Some(top) = stack.last()
                && stmt
                && top
                    .frame
                    .until
                    .is_some_and(|w| t.text.eq_ignore_ascii_case(w))
            {
                let open = stack.pop().expect("a frame is open");
                self.out.push((i, -1));
                after_until = Some(open.frame.open);
                continue;
            }
            if pending.is_some_and(|f| f.body_after.is_some_and(|w| t.text.eq_ignore_ascii_case(w)))
            {
                let frame = pending.take().expect("a frame is pending");
                stack.push(Open {
                    frame,
                    arms: false,
                    body: true,
                    waiting: None,
                });
                self.out.push((i + 1, 1));
                continue;
            }
            for f in spec.frames {
                if t.text.eq_ignore_ascii_case(f.open) && self.opens(f, i, stmt) {
                    if f.body_after.is_some() {
                        pending = Some(f);
                    } else {
                        let arms = !f.arms.is_empty();
                        stack.push(Open {
                            frame: f,
                            arms: false,
                            body: !arms,
                            waiting: None,
                        });
                        if !arms {
                            self.out.push((i + 1, 1));
                        }
                    }
                    break;
                }
            }
        }
    }
}

/// The indent (`1`) and dedent (`-1`) points of `toks`: `(n, d)` puts `d` on the point before
/// token `n`. `blocks` computes those of the frames of the spec, `hanging` those of its hanging
/// statements.
pub fn points(
    toks: &[Tok<'_>],
    spec: &'static Spec,
    blocks: bool,
    hanging: bool,
) -> Vec<(usize, i8)> {
    let mut walk = Walk {
        toks,
        spec,
        out: Vec::new(),
    };
    walk.run(blocks, hanging);
    walk.out
}

#[cfg(test)]
mod tests {
    use super::*;

    static SPEC: Spec = Spec {
        statement_after: &["THEN", "ELSE"],
        frames: &[
            Frame {
                open: "BEGIN",
                ..Frame::NONE
            },
            Frame {
                open: "CASE",
                statement_start: true,
                arms: &[
                    Arm {
                        word: "WHEN",
                        body_after: Some("THEN"),
                    },
                    Arm {
                        word: "ELSE",
                        body_after: None,
                    },
                ],
                arm_indent: true,
                ..Frame::NONE
            },
        ],
        end: "END",
        hanging: &[Hanging {
            head: &["ALTER", "TABLE"],
            optional: &[],
        }],
    };

    /// Upper-case words are keywords, lower-case ones names, `.` a dot, `;` a separator.
    fn toks(src: &str) -> Vec<Tok<'_>> {
        let mut v: Vec<Tok> = src
            .split_whitespace()
            .map(|w| Tok {
                text: w,
                keyword: w.chars().all(|c| c.is_ascii_uppercase()),
                separator: w == ";",
                name: if w == "." {
                    NameKind::Dot
                } else if w.chars().all(|c| c.is_ascii_lowercase()) {
                    NameKind::Part
                } else {
                    NameKind::None
                },
            })
            .collect();
        v.push(Tok {
            text: "",
            keyword: false,
            separator: false,
            name: NameKind::None,
        });
        v
    }

    #[test]
    fn case_arms_and_bodies() {
        let t = toks("CASE x WHEN THEN a ; ELSE b ; END CASE ;");
        assert_eq!(
            points(&t, &SPEC, true, false),
            [(2, 1), (4, 1), (6, -1), (7, 1), (9, -1), (9, -1)]
        );
    }

    #[test]
    fn begin_end_and_hanging() {
        let t = toks("BEGIN a ; END ;");
        assert_eq!(points(&t, &SPEC, true, false), [(1, 1), (3, -1)]);
        let t = toks("ALTER TABLE t . u ADD c ;");
        assert_eq!(points(&t, &SPEC, false, true), [(5, 1), (7, -1)]);
    }
}
