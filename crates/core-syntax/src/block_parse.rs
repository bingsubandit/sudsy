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
    /// The opener must start a statement.
    pub statement_start: bool,
    /// Words that, written before the opener, make it the end of another block (`END LOOP`).
    pub not_after: &'static [&'static str],
    /// Words that, written after the opener, make it something else (`BEGIN WORK`); the empty
    /// string is the end of the file.
    pub not_before: &'static [&'static str],
    /// The body starts after this later word (`DO` of `WHILE`, `THEN` of `IF`) instead of after
    /// the opener; a condition stands before it.
    pub body_after: Option<&'static str>,
    /// A word that ends the body without an `END` (`UNTIL` of `REPEAT`); `END <open>` follows.
    pub until: Option<&'static str>,
    /// The arm words of the block, each starting a branch: the first one indents the arms, the
    /// body of every arm is indented further.
    pub arms: &'static [Arm],
    /// The arms hang one level under the block (`WHEN` under `CASE`); the bodies of the arms are
    /// indented in both cases.
    pub arm_indent: bool,
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
        statement_start: false,
        not_after: &[],
        not_before: &[],
        body_after: None,
        until: None,
        arms: &[],
        arm_indent: false,
    };

    /// Does the opener, written before the word `next`, open this block?
    pub fn admits(&self, next: &str) -> bool {
        !self.not_before.iter().any(|w| next.eq_ignore_ascii_case(w))
    }
}

/// The parser of a language as the shared block parse needs it.
pub trait Cursor {
    /// The text of the token `n` ahead (empty at the end).
    fn text_at(&self, n: usize) -> &str;
    fn at_end(&self) -> bool;
    /// The token at the cursor ends a statement (`;`).
    fn at_separator(&self) -> bool;
    /// A label (`name:`) is at the cursor.
    fn at_label(&self) -> bool;
    /// Takes the label and its colon.
    fn bump_label(&mut self);
    /// Takes the token at the cursor as a keyword.
    fn bump_keyword(&mut self);
    /// Takes the token at the cursor, whatever it is.
    fn bump_any(&mut self);
    fn expression(&mut self);
    /// One statement, as a node of its own.
    fn statement(&mut self);
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

/// Statements up to the next arm word of `f` or its closer `end <open>`.
fn body<C: Cursor + ?Sized>(c: &mut C, f: &Frame, end: &str) {
    while !c.at_end()
        && !f.arms.iter().any(|a| at(c, 0, a.word))
        && !(at(c, 0, end) && at(c, 1, f.open))
    {
        if c.at_separator() {
            c.bump_any();
            continue;
        }
        let before = c.position();
        c.statement();
        if c.position() == before {
            c.bump_any();
        }
    }
}

/// A statement with arms: the opener, its condition or subject, the arms with their bodies and
/// the closer `end <open>`.
pub fn arm_block<C: Cursor + ?Sized>(c: &mut C, f: &Frame, end: &str) {
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

/// A flat block opener (`[label:] BEGIN`, `LOOP`, `REPEAT`, `WHILE cond DO`): the opener alone,
/// and the statement written after it on the same line is a statement of its own. False when no
/// block of `frames` opens at the cursor.
pub fn block_opener<C: Cursor + ?Sized>(c: &mut C, frames: &[Frame], end: &str) -> bool {
    let n = if c.at_label() { 2 } else { 0 };
    let Some(f) = frames
        .iter()
        .find(|f| f.arms.is_empty() && at(c, n, f.open) && f.admits(c.text_at(n + 1)))
    else {
        return false;
    };
    if n > 0 {
        c.bump_label();
    }
    c.bump_keyword();
    condition(c, f.body_after);
    if !c.at_end() && !c.at_separator() && !at(c, 0, end) {
        c.statement();
    }
    true
}
