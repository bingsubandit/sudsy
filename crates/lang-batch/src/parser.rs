//! A lossless parser of Batchfile on the tokens of the lexer: a file is a list of lines, a line a
//! `Sequence` of statements joined by operators, a block holds lines. `IF` and `ELSE` take the rest
//! of their line, or a block, as a `Sequence`; `FOR` the same after `DO`.
//!
//! Recovery: a `)` with no block is one `ERROR` token; a block with no `)` ends at the end of the
//! file (a label at column 0 inside a block is valid, so it does not end one). Nesting deeper than `LEVELS` is one `ERROR` node (a balanced group).
//!
//! concept: none (the productions of Batchfile on the parser kit of core_syntax::parse)

use crate::generated::kind as k;
use crate::lexer::{Lexed, lex};
use core_syntax::SyntaxKind;
use core_syntax::parse::{self, MAX_DEPTH, Parsed};
use provider_api::{ParseResult, SourceFile};

/// Statements nested in statements: each level is up to four levels of the tree.
const LEVELS: usize = MAX_DEPTH / 5;

pub fn parse_file(file: &SourceFile) -> ParseResult {
    let Parsed { tree, errors } = parse(&file.text);
    ParseResult { tree, errors }
}

pub fn parse(src: &str) -> Parsed {
    let Lexed {
        tokens,
        errors,
        starts,
    } = lex(src);
    let mut cx = Cx {
        p: parse::Parser::new(src, (tokens, errors), k::TRIVIA_MASK, k::EOF),
        starts,
    };
    cx.p.start(k::FILE);
    while !cx.p.at_eof() {
        cx.line();
    }
    cx.p.bump(); // Eof
    cx.p.finish();
    cx.p.done()
}

struct Cx<'a> {
    p: parse::Parser<'a>,
    starts: Vec<bool>,
}

fn is_part(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::TEXT | k::VAR | k::DELAYED_VAR | k::ESCAPE | k::QUOTE | k::OPTION
    )
}

impl Cx<'_> {
    fn cur(&self) -> SyntaxKind {
        self.p.cur()
    }

    /// The current token is on the line of the one before it.
    fn here(&self) -> bool {
        !self.p.at_eof() && !self.starts[self.p.pos]
    }

    /// The current token touches the one before it.
    fn touching(&self) -> bool {
        self.p.pos > 0 && self.p.tok(self.p.pos).start == self.p.tok(self.p.pos - 1).end
    }

    /// One line of the file or of a block; a token that starts no statement is an error.
    fn line(&mut self) {
        let before = self.p.pos;
        self.sequence();
        if self.p.pos == before {
            self.stray();
        }
    }

    fn stray(&mut self) {
        self.p.error("unexpected-token", "unexpected token");
        self.p.start(SyntaxKind::ERROR);
        self.p.bump();
        self.p.finish();
    }

    fn sequence(&mut self) {
        self.p.start(k::SEQUENCE);
        loop {
            self.statement();
            if self.cur() == k::OPERATOR && self.here() {
                self.p.bump();
                if self.here() {
                    continue;
                }
            }
            break;
        }
        self.p.finish();
    }

    fn statement(&mut self) {
        while self.cur() == k::AT {
            self.p.bump();
            if !self.here() {
                return;
            }
        }
        match self.cur() {
            k::NAME | k::PROGRAM | k::REDIRECT => self.simple(),
            kind if is_part(kind) => self.simple(),
            k::REM_WORD | k::COLON_COLON => self.comment(),
            k::LABEL => self.label(),
            k::L_PAREN => self.block(),
            k::IF_WORD => self.if_command(),
            k::FOR_WORD => self.for_command(),
            k::R_PAREN | k::ELSE_WORD | k::EOF => {}
            k::UNKNOWN | k::OPERATOR => {
                self.p.start(SyntaxKind::ERROR);
                self.p.bump();
                self.p.finish();
            }
            _ => self.stray(),
        }
    }

    fn simple(&mut self) {
        self.p.start(k::SIMPLE);
        if self.cur() == k::REDIRECT {
            self.redirection();
        }
        while self.here() && self.cur() == k::REDIRECT {
            self.redirection();
        }
        if self.here() && self.cur() == k::L_PAREN {
            // Redirections in front of a block belong to it.
            self.block();
        } else if matches!(self.cur(), k::NAME | k::PROGRAM) {
            self.p.bump();
        } else if is_part(self.cur()) {
            self.argument();
        }
        while self.here() {
            match self.cur() {
                k::REDIRECT => self.redirection(),
                kind if is_part(kind) => self.argument(),
                _ => break,
            }
        }
        self.p.finish();
    }

    fn argument(&mut self) {
        self.p.start(k::ARGUMENT);
        loop {
            if self.cur() == k::QUOTE {
                self.quoted();
            } else {
                self.p.bump();
            }
            if !(self.here() && self.touching() && is_part(self.cur())) {
                break;
            }
        }
        self.p.finish();
    }

    fn quoted(&mut self) {
        self.p.start(k::QUOTED);
        self.p.bump();
        while self.here()
            && self.touching()
            && matches!(self.cur(), k::TEXT | k::VAR | k::DELAYED_VAR)
        {
            self.p.bump();
        }
        if self.here() && self.touching() && self.cur() == k::QUOTE {
            self.p.bump();
        }
        self.p.finish();
    }

    fn redirection(&mut self) {
        self.p.start(k::REDIRECTION);
        self.p.bump();
        if self.here() && is_part(self.cur()) {
            self.argument();
        }
        self.p.finish();
    }

    fn comment(&mut self) {
        self.p.start(k::COMMENT_LINE);
        self.p.bump();
        if self.here() && matches!(self.cur(), k::COMMENT_TEXT | k::COMMENT_OTHER) {
            self.p.bump();
        }
        self.p.finish();
    }

    fn label(&mut self) {
        self.p.start(k::LABEL_LINE);
        self.p.bump();
        if self.here() && self.cur() == k::COMMENT_OTHER {
            self.p.bump();
        }
        self.p.finish();
    }

    fn block(&mut self) {
        if !self.p.enter(LEVELS) {
            self.p.error(
                "nesting-too-deep",
                format!("blocks are nested deeper than {LEVELS} levels"),
            );
            let limit = self.p.eof_index();
            self.p.error_group(k::L_PAREN, k::R_PAREN, limit);
            return;
        }
        self.p.start(k::BLOCK);
        let open = self.p.pos;
        self.p.bump();
        while !self.p.at_eof() && self.cur() != k::R_PAREN {
            self.line();
        }
        if self.cur() == k::R_PAREN {
            self.p.bump();
        } else {
            self.p
                .error_at(open, "unclosed-block", "expected `)` for this `(`");
        }
        self.p.finish();
        self.p.leave();
    }

    /// A statement nested too deep: the rest of its line, as one `ERROR` node.
    fn too_deep(&mut self) {
        self.p.error(
            "nesting-too-deep",
            format!("statements are nested deeper than {LEVELS} levels"),
        );
        self.p.start(SyntaxKind::ERROR);
        let limit = self.p.eof_index();
        loop {
            if self.cur() == k::L_PAREN {
                self.p.error_group(k::L_PAREN, k::R_PAREN, limit);
            } else {
                self.p.bump();
            }
            if !self.here() {
                break;
            }
        }
        self.p.finish();
    }

    fn operand(&mut self) {
        if self.here() && is_part(self.cur()) {
            self.argument();
        }
    }

    fn word_is(&self, word: &str) -> bool {
        self.p.text_at(self.p.pos).eq_ignore_ascii_case(word)
    }

    fn if_command(&mut self) {
        if !self.p.enter(LEVELS) {
            return self.too_deep();
        }
        self.p.start(k::IF_COMMAND);
        self.p.bump();
        self.p.start(k::CONDITION);
        while self.here()
            && (self.cur() == k::OPTION || (self.cur() == k::COND_WORD && self.word_is("not")))
        {
            self.p.bump();
        }
        if self.here() && self.cur() == k::COND_WORD {
            self.p.bump();
            self.operand();
        } else {
            self.operand();
            if self.here() && matches!(self.cur(), k::COMPARE | k::COND_WORD) {
                self.p.bump();
                self.operand();
            }
        }
        self.p.finish();
        if self.here() {
            self.sequence();
        }
        if self.here() && self.cur() == k::ELSE_WORD {
            self.p.start(k::ELSE_CLAUSE);
            self.p.bump();
            if self.here() {
                self.sequence();
            }
            self.p.finish();
        }
        self.p.finish();
        self.p.leave();
    }

    fn for_command(&mut self) {
        if !self.p.enter(LEVELS) {
            return self.too_deep();
        }
        self.p.start(k::FOR_COMMAND);
        self.p.bump();
        while self.here() && self.cur() != k::IN_WORD {
            match self.cur() {
                kind if is_part(kind) => self.argument(),
                k::REDIRECT => self.redirection(),
                _ => self.p.bump(),
            }
        }
        if self.here() && self.cur() == k::IN_WORD {
            self.p.bump();
            if self.here() && self.cur() == k::L_PAREN {
                self.for_list();
            }
            if self.here() && self.cur() == k::DO_WORD {
                self.p.bump();
                if self.here() {
                    self.sequence();
                }
            }
        }
        self.p.finish();
        self.p.leave();
    }

    fn for_list(&mut self) {
        self.p.start(k::FOR_LIST);
        let open = self.p.pos;
        self.p.bump();
        while !self.p.at_eof() && self.cur() != k::R_PAREN {
            match self.cur() {
                kind if is_part(kind) => self.argument(),
                k::REDIRECT => self.redirection(),
                _ => self.p.bump(),
            }
        }
        if self.cur() == k::R_PAREN {
            self.p.bump();
        } else {
            self.p
                .error_at(open, "unclosed-list", "expected `)` for this `(`");
        }
        self.p.finish();
    }
}
