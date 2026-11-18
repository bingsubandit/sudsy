//! oxc's AST and tokens as our lossless tree.
//!
//! The lexer's tokens and the AST nodes' spans (one `Visit` walk over every `AstKind`) go to
//! `provider_api::adapter::replay`, which builds the tree; this file knows oxc's kinds. Nodes with no token (`JSXEmptyExpression`) and kinds without a
//! grammar node (`Program`, which is the root node of the tree, and `Hashbang`, a comment) are
//! left out.

use crate::generated::kind as k;
use core_syntax::{Grammar, SyntaxKind, grammars};
use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_ast::AstKind;
use oxc_ast::ast::{Expression, JSXChild, JSXExpression, TemplateElement};
use oxc_ast_visit::Visit;
use oxc_parser::config::TokensParserConfig;
use oxc_parser::{Kind, ParseOptions, Parser};
use oxc_span::{GetSpan, SourceType};
use provider_api::adapter::{KindNames, Node, Tok, replay_grammar};
use provider_api::{ParseError, ParseResult, SourceFile};

/// The source type of a file name; JavaScript has JSX whatever its extension (as in Babel), and
/// text of no known extension (a Markdown fence) is JavaScript.
pub fn source_type(path: &str) -> SourceType {
    let st = SourceType::from_path(path).unwrap_or_else(|_| SourceType::jsx());
    if st.is_javascript() {
        st.with_jsx(true)
    } else {
        st
    }
}

/// A file of the provider; an embedded expression (`EmbedMode::Expression`) is the only element
/// of an array.
pub fn parse_file(file: &SourceFile) -> ParseResult {
    let text = match file.language_version.as_deref() {
        Some("expression" | "ng-expression" | "ng-nested") => &format!("[\n{}\n]", file.text),
        _ => &file.text,
    };
    parse(text, source_type(&file.id))
}

/// Runs `attempt` (it returns its result and whether it had syntax errors) as a module first,
/// as Babel does, and as the unambiguous type of the extension (a script when the file has no
/// module syntax) only when that fails and the second try is clean.
pub fn module_first<R>(st: SourceType, attempt: impl Fn(SourceType) -> (R, bool)) -> R {
    if !st.is_unambiguous() {
        return attempt(st).0;
    }
    let first = attempt(st.with_module(true));
    if !first.1 {
        return first.0;
    }
    let second = attempt(st);
    if second.1 { first.0 } else { second.0 }
}

pub fn parse(src: &str, source_type: SourceType) -> ParseResult {
    module_first(source_type, |st| {
        let r = parse_as(src, st);
        let failed = !r.errors.is_empty();
        (r, failed)
    })
}

fn parse_as(src: &str, source_type: SourceType) -> ParseResult {
    let allocator = Allocator::default();
    let options = ParseOptions {
        allow_return_outside_function: true,
        ..ParseOptions::default()
    };
    let parsed = Parser::new(&allocator, src, source_type)
        .with_options(options)
        .with_config(TokensParserConfig)
        .parse();
    let mut errors: Vec<ParseError> = parsed
        .diagnostics
        .errors()
        .map(|d| {
            let (start, len) = d.labels.first().map_or((0, 0), |l| (l.offset(), l.len()));
            ParseError::syntax(start, len, d.message.to_string())
        })
        .collect();
    // A fatal error leaves nothing to take tokens and nodes from: the file is one unknown token.
    let (tokens, nodes) = if parsed.fatal_error {
        let whole = Tok {
            kind: k::UNKNOWN,
            start: 0,
            end: src.len() as u32,
        };
        (vec![whole], Vec::new())
    } else {
        let mut collector = Collector::new(grammars::JS.get());
        collector.comment_starts = parsed
            .program
            .comments
            .iter()
            .map(|c| c.span.start)
            .collect();
        collector.visit_program(&parsed.program);
        // oxc visits a function's `this` parameter before its parameter list (which starts before
        // it) and a class's decorators inside the class (they may precede `export`): `replay`
        // wants parents first, in source order.
        let order = |n: &Node| (n.start, std::cmp::Reverse(n.end));
        if !collector.nodes.is_sorted_by_key(order) {
            collector.nodes.sort_by_key(order);
        }
        let tokens = (parsed.tokens.iter())
            .filter(|t| t.end() > t.start())
            .map(|t| Tok {
                kind: token_kind(t.kind()),
                start: t.start(),
                end: t.end(),
            })
            .collect();
        (tokens, collector.nodes)
    };
    let mut result = replay_grammar(src, grammars::JS.get(), &tokens, &nodes, union_bars);
    errors.append(&mut result.errors);
    errors.sort_by_key(|e| e.start);
    result.errors = errors;
    result
}

/// The bars of a union type are separators, a bitwise `|` an operator.
fn union_bars(t: Tok, parent: Option<SyntaxKind>) -> Tok {
    match t.kind {
        k::PIPE if parent != Some(k::TS_UNION_TYPE) => Tok {
            kind: k::OPERATOR,
            ..t
        },
        _ => t,
    }
}

/// Our token kind for oxc's.
fn token_kind(kind: Kind) -> SyntaxKind {
    match kind {
        Kind::LParen => k::L_PAREN,
        Kind::RParen => k::R_PAREN,
        Kind::LCurly => k::L_BRACE,
        Kind::RCurly => k::R_BRACE,
        Kind::LBrack => k::L_BRACKET,
        Kind::RBrack => k::R_BRACKET,
        Kind::LAngle => k::L_ANGLE,
        Kind::RAngle => k::R_ANGLE,
        Kind::Comma => k::COMMA,
        Kind::Semicolon => k::SEMICOLON,
        Kind::Colon => k::COLON,
        Kind::Dot => k::DOT,
        Kind::Dot3 => k::DOT3,
        Kind::Arrow => k::ARROW,
        Kind::Question => k::QUESTION,
        Kind::QuestionDot => k::QUESTION_DOT,
        Kind::At => k::AT,
        Kind::Pipe => k::PIPE,
        Kind::Ident => k::IDENTIFIER,
        Kind::Str => k::STRING,
        Kind::RegExp => k::REG_EXP,
        Kind::PrivateIdentifier => k::PRIVATE_NAME,
        Kind::JSXText => k::JSX_TEXT_RUN,
        Kind::NoSubstitutionTemplate
        | Kind::TemplateHead
        | Kind::TemplateMiddle
        | Kind::TemplateTail => k::TEMPLATE_CHUNK,
        Kind::Decimal
        | Kind::Float
        | Kind::Binary
        | Kind::Octal
        | Kind::Hex
        | Kind::PositiveExponential
        | Kind::NegativeExponential => k::NUMBER,
        Kind::DecimalBigInt | Kind::BinaryBigInt | Kind::OctalBigInt | Kind::HexBigInt => {
            k::BIG_INT
        }
        kind if kind.is_reserved_keyword() => k::KEYWORD,
        kind if kind.is_any_keyword() => k::IDENTIFIER,
        _ => k::OPERATOR,
    }
}

/// The nodes of the AST in pre-order.
struct Collector {
    /// Our node kind by `AstType`; `None` is a kind with no node.
    kinds: KindNames,
    nodes: Vec<Node>,
    /// Starts of the `{" "}` children of elements and of their strings (see `enter_node`).
    jsx_space: std::collections::HashSet<u32>,
    /// Where the comments start: a `{" "}` with a comment inside is an expression like any other.
    comment_starts: Vec<u32>,
    /// Starts of the chains in parentheses that an optional link continues, `(a?.b)?.c`: they are
    /// no node of their own, as the output (`a?.b?.c`) has none.
    merged_chains: std::collections::HashSet<u32>,
}

impl Collector {
    fn new(grammar: &'static Grammar) -> Self {
        Collector {
            // `Program` is the root and `Hashbang` a comment.
            kinds: KindNames::new(
                grammar,
                usize::from(oxc_ast::ast_kind::AST_TYPE_MAX) + 1,
                |name| matches!(name, "Program" | "Hashbang"),
            ),
            nodes: Vec::new(),
            jsx_space: std::collections::HashSet::new(),
            comment_starts: Vec::new(),
            merged_chains: std::collections::HashSet::new(),
        }
    }

    fn note_merged_chain(&mut self, link: &Expression<'_>) {
        if let Expression::ParenthesizedExpression(p) = link
            && let Expression::ChainExpression(c) = &p.expression
        {
            self.merged_chains.insert(c.span.start);
        }
    }

    fn note_jsx_space(&mut self, children: &[JSXChild<'_>]) {
        for child in children {
            if let JSXChild::ExpressionContainer(c) = child
                && let JSXExpression::StringLiteral(s) = &c.expression
                && s.value == " "
                && !self
                    .comment_starts
                    .iter()
                    .any(|&p| p > c.span.start && p < c.span.end)
            {
                self.jsx_space.extend([c.span.start, s.span.start]);
            }
        }
    }
}

impl<'a> Visit<'a> for Collector {
    /// The quasis of a template lie inside its chunk tokens and never become nodes; visited
    /// before the expressions they would hold back the nodes of those (which are in source order).
    fn visit_template_elements(&mut self, _: &ArenaVec<'a, TemplateElement<'a>>) {}

    fn enter_node(&mut self, kind: AstKind<'a>) {
        let ty = kind.ty();
        let ours = self
            .kinds
            .get(ty as usize, false, || format!("{ty:?}"))
            .filter(|_| ty != oxc_ast::AstType::Program);
        let span = kind.span();
        // A `{" "}` child of an element is JSX whitespace, which the formatter moves between
        // text and `{" "}` like text: it is a `JSXText` node (transparent to the safety check)
        // with no node for its string.
        match kind {
            AstKind::JSXElement(e) => self.note_jsx_space(&e.children),
            AstKind::JSXFragment(f) => self.note_jsx_space(&f.children),
            AstKind::StaticMemberExpression(m) if m.optional => self.note_merged_chain(&m.object),
            AstKind::ComputedMemberExpression(m) if m.optional => self.note_merged_chain(&m.object),
            AstKind::CallExpression(c) if c.optional => self.note_merged_chain(&c.callee),
            _ => {}
        }
        let ours = match kind {
            AstKind::JSXExpressionContainer(_) if self.jsx_space.contains(&span.start) => {
                Some(k::JSX_TEXT)
            }
            AstKind::StringLiteral(_) if self.jsx_space.contains(&span.start) => None,
            AstKind::ChainExpression(_) if self.merged_chains.contains(&span.start) => None,
            _ => ours,
        };
        if let Some(kind) = ours.filter(|_| span.end > span.start) {
            self.nodes.push(Node {
                start: span.start,
                end: span.end,
                kind,
            });
        }
    }
}
