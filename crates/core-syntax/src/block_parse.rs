//! The parse of statement blocks written with keywords: a block that opens at a word (`BEGIN`,
//! `LOOP`, `WHILE cond DO`, `REPEAT`, optionally labelled) and a statement with arms
//! (`IF cond THEN .. ELSEIF .. ELSE .. END IF`, `CASE [expr] WHEN cond THEN .. ELSE .. END CASE`).
//! The language gives its words as [`Frame`]s and a [`Cursor`] over its parser; the shared code
//! decides where the arms and bodies end, what the closer is and where a label and the statement
//! after an opener go.
//!
//! concept: keyword-block-parse
//! what: Parse of keyword blocks and arm statements from opener, arm and closer words.
//! tools: sqlfluff LT02
//! reads: path:block_parse::Frame

/// A block that opens at a word. The body starts after the opener, after `body_after`, or, for
/// blocks with arms and no `body_after`, at the first arm.
pub struct Frame {
    pub open: &'static str,
    /// The opener word is also a function or an expression (`IF()`, `REPEAT()`, `CASE`): it opens
    /// a block only at the start of a statement.
    pub also_expression: bool,
    /// The body starts after this later word (`DO` of `WHILE`, `THEN` of `IF`) instead of after
    /// the opener; a condition stands before it.
    pub body_after: Option<&'static str>,
    /// A word that ends the body without an `END` (`UNTIL` of `REPEAT`); `END <open>` follows.
    pub until: Option<&'static str>,
    /// The arm words of the block, each starting a branch: the first one indents the arms, the
    /// body of every arm is indented further.
    pub arms: &'static [Arm],
}

/// A branch of a block with arms.
pub struct Arm {
    pub word: &'static str,
    /// A condition stands after the arm word and this word starts the body (`THEN` of `WHEN`);
    /// none: the body starts after `word` itself.
    pub body_after: Option<&'static str>,
}

impl Frame {
    /// A frame with nothing set, for `..Frame::NONE` in a table.
    pub const NONE: Frame = Frame {
        open: "",
        also_expression: false,
        body_after: None,
        until: None,
        arms: &[],
    };

    /// The arms hang one level under the block (`WHEN` under `CASE`): its first arm comes before
    /// any body, which is indented in both cases.
    pub fn arms_hang(&self) -> bool {
        self.body_after.is_none() && !self.arms.is_empty()
    }
}

/// The words of the blocks of a language.
pub struct Blocks {
    pub frames: &'static [Frame],
    /// The word that ends a block (`END`).
    pub end: &'static str,
    /// Word runs that are a statement of their own, not an opener (`BEGIN WORK`).
    pub statements: &'static [&'static [&'static str]],
}

impl Blocks {
    /// Does the opener of `f`, word 0 of `word_at`, open a block? Not when it ends the input or
    /// a statement, or when it starts one of the `statements`. `separator_at(n)`: word `n` ends
    /// a statement.
    pub fn admits<'a>(
        &self,
        f: &Frame,
        word_at: impl Fn(usize) -> &'a str,
        separator_at: impl Fn(usize) -> bool,
    ) -> bool {
        !word_at(1).is_empty()
            && !separator_at(1)
            && !self.statements.iter().any(|run| {
                run.first().is_some_and(|w| w.eq_ignore_ascii_case(f.open))
                    && run
                        .iter()
                        .enumerate()
                        .all(|(k, w)| word_at(k).eq_ignore_ascii_case(w))
            })
    }

    /// Words after which a statement starts: the openers of blocks with a plain body, the words
    /// that start a body (`THEN`, `DO`) and the arms with a plain body (`ELSE`).
    pub fn starts_statement_after(&self, word: &str) -> bool {
        self.frames.iter().any(|f| {
            let plain = f.body_after.is_none() && f.arms.is_empty();
            (plain && word.eq_ignore_ascii_case(f.open))
                || f.body_after.is_some_and(|w| word.eq_ignore_ascii_case(w))
                || f.arms
                    .iter()
                    .any(|a| word.eq_ignore_ascii_case(a.body_after.unwrap_or(a.word)))
        })
    }
}

/// The parser of a language as the shared block parse needs it.
pub trait Cursor {
    /// The text of the token `n` ahead (empty at the end).
    fn text_at(&self, n: usize) -> &str;
    fn at_end(&self) -> bool;
    /// The token `n` ahead ends a statement (`;`).
    fn separator_at(&self, n: usize) -> bool;
    /// The token at the cursor closes the bracket the statements stand in (`)`).
    fn at_close(&self) -> bool;
    /// A label (`name:`) is at the cursor.
    fn at_label(&self) -> bool;
    /// Takes the label and its colon.
    fn bump_label(&mut self);
    /// Takes the token at the cursor as a keyword.
    fn bump_keyword(&mut self);
    /// Takes the token at the cursor, whatever it is.
    fn bump_any(&mut self);
    fn expression(&mut self);
    /// One statement, as a node of its own; `bracketed` when a bracket holds it.
    fn statement(&mut self, bracketed: bool);
    /// How far the cursor is, to see whether a statement took anything.
    fn position(&self) -> usize;
}

fn at<C: Cursor + ?Sized>(c: &C, n: usize, word: &str) -> bool {
    c.text_at(n).eq_ignore_ascii_case(word)
}

/// A condition when the arm or opener has one, and the word that starts the body.
fn condition<C: Cursor + ?Sized>(c: &mut C, body_after: Option<&str>) {
    if let Some(word) = body_after {
        c.expression();
        if at(c, 0, word) {
            c.bump_keyword();
        }
    }
}

/// Statements up to the `stop` condition or the end; a bracketed list stops at the bracket that
/// closes it.
fn statements<C: Cursor + ?Sized>(c: &mut C, bracketed: bool, stop: impl Fn(&C) -> bool) {
    while !c.at_end() && !stop(c) {
        if c.separator_at(0) {
            c.bump_any();
            continue;
        }
        let before = c.position();
        c.statement(bracketed);
        if c.position() == before {
            c.bump_any();
        }
    }
}

/// Statements up to the next arm word of `f` or its closer `end <open>`.
fn body<C: Cursor + ?Sized>(c: &mut C, f: &Frame, end: &str) {
    statements(c, false, |c| {
        f.arms.iter().any(|a| at(c, 0, a.word)) || (at(c, 0, end) && at(c, 1, f.open))
    });
}

/// The statements of a bracket (`DO INSTEAD (a; b)`), up to its closing bracket or the end.
pub fn statement_list<C: Cursor + ?Sized>(c: &mut C) {
    statements(c, true, |c| c.at_close());
}

/// A statement with arms: the opener, its condition or subject, the arms with their bodies and
/// the closer `end <open>`.
fn arm_block<C: Cursor + ?Sized>(c: &mut C, f: &Frame, end: &str) {
    c.bump_keyword();
    if f.body_after.is_some() {
        condition(c, f.body_after);
        body(c, f, end);
    } else if !at(c, 0, f.arms[0].word) {
        c.expression();
    }
    while let Some(arm) = f.arms.iter().find(|a| at(c, 0, a.word)) {
        c.bump_keyword();
        condition(c, arm.body_after);
        body(c, f, end);
    }
    if at(c, 0, end) && at(c, 1, f.open) {
        c.bump_keyword();
        c.bump_keyword();
    }
}

/// The frame of `blocks` that opens at the token `n` ahead, with or without arms.
fn opening<'b, C: Cursor + ?Sized>(
    c: &C,
    blocks: &'b Blocks,
    n: usize,
    arms: bool,
) -> Option<&'b Frame> {
    blocks.frames.iter().find(|f| {
        f.arms.is_empty() != arms
            && at(c, n, f.open)
            && blocks.admits(f, |k| c.text_at(n + k), |k| c.separator_at(n + k))
    })
}

/// A flat block opener (`[label:] BEGIN`, `LOOP`, `REPEAT`, `WHILE cond DO`): the opener alone,
/// and the statement written after it on the same line is a statement of its own. False when no
/// block of `blocks` opens at the cursor.
fn block_opener<C: Cursor + ?Sized>(c: &mut C, blocks: &Blocks) -> bool {
    let n = if c.at_label() { 2 } else { 0 };
    let Some(f) = opening(c, blocks, n, false) else {
        return false;
    };
    if n > 0 {
        c.bump_label();
    }
    c.bump_keyword();
    condition(c, f.body_after);
    if !c.at_end() && !c.separator_at(0) && !at(c, 0, blocks.end) {
        c.statement(false);
    }
    true
}

/// One block statement of `blocks` at the cursor: a flat block opener or a statement with arms,
/// whichever frame opens here. False when none does, and nothing is taken.
pub fn statement<C: Cursor + ?Sized>(c: &mut C, blocks: &Blocks) -> bool {
    if block_opener(c, blocks) {
        return true;
    }
    let Some(f) = opening(c, blocks, 0, true) else {
        return false;
    };
    arm_block(c, f, blocks.end);
    true
}
