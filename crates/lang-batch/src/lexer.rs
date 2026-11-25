//! The lossless lexer of Batchfile: every byte of the input ends up in exactly one token.
//!
//! cmd reads a line by what its command word is, so the lexer keeps that state: at the start of a
//! command it reads the command word, then the rest as blank-separated words (`Generic`) or, after
//! `echo`, `title`, `prompt` and `set name=`, as text whose blanks are content (`Text`). Parts of a
//! word: plain text, `%VAR%`, `!VAR!`, `^x`, and quotes (`"` ends at the closing `"` or the end of the
//! line). A `)` ends a word only inside a block. `^` at the end of a line is a `Continuation`
//! (inside the text of `echo`, `set` and comments it is part of the text). Blanks at the end of a
//! line are part of the text in the text of `echo`, `set` and the like, trivia everywhere else.
//! `starts[i]` says that the real token `i` is the first of its line.
//!
//! concept: none (the tokens of Batchfile)

use crate::commands::{self, Word};
use crate::generated::kind as k;
use core_syntax::SyntaxKind;
use core_syntax::parse::{ParseError, Token};

pub struct Lexed {
    pub tokens: Vec<Token>,
    pub errors: Vec<ParseError>,
    pub starts: Vec<bool>,
}

pub fn lex(src: &str) -> Lexed {
    let mut lx = Lexer {
        b: src.as_bytes(),
        pos: 0,
        tokens: Vec::with_capacity(src.len() / 4 + 8),
        errors: Vec::new(),
        starts: Vec::with_capacity(src.len() / 5 + 8),
        fresh: true,
        last: None,
        depth: 0,
        in_set: false,
        want_command: true,
        text: false,
        arg_parens: 0,
        var_command: false,
    };
    while lx.pos < lx.b.len() {
        lx.step();
    }
    let end = lx.b.len();
    lx.starts.push(true);
    lx.tokens.push(Token {
        kind: k::EOF,
        start: end,
        end,
    });
    Lexed {
        tokens: lx.tokens,
        errors: lx.errors,
        starts: lx.starts,
    }
}

struct Lexer<'a> {
    b: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
    errors: Vec<ParseError>,
    starts: Vec<bool>,
    /// No token on this line yet.
    fresh: bool,
    /// The last real token of this line.
    last: Option<SyntaxKind>,
    /// Open blocks.
    depth: u32,
    /// Inside the parentheses of `FOR ... IN ( ... )`.
    in_set: bool,
    /// The next word is a command word.
    want_command: bool,
    /// The rest of the line is text.
    text: bool,
    /// Parentheses opened inside the arguments of the command: their `)` does not close a block.
    arg_parens: u32,
    /// The command word of this command is a variable.
    var_command: bool,
}

fn is_blank(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

fn is_eol(c: u8) -> bool {
    c == b'\n' || c == b'\r'
}

impl Lexer<'_> {
    #[inline]
    fn at(&self, i: usize) -> u8 {
        self.b.get(self.pos + i).copied().unwrap_or(0)
    }

    fn emit(&mut self, kind: SyntaxKind, start: usize) {
        self.tokens.push(Token {
            kind,
            start,
            end: self.pos,
        });
        if kind != k::WHITESPACE && kind != k::NEWLINE && kind != k::CONTINUATION {
            self.starts.push(self.fresh);
            self.fresh = false;
            self.last = Some(kind);
        }
    }

    /// A token of `len` bytes.
    fn take(&mut self, kind: SyntaxKind, len: usize) {
        let start = self.pos;
        self.pos += len;
        self.emit(kind, start);
    }

    fn step(&mut self) {
        let c = self.b[self.pos];
        if is_eol(c) {
            return self.newline();
        }
        if c == b'^' && self.continues() {
            let start = self.pos;
            self.pos += 1;
            self.eat_newline();
            return self.emit(k::CONTINUATION, start);
        }
        if is_blank(c) && (self.want_command || !self.text) {
            return self.blank();
        }
        self.token(c);
    }

    /// `^` here is followed by a line break.
    fn continues(&self) -> bool {
        is_eol(self.at(1))
    }

    fn eat_newline(&mut self) {
        if self.at(0) == b'\r' && self.at(1) == b'\n' {
            self.pos += 2;
        } else {
            self.pos += 1;
        }
    }

    fn newline(&mut self) {
        let start = self.pos;
        self.eat_newline();
        self.emit(k::NEWLINE, start);
        self.fresh = true;
        self.last = None;
        self.want_command = true;
        self.text = false;
        self.arg_parens = 0;
        self.var_command = false;
    }

    fn blank(&mut self) {
        let start = self.pos;
        while is_blank(self.at(0)) {
            self.pos += 1;
        }
        self.emit(k::WHITESPACE, start);
    }

    fn token(&mut self, c: u8) {
        if self.want_command {
            match c {
                b':' if self.last.is_none() => return self.colon_line(),
                b'@' => return self.take(k::AT, 1),
                b'&' | b'|' => return self.take(k::OPERATOR, 1),
                b'(' => {
                    self.depth += 1;
                    return self.take(k::L_PAREN, 1);
                }
                b')' => return self.close_paren(),
                _ => {}
            }
            if self.redirect_at() {
                let handle = self.redirect();
                self.skip_blanks();
                if handle {
                    return;
                }
                if !is_eol(self.at(0)) && self.pos < self.b.len() {
                    self.word(false);
                }
                return;
            }
            return self.command_word();
        }
        match c {
            b'&' | b'|' => {
                let n = if self.at(1) == c { 2 } else { 1 };
                self.take(k::OPERATOR, n);
                self.want_command = true;
                self.text = false;
                self.arg_parens = 0;
                return;
            }
            b'<' | b'>' => return self.redirect_generic(),
            b'(' if self.var_command => {
                // A variable that stands for `if ...` or `for ...` opens a block as they do.
                self.var_command = false;
                self.depth += 1;
                self.want_command = true;
                return self.take(k::L_PAREN, 1);
            }
            b')' if self.closes() => return self.close_paren(),
            _ => {}
        }
        if self.last == Some(k::R_PAREN) && self.keyword_at(b"else") {
            self.take(k::ELSE_WORD, 4);
            self.want_command = true;
            return;
        }
        if self.redirect_at() {
            return self.redirect_generic();
        }
        self.word(self.text);
    }

    fn redirect_generic(&mut self) {
        self.redirect();
        self.text = false;
    }

    /// A `)` closes a block or the list of a `FOR`.
    fn closes(&self) -> bool {
        (self.depth > 0 || self.in_set) && self.arg_parens == 0
    }

    fn close_paren(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
            self.take(k::R_PAREN, 1);
            self.want_command = false;
            self.text = false;
        } else {
            let start = self.pos as u32;
            self.errors
                .push(ParseError::new(start, 1, "stray-paren", "unexpected `)`"));
            self.take(k::UNKNOWN, 1);
        }
    }

    fn skip_blanks(&mut self) {
        if is_blank(self.at(0)) {
            self.blank();
        }
    }

    /// A redirection starts here: `>`, `<`, or a digit and one of them.
    fn redirect_at(&self) -> bool {
        let c = self.at(0);
        c == b'>' || c == b'<' || (c.is_ascii_digit() && matches!(self.at(1), b'>' | b'<'))
    }

    /// A redirection token; true when it names its target (`>&2`).
    fn redirect(&mut self) -> bool {
        let start = self.pos;
        if self.at(0).is_ascii_digit() {
            self.pos += 1;
        }
        let c = self.at(0);
        self.pos += 1;
        if c == b'>' && self.at(0) == b'>' {
            self.pos += 1;
        }
        let handle = self.at(0) == b'&' && self.at(1).is_ascii_digit();
        if handle {
            self.pos += 2;
        }
        self.emit(k::REDIRECT, start);
        handle
    }

    /// The lowercase word `w` starts here and ends at a blank, `(` or the end of the line.
    fn keyword_at(&self, w: &[u8]) -> bool {
        let end = self.pos + w.len();
        self.b
            .get(self.pos..end)
            .is_some_and(|s| s.eq_ignore_ascii_case(w))
            && self
                .b
                .get(end)
                .is_none_or(|&c| is_blank(c) || is_eol(c) || c == b'(')
    }

    /// `:name` or `::text` at the start of a line.
    fn colon_line(&mut self) {
        if self.at(1) == b':' {
            self.take(k::COLON_COLON, 2);
            self.skip_blanks_trivia();
            let kind = if self.at(0).is_ascii_alphanumeric() {
                k::COMMENT_TEXT
            } else {
                k::COMMENT_OTHER
            };
            return self.comment_text(kind);
        }
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.b.len() && !is_blank(self.b[self.pos]) && !is_eol(self.b[self.pos]) {
            self.pos += 1;
        }
        self.emit(k::LABEL, start);
        self.skip_blanks_trivia();
        if !self.at_end() {
            let start = self.pos;
            self.eat_line_text(false);
            self.finish_text(start, k::COMMENT_OTHER);
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.b.len() || is_eol(self.at(0))
    }

    /// Blanks that may be followed by text: always trivia.
    fn skip_blanks_trivia(&mut self) {
        let start = self.pos;
        while is_blank(self.at(0)) {
            self.pos += 1;
        }
        if self.pos > start {
            self.emit(k::WHITESPACE, start);
        }
    }

    /// Blanks in front of more text: trivia; also at the end of the line.
    fn skip_blanks_unless_end(&mut self) {
        let mut i = 0;
        while is_blank(self.at(i)) {
            i += 1;
        }
        if i > 0 && !is_eol(self.at(i)) && self.pos + i < self.b.len() {
            self.skip_blanks_trivia();
        }
    }

    /// The text of a comment up to the end of the line; `^` and a line break go on in the next line.
    fn comment_text(&mut self, kind: SyntaxKind) {
        if self.at_end() {
            return;
        }
        let start = self.pos;
        self.eat_line_text(true);
        self.finish_text(start, kind);
    }

    fn eat_line_text(&mut self, continued: bool) {
        while self.pos < self.b.len() && !is_eol(self.b[self.pos]) {
            if continued && self.b[self.pos] == b'^' && is_eol(self.at(1)) {
                self.pos += 1;
                self.eat_newline();
            } else {
                self.pos += 1;
            }
        }
    }

    /// Emits `start..pos` without its trailing blanks; the blanks stay for the next step.
    fn finish_text(&mut self, start: usize, kind: SyntaxKind) {
        let mut end = self.pos;
        while end > start && is_blank(self.b[end - 1]) {
            end -= 1;
        }
        self.pos = end;
        self.emit(kind, start);
    }

    /// The word at the start of a command.
    fn command_word(&mut self) {
        let start = self.pos;
        let mut i = start;
        while i < self.b.len()
            && !matches!(
                self.b[i],
                b' ' | b'\t'
                    | b'\n'
                    | b'\r'
                    | b'&'
                    | b'|'
                    | b'<'
                    | b'>'
                    | b'^'
                    | b'"'
                    | b'%'
                    | b'!'
            )
            && !(self.b[i] == b')' && self.closes())
        {
            i += 1;
        }
        let composite = i < self.b.len() && matches!(self.b[i], b'^' | b'"' | b'%' | b'!');
        if composite {
            // A word made of parts is a program or a path.
            self.want_command = false;
            self.text = false;
            self.var_command = matches!(self.b[start], b'%' | b'!');
            return self.word(false);
        }
        let run = &self.b[start..i];
        let name_len = run.iter().take_while(|c| c.is_ascii_alphanumeric()).count();
        let (name, rest) = run.split_at(name_len);
        let known = commands::word(name).filter(|_| {
            rest.is_empty()
                || (commands::ends_word(rest[0]) || (rest[0] == b'.' && commands::dot_ends(name)))
        });
        let Some(word) = known else {
            self.want_command = false;
            self.text = false;
            return self.take(k::PROGRAM, run.len().max(1));
        };
        self.want_command = false;
        self.text = false;
        match word {
            Word::If => {
                self.take(k::IF_WORD, name_len);
                self.if_head();
            }
            Word::For => {
                self.take(k::FOR_WORD, name_len);
                self.for_head();
            }
            Word::Rem => {
                self.take(k::REM_WORD, name_len);
                self.rem_text();
            }
            Word::Set => {
                self.take(k::NAME, name_len);
                self.set_head();
            }
            Word::Text => {
                self.take(k::NAME, name_len);
                if is_blank(self.at(0)) && self.pos + 1 < self.b.len() && !is_eol(self.at(1)) {
                    let s = self.pos;
                    self.pos += 1;
                    self.emit(k::WHITESPACE, s);
                }
                self.text = true;
            }
            Word::Internal => self.take(k::NAME, name_len),
        }
    }

    fn rem_text(&mut self) {
        if self.at_end() {
            return;
        }
        let spaced = is_blank(self.at(0));
        self.skip_blanks_trivia();
        self.comment_text(if spaced {
            k::COMMENT_TEXT
        } else {
            k::COMMENT_OTHER
        });
    }

    /// After `set`: the switch `/a` (arithmetic: words) or `/p` and a plain `name=value` (text).
    fn set_head(&mut self) {
        self.skip_blanks_unless_end();
        if self.at(0) == b'/' && matches!(self.at(1), b'a' | b'A' | b'p' | b'P') {
            let arithmetic = matches!(self.at(1), b'a' | b'A');
            if !arithmetic || self.at(2) == 0 || is_blank(self.at(2)) || is_eol(self.at(2)) {
                self.take(k::OPTION, 2);
                if !arithmetic {
                    self.skip_blanks_trivia();
                    self.text = true;
                }
                return;
            }
        }
        self.text = self.at(0) != b'"';
    }

    /// After `if`: switches, `not`, the test and its operands.
    fn if_head(&mut self) {
        loop {
            self.skip_blanks_trivia();
            if self.at_end() {
                return;
            }
            let word = self.bare_word();
            if word.len() == 2 && word[0] == b'/' {
                self.take(k::OPTION, 2);
            } else if word.eq_ignore_ascii_case(b"not") {
                self.take(k::COND_WORD, 3);
            } else {
                break;
            }
        }
        let word = self.bare_word();
        if commands::test_word(word) {
            let n = word.len();
            self.take(k::COND_WORD, n);
            self.skip_blanks_trivia();
            self.operand(false);
        } else {
            self.operand(true);
            self.skip_blanks_trivia();
            if self.at(0) == b'=' && self.at(1) == b'=' {
                self.take(k::COMPARE, 2);
            } else {
                let word = self.bare_word();
                if commands::comparison(word) {
                    let n = word.len();
                    self.take(k::COND_WORD, n);
                } else {
                    self.want_command = true;
                    return;
                }
            }
            self.skip_blanks_trivia();
            self.operand(false);
        }
        self.want_command = true;
    }

    fn operand(&mut self, stop_eq: bool) {
        if !self.at_end() {
            self.word_with(false, stop_eq);
        }
    }

    /// The letters of a keyword at the position, up to a blank, `(`, `)` or the end of the line.
    fn bare_word(&self) -> &[u8] {
        let mut i = self.pos;
        while i < self.b.len()
            && !matches!(
                self.b[i],
                b' ' | b'\t'
                    | b'\n'
                    | b'\r'
                    | b'('
                    | b')'
                    | b'&'
                    | b'|'
                    | b'<'
                    | b'>'
                    | b'^'
                    | b'"'
                    | b'%'
                    | b'!'
            )
        {
            i += 1;
        }
        &self.b[self.pos..i]
    }

    /// After `for`: switches, the variable, `in`, the list in parentheses and `do`.
    fn for_head(&mut self) {
        loop {
            self.skip_blanks_trivia();
            if self.at_end() {
                return;
            }
            let word = self.bare_word();
            if word.eq_ignore_ascii_case(b"in") {
                self.take(k::IN_WORD, 2);
                break;
            }
            if word.len() == 2 && word[0] == b'/' {
                self.take(k::OPTION, 2);
            } else {
                self.word(false);
            }
        }
        self.skip_blanks_trivia();
        if self.at(0) != b'(' {
            return;
        }
        self.take(k::L_PAREN, 1);
        self.in_set = true;
        let mut nest = 1u32;
        while self.pos < self.b.len() && nest > 0 {
            let c = self.b[self.pos];
            if is_eol(c) {
                self.newline();
                self.want_command = false;
            } else if c == b'^' && self.continues() {
                self.step();
            } else if is_blank(c) {
                self.blank();
            } else if c == b')' {
                nest -= 1;
                if nest == 0 {
                    break;
                }
                self.take(k::TEXT, 1);
            } else if c == b'(' {
                nest += 1;
                self.take(k::TEXT, 1);
            } else if self.redirect_at() {
                self.redirect();
            } else {
                self.word_with(false, false);
            }
        }
        self.in_set = false;
        if nest == 0 {
            self.take(k::R_PAREN, 1);
            self.skip_blanks_trivia();
            if self.keyword_at(b"do") {
                self.take(k::DO_WORD, 2);
                self.want_command = true;
            }
        }
    }

    fn word(&mut self, text: bool) {
        self.word_with(text, false);
    }

    /// One word as parts. In `text`, blanks are part of the text; else a blank ends the word.
    fn word_with(&mut self, text: bool, stop_eq: bool) {
        let mut run = self.pos;
        let first = self.pos;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            let stop = match c {
                b' ' | b'\t' => !text,
                b'\n' | b'\r' | b'&' | b'|' | b'<' | b'>' => true,
                b')' => self.closes(),
                b'=' => stop_eq && self.at(1) == b'=',
                b'^' => !text && self.continues(),
                d if d.is_ascii_digit() => {
                    text && matches!(self.at(1), b'>' | b'<')
                        && (self.pos == first || is_blank(self.b[self.pos - 1]))
                }
                _ => false,
            };
            if stop {
                break;
            }
            match c {
                b'"' => {
                    self.flush(run);
                    self.quoted();
                    run = self.pos;
                }
                b'%' | b'!' => match self.variable() {
                    Some(end) => {
                        self.flush(run);
                        let kind = if c == b'%' { k::VAR } else { k::DELAYED_VAR };
                        let n = end - self.pos;
                        self.take(kind, n);
                        run = self.pos;
                    }
                    None => self.pos += 1,
                },
                b'(' => {
                    self.arg_parens += 1;
                    self.pos += 1;
                }
                b')' => {
                    self.arg_parens = self.arg_parens.saturating_sub(1);
                    self.pos += 1;
                }
                b'^' if self.continues() => {
                    // Text goes on in the next line.
                    self.pos += 1;
                    self.eat_newline();
                }
                b'^' if self.pos + 1 < self.b.len() => {
                    self.flush(run);
                    let n = self.char_len(1) + 1;
                    self.take(k::ESCAPE, n);
                    run = self.pos;
                }
                _ => self.pos += 1,
            }
        }
        self.flush(run);
        if self.pos == first {
            // Nothing took the byte (a lone `^` at the end of the file): keep it as text.
            self.take(k::TEXT, 1);
        }
    }

    fn char_len(&self, at: usize) -> usize {
        let c = self.at(at);
        match c {
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            _ => 1,
        }
        .min(self.b.len().saturating_sub(self.pos + at))
        .max(1)
    }

    fn flush(&mut self, from: usize) {
        if self.pos > from {
            self.emit(k::TEXT, from);
        }
    }

    /// A quoted part: `"`, text with variables, and the closing `"` unless the line ends first.
    fn quoted(&mut self) {
        self.take(k::QUOTE, 1);
        let mut run = self.pos;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            if c == b'"' || is_eol(c) {
                break;
            }
            if matches!(c, b'%' | b'!')
                && let Some(end) = self.variable()
            {
                self.flush(run);
                let kind = if c == b'%' { k::VAR } else { k::DELAYED_VAR };
                let n = end - self.pos;
                self.take(kind, n);
                run = self.pos;
                continue;
            }
            self.pos += 1;
        }
        self.flush(run);
        if self.at(0) == b'"' {
            self.take(k::QUOTE, 1);
        }
    }

    /// Where the variable that starts at the position ends: `%name%`, `%1`, `%*`, `%~dp0`, `%%i`, `!name!`.
    fn variable(&self) -> Option<usize> {
        let b = self.b;
        let i = self.pos;
        let c = b[i];
        let at = |j: usize| b.get(j).copied().unwrap_or(0);
        if c == b'!' {
            let mut j = i + 1;
            while j < b.len()
                && !matches!(
                    b[j],
                    b'!' | b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'&' | b'|' | b'<' | b'>' | b'^'
                )
            {
                j += 1;
            }
            return (j > i + 1 && at(j) == b'!').then_some(j + 1);
        }
        let next = at(i + 1);
        if next == b'%' {
            // `%%i` and `%%~nxi` are the variable of a FOR; `%%` is a percent sign.
            let mut j = i + 2;
            if at(j) == b'~' {
                while at(j).is_ascii_alphabetic() || at(j) == b'~' || at(j) == b'$' || at(j) == b':'
                {
                    j += 1;
                }
                return (at(j).is_ascii_alphanumeric()).then_some(j + 1);
            }
            return at(j).is_ascii_alphanumeric().then_some(j + 1);
        }
        if next.is_ascii_digit() || next == b'*' {
            return Some(i + 2);
        }
        if next == b'~' {
            let mut j = i + 2;
            while at(j).is_ascii_alphabetic() || at(j) == b'$' || at(j) == b':' {
                j += 1;
            }
            if at(j).is_ascii_digit() || at(j) == b'*' {
                return Some(j + 1);
            }
        }
        let mut j = i + 1;
        while j < b.len()
            && !matches!(
                b[j],
                b'%' | b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'&' | b'|' | b'<' | b'>' | b'^'
            )
        {
            j += 1;
        }
        (j > i + 1 && at(j) == b'%').then_some(j + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_is_in_a_token() {
        let src = "@echo off\r\nif /i \"%a%\"==\"b\" (\n  echo  x ^& y >f 2>&1\n) else echo z\n:l\n::c\nrem  q\nfor %%i in (a b) do echo %%i !v!\nset a=b \n^\n";
        let lexed = lex(src);
        let joined: String = lexed.tokens.iter().map(|t| &src[t.start..t.end]).collect();
        assert_eq!(joined, src);
    }
}
