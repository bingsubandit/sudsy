//! Expressions: precedence climbing for the binary operators; the left-recursive forms (binary
//! operators, member chains, array access, method references) wrap what was built with a
//! checkpoint, so they are loops.
//!
//! concept: new precedence-expression-parser
//! what: Expression parsing by operator precedence and associativity.

use super::P;
use super::look::Paren;
use crate::generated::kind as k;
use crate::lexer::T;
use core_format::deep;
use core_syntax::Checkpoint;
use core_syntax::parse::{Operator, Pratt, climb};

impl P<'_> {
    /// An expression: assignment is the loosest operator.
    pub(super) fn expr(&mut self) {
        let cp = self.cp();
        self.ternary();
        if matches!(self.cur(), T::Eq | T::AssignOp) {
            self.wrap(cp, k::ASSIGNMENT_EXPRESSION, |p| {
                p.bump();
                deep(|| p.expr());
            });
        }
    }

    pub(super) fn ternary(&mut self) {
        let cp = self.cp();
        climb(self, 1);
        if self.at(T::Question) {
            self.wrap(cp, k::TERNARY_EXPRESSION, |p| {
                p.bump_as(k::OPERATOR);
                let lambda = std::mem::replace(&mut p.no_lambda, false);
                deep(|| p.expr());
                p.no_lambda = lambda;
                p.expect(T::Colon, "`:`");
                deep(|| p.ternary());
            });
        }
    }

    /// The binary operator at the current token: its precedence (the grammar's `precedence`
    /// lines), its number of tokens (`>>` and `>>>` are joined from adjacent `>` tokens) and the
    /// kind of its token; `instanceof` has token count 0.
    fn binary_op(&self) -> Option<Operator> {
        let t = self.cur();
        let grammar = core_syntax::grammars::JAVA.get();
        if t == T::Instanceof {
            let prec = grammar.precedence("instanceof")?;
            return Some(Operator {
                prec,
                tokens: 0,
                kind: k::OPERATOR,
            });
        }
        let mut n = 1;
        while t == T::Gt && n < 3 && self.nth(n) == T::Gt && self.adjacent(self.pos + n - 1) {
            n += 1;
        }
        let (first, last) = (self.toks.get(self.pos)?, self.toks.get(self.pos + n - 1)?);
        let text = &self.src[first.start as usize..last.end as usize];
        let prec = grammar.precedence(text)?;
        let kind = match t {
            T::Lt => k::LT,
            T::Gt if n == 1 => k::GT,
            _ => k::OPERATOR,
        };
        Some(Operator {
            prec,
            tokens: n,
            kind,
        })
    }

    /// After `expr instanceof`: a type or a pattern.
    fn instanceof_rest(&mut self) {
        self.bump_kw();
        self.modifier_tokens(false);
        self.ty();
        if self.at(T::LParen) {
            self.node(k::INSTANCEOF_PATTERN, |p| p.record_pattern_body());
        } else if self.at_name() && !matches!(self.nth(1), T::Dot | T::LParen) {
            self.node(k::INSTANCEOF_PATTERN, |p| {
                p.node(k::PATTERN_VARIABLE, |p| p.bump())
            });
        }
    }

    fn unary(&mut self) {
        deep(|| match self.cur() {
            T::Plus | T::Minus | T::Not | T::Tilde => self.node(k::UNARY_EXPRESSION, |p| {
                p.bump();
                p.unary();
            }),
            T::PlusPlus | T::MinusMinus => self.node(k::UPDATE_EXPRESSION, |p| {
                p.bump();
                p.unary();
            }),
            T::LParen if self.paren_kind() == Paren::Cast => self.node(k::CAST_EXPRESSION, |p| {
                p.bump();
                p.ty();
                while p.at(T::Amp) {
                    p.bump_as(k::OPERATOR);
                    p.ty();
                }
                p.expect(T::RParen, "`)`");
                p.unary();
            }),
            _ => self.postfix(),
        })
    }

    /// A primary expression and what follows it: member access, calls, indexing, `++`, `::`.
    pub(super) fn postfix(&mut self) {
        let cp = self.cp();
        self.primary();
        core_syntax::access::access_chain(&mut Chain { p: self, cp });
    }

    /// An expression inside brackets: lambdas are allowed again.
    pub(super) fn nested_expr(&mut self) {
        let lambda = std::mem::replace(&mut self.no_lambda, false);
        self.expr();
        self.no_lambda = lambda;
    }

    /// `.name`, `.name(...)`, `.<T>name(...)`, `.new`, `.this`, `.super`, `.class` after the
    /// expression that starts at `cp`. False when what follows the dot is none of them.
    fn dot_suffix(&mut self, cp: core_syntax::Checkpoint) -> bool {
        match self.nth(1) {
            t if t.is_name() => {
                if self.nth(2) == T::LParen {
                    self.wrap(cp, k::METHOD_INVOCATION, |p| {
                        p.bump();
                        p.method_name();
                        p.arguments();
                    });
                } else {
                    self.wrap(cp, k::FIELD_ACCESS, |p| {
                        p.bump();
                        p.bump();
                    });
                }
            }
            T::Lt => self.wrap(cp, k::METHOD_INVOCATION, |p| {
                p.bump();
                p.type_arguments();
                p.method_name();
                p.arguments();
            }),
            T::New => self.wrap(cp, k::OBJECT_CREATION_EXPRESSION, |p| {
                p.bump();
                p.creator_rest();
            }),
            T::This => self.wrap(cp, k::FIELD_ACCESS, |p| {
                p.bump();
                p.node(k::THIS_EXPRESSION, |p| p.bump_kw());
            }),
            // `outer.super(...)` is a constructor invocation, which the statement reads.
            T::Super if self.nth(2) == T::LParen => return false,
            T::Super => self.wrap(cp, k::QUALIFIED_SUPER_EXPRESSION, |p| {
                p.bump();
                p.node(k::SUPER_EXPRESSION, |p| p.bump_kw());
            }),
            T::Class => self.wrap(cp, k::CLASS_LITERAL, |p| {
                p.bump();
                p.bump_kw();
            }),
            _ => {
                self.error("expected a name after `.`");
                return false;
            }
        }
        true
    }

    fn method_name(&mut self) {
        if self.at_name() {
            self.node(k::METHOD_NAME, |p| p.bump());
        } else {
            self.error("expected a method name");
        }
    }

    /// `(a, b)`.
    pub(super) fn arguments(&mut self) {
        self.node(k::ARGUMENT_LIST, |p| {
            p.expect(T::LParen, "`(`");
            let lambda = std::mem::replace(&mut p.no_lambda, false);
            p.list(T::RParen, |p| p.expr());
            p.no_lambda = lambda;
        });
    }

    /// `::` after the expression or type: `::new` or `::name`, with type arguments.
    fn method_reference_rest(&mut self) {
        self.bump();
        if self.at(T::Lt) {
            self.type_arguments();
        }
        if self.at(T::New) {
            self.bump_kw();
        } else if self.at_name() {
            self.node(k::METHOD_REFERENCE_NAME, |p| p.bump());
        } else {
            self.error("expected a method name");
        }
    }

    fn primary(&mut self) {
        match self.cur() {
            T::NumberLit | T::StringLit | T::CharLit | T::True | T::False | T::Null => self.bump(),
            T::This => self.node(k::THIS_EXPRESSION, |p| p.bump_kw()),
            T::Super => self.node(k::SUPER_EXPRESSION, |p| p.bump_kw()),
            T::New => self.creator(),
            T::Switch => self.switch(true),
            T::LParen => match self.paren_kind() {
                Paren::Lambda => self.lambda(),
                _ => self.node(k::PARENTHESIZED_EXPRESSION, |p| {
                    p.bump();
                    p.nested_expr();
                    p.expect(T::RParen, "`)`");
                }),
            },
            T::Void => {
                let cp = self.cp();
                self.bump_kw();
                if self.at(T::Dot) && self.nth(1) == T::Class {
                    self.wrap(cp, k::CLASS_LITERAL, |p| {
                        p.bump();
                        p.bump_kw();
                    });
                } else {
                    self.error("expected `.class`");
                }
            }
            t if t.is_primitive() => self.type_expr(),
            t if t.is_name() => {
                let next = self.nth(1);
                if next == T::Arrow && !self.no_lambda {
                    self.lambda();
                } else if next == T::LParen {
                    self.node(k::METHOD_INVOCATION, |p| {
                        p.node(k::METHOD_NAME, |p| p.bump());
                        p.arguments();
                    });
                } else if matches!(next, T::Dot | T::Lt | T::LBracket) && self.type_expr_ahead() {
                    self.type_expr();
                } else {
                    self.bump();
                }
            }
            T::RParen | T::RBracket | T::RBrace | T::LBrace | T::Semi | T::Comma | T::Colon => {
                self.error("expected an expression");
            }
            _ if self.at_eof() => self.error("expected an expression"),
            _ => self.bump_error("expected an expression"),
        }
    }

    /// Whether a type starts here that is followed by `::` or `.class`: a method reference or a
    /// class literal on a type (`List<String>::of`, `a.B[].class`).
    fn type_expr_ahead(&self) -> bool {
        match self.skip_type(self.pos, true) {
            Some(e) => {
                self.at_idx(e) == T::ColonColon
                    || (self.at_idx(e) == T::Dot && self.at_idx(e + 1) == T::Class)
            }
            None => false,
        }
    }

    /// A type that is the receiver of `::` or `.class`.
    fn type_expr(&mut self) {
        let cp = self.cp();
        self.ty();
        match self.cur() {
            T::ColonColon => self.wrap(cp, k::METHOD_REFERENCE, |p| p.method_reference_rest()),
            T::Dot if self.nth(1) == T::Class => self.wrap(cp, k::CLASS_LITERAL, |p| {
                p.bump();
                p.bump_kw();
            }),
            _ => self.error("expected `.class` or `::`"),
        }
    }

    // Lambdas

    fn lambda(&mut self) {
        self.node(k::LAMBDA_EXPRESSION, |p| {
            if p.at(T::LParen) {
                p.lambda_parameters();
            } else {
                p.bump();
            }
            p.expect(T::Arrow, "`->`");
            let lambda = std::mem::replace(&mut p.no_lambda, false);
            if p.at(T::LBrace) {
                p.block();
            } else {
                deep(|| p.expr());
            }
            p.no_lambda = lambda;
        });
    }

    fn lambda_parameters(&mut self) {
        let close = self.close.get(self.pos).map_or(usize::MAX, |&c| c as usize);
        // Names only: `(a, b)`.
        let mut i = self.pos + 1;
        let mut names = i < close;
        while names && i < close {
            names = self.at_idx(i).is_name()
                && (i + 1 == close || (self.at_idx(i + 1) == T::Comma && i + 2 < close));
            i += 2;
        }
        let kind = if self.pos + 1 == close {
            k::LAMBDA_FORMAL_PARAMETERS
        } else if names {
            k::INFERRED_PARAMETERS
        } else {
            k::LAMBDA_FORMAL_PARAMETERS
        };
        self.node(kind, |p| {
            p.bump();
            if kind == k::INFERRED_PARAMETERS {
                p.list(T::RParen, |p| p.bump());
            } else {
                p.list(T::RParen, |p| {
                    if p.at(T::Var) && p.nth(1).is_name() {
                        p.bump_kw();
                        p.bump();
                    } else {
                        p.formal_parameter();
                    }
                });
            }
        });
    }

    // Creation

    fn creator(&mut self) {
        let mut j = self.pos + 1;
        if self.at_idx(j) == T::Lt {
            j = self.skip_type_args(j, 0).unwrap_or(j);
        }
        let array = self
            .skip_type(j, false)
            .is_some_and(|e| self.at_idx(self.skip_annotations(e)) == T::LBracket);
        if array {
            self.node(k::ARRAY_CREATION_EXPRESSION, |p| {
                p.bump_kw();
                p.node(k::ARRAY_CREATION_TYPE, |p| {
                    p.annotations();
                    p.simple_type();
                });
                p.array_dimensions();
                if p.at(T::LBrace) {
                    p.array_initializer();
                }
            });
        } else {
            self.node(k::OBJECT_CREATION_EXPRESSION, |p| p.creator_rest());
        }
    }

    /// `new`, its type, arguments and class body (the qualifier, if any, is built).
    fn creator_rest(&mut self) {
        self.bump_kw();
        if self.at(T::Lt) {
            self.type_arguments();
        }
        self.node(k::CREATED_CLASS_TYPE, |p| {
            p.annotations();
            p.simple_type();
        });
        self.arguments();
        if self.at(T::LBrace) {
            self.class_body(k::CLASS_BODY);
        }
    }

    /// `[expr]` and `[]` after the element type of an array creation.
    fn array_dimensions(&mut self) {
        loop {
            let j = self.skip_annotations(self.pos);
            if self.at_idx(j) != T::LBracket {
                return;
            }
            if self.at_idx(j + 1) == T::RBracket {
                self.node(k::DIMENSION, |p| {
                    p.annotations();
                    p.bump();
                    p.bump();
                });
            } else {
                self.node(k::DIM_EXPR, |p| {
                    p.annotations();
                    p.bump();
                    p.nested_expr();
                    p.expect(T::RBracket, "`]`");
                });
            }
        }
    }

    /// `{a, b, {c}}`.
    pub(super) fn array_initializer(&mut self) {
        self.node(k::ARRAY_INITIALIZER, |p| {
            p.bump();
            let lambda = std::mem::replace(&mut p.no_lambda, false);
            p.list(T::RBrace, |p| {
                if p.at(T::LBrace) {
                    deep(|| p.array_initializer());
                } else {
                    p.expr();
                }
            });
            p.no_lambda = lambda;
        });
    }
}

impl Pratt for P<'_> {
    type Mark = Checkpoint;

    fn mark(&mut self) -> Checkpoint {
        self.cp()
    }

    fn operand(&mut self) {
        self.unary();
    }

    fn operator(&self) -> Option<Operator> {
        self.binary_op()
    }

    fn infix(&mut self, at: Checkpoint, op: Operator) {
        if op.tokens == 0 {
            self.wrap(at, k::INSTANCEOF_EXPRESSION, |p| p.instanceof_rest());
        } else {
            self.wrap(at, k::BINARY_EXPRESSION, |p| {
                p.emit(op.kind, op.tokens);
                climb(p, op.rhs_min());
            });
        }
    }
}

/// The access chain of the expression that starts at `cp`.
struct Chain<'a, 'b> {
    p: &'a mut P<'b>,
    cp: core_syntax::Checkpoint,
}

impl core_syntax::access::Access for Chain<'_, '_> {
    fn at_subscript(&self) -> bool {
        self.p.cur() == T::LBracket
    }
    fn subscript(&mut self) {
        self.p.wrap(self.cp, k::ARRAY_ACCESS, |p| {
            p.bump();
            p.nested_expr();
            p.expect(T::RBracket, "`]`");
        });
    }
    fn at_member(&self) -> bool {
        self.p.cur() == T::Dot
    }
    fn member(&mut self) -> bool {
        self.p.dot_suffix(self.cp)
    }
    fn suffix(&mut self) -> bool {
        match self.p.cur() {
            T::PlusPlus | T::MinusMinus => self.p.wrap(self.cp, k::UPDATE_EXPRESSION, |p| p.bump()),
            T::ColonColon => self
                .p
                .wrap(self.cp, k::METHOD_REFERENCE, |p| p.method_reference_rest()),
            _ => return false,
        }
        true
    }
}
