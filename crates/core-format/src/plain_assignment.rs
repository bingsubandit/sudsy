//! Which `name op value` statements the spacing options apply to: all, or only the plain ones
//! (the name at the start of its line, letters, digits and underscores, no substitution reference
//! on the line and no colon that makes it read like a rule); another stays as written.
//!
//! concept: assignment-scope
//! what: A statement `name op value` is laid out by the spacing options only when its form is plain.
//! tools: mbake assignment_spacing
//! reads: option:spaced_assignments, path:plain_assignment::AssignmentForm

use crate::source_indent::SourceIndent;
use crate::{Doc, FormatCtx, comments};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

/// The token kinds of an assignment statement.
pub struct AssignmentForm {
    /// The kind of an assignment statement node.
    pub statement: SyntaxKind,
    /// The kinds of a name token.
    pub names: &'static [SyntaxKind],
    /// The kinds of an assignment operator.
    pub operators: &'static [SyntaxKind],
}

impl AssignmentForm {
    /// Do the spacing options apply to the assignment `node` (`spaced_assignments`)? The column of
    /// the name counts only when the layout keeps the indentation (`indent_kept`): else the
    /// indentation of the output says nothing about the input's, and the next run must decide alike.
    pub fn spaced(&self, cx: &FormatCtx<'_>, node: &SyntaxNode, indent_kept: bool) -> bool {
        cx.options.is(core_config::opt::SPACED_ASSIGNMENTS, "all")
            || self.is_plain(cx, node, indent_kept)
    }

    /// Is `node` an assignment that the spacing options leave as written?
    pub fn as_written(&self, cx: &FormatCtx<'_>, node: &SyntaxNode, indent_kept: bool) -> bool {
        node.kind() == self.statement && !self.spaced(cx, node, indent_kept)
    }

    /// `build()` (the line of `node`) at the blanks of the source when the layout keeps them
    /// (`indent_kept`) for the lines of `kinds` and for an assignment that stays as written.
    pub fn line_at_source<E>(
        &self,
        cx: &FormatCtx<'_>,
        node: &SyntaxNode,
        kinds: &SourceIndent,
        indent_kept: bool,
        build: impl FnOnce() -> Result<Doc, E>,
    ) -> Result<Doc, E> {
        if indent_kept
            && (kinds.applies(node) || self.as_written(cx, node, indent_kept))
            && let Some(first) = cx.first_token(node)
        {
            return cx.at_source_indent(&first, build);
        }
        build()
    }

    /// The scope of the line that `node` starts.
    pub fn scope(&self, cx: &FormatCtx<'_>, node: &SyntaxNode, indent_kept: bool) -> Scope {
        Scope {
            exact: self.as_written(cx, node, indent_kept),
        }
    }

    /// Is the first token of `node` a name of letters, digits and underscores (not starting with
    /// a digit) at the start of its line, and the next an operator, and the first line of the
    /// statement without a substitution reference or a rule-like colon?
    fn is_plain(&self, cx: &FormatCtx<'_>, node: &SyntaxNode, indent_kept: bool) -> bool {
        let Some(name) = cx.first_token(node) else {
            return false;
        };
        let text = name.text();
        let identifier = text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !text.starts_with(|c: char| c.is_ascii_digit());
        identifier
            && self.names.contains(&name.kind())
            && (!indent_kept || comments::column(&name) == 0)
            && next_token(cx, &name).is_some_and(|t| self.operators.contains(&t.kind()))
            && {
                let line = first_line(node);
                !has_substitution_reference(&line) && !reads_like_rule(&line)
            }
    }
}

/// Whether the part of a line being written is an assignment left as written: its blanks stay and
/// no spacing option applies.
pub struct Scope {
    exact: bool,
}

impl Scope {
    /// Is the part written an assignment left as written?
    pub fn exact(&self) -> bool {
        self.exact
    }

    /// Enters the assignment `node` nested in the line (`export A=b`); `leave` with the result.
    pub fn enter(
        &mut self,
        form: &AssignmentForm,
        cx: &FormatCtx<'_>,
        node: &SyntaxNode,
        indent_kept: bool,
    ) -> bool {
        std::mem::replace(&mut self.exact, form.as_written(cx, node, indent_kept))
    }

    pub fn leave(&mut self, outer: bool) {
        self.exact = outer;
    }

    /// Does the blank token `blank` stay as written? It does inside an exact assignment, after
    /// something on the line, and not at a line continuation (`continuation` says which tokens
    /// are one) or next to one.
    pub fn keeps_blank(
        &self,
        blank: &SyntaxToken,
        after_something: bool,
        continuation: impl Fn(&SyntaxToken) -> bool,
    ) -> bool {
        self.exact
            && after_something
            && !blank.prev_token().is_some_and(|p| continuation(&p))
            && !blank.next_token().is_some_and(|n| continuation(&n))
    }
}

/// The first line of the statement (the tokens may start with the line breaks before it).
fn first_line(node: &SyntaxNode) -> String {
    let mut line = String::new();
    for t in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let text = if line.is_empty() {
            t.text().trim_start()
        } else {
            t.text()
        };
        if let Some(end) = text.find('\n') {
            line.push_str(&text[..end]);
            break;
        }
        line.push_str(text);
    }
    line
}

/// Does the line hold `$(name:pattern=replacement)`?
fn has_substitution_reference(line: &str) -> bool {
    line.match_indices("$(").any(|(i, _)| {
        let inner = line[i + 2..].split(')').next().unwrap_or_default();
        inner
            .split_once(':')
            .is_some_and(|(_, after)| after.contains('='))
            && line[i + 2..].contains(')')
    })
}

/// Is the line `name=value` without a blank after the operator and with a colon in the first word
/// of the value, which is no URL, time or path (`A=b:c` is a rule for a reader)?
fn reads_like_rule(line: &str) -> bool {
    let line = line.trim();
    let Some((name, value)) = line.split_once('=') else {
        return false;
    };
    let word = value.split_whitespace().next().unwrap_or_default();
    let identifier = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    identifier
        && !value.starts_with(char::is_whitespace)
        && word.contains(':')
        && !colon_is_value(value)
}

/// A value whose colons are part of it: a URL (`scheme://`), a date and time, or a path.
fn colon_is_value(value: &str) -> bool {
    let v = value.trim().trim_matches(['"', '\'']);
    let bytes = v.as_bytes();
    let url = v.split_once("://").is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    });
    let drive = bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    let path =
        (v.starts_with('/') || v.starts_with("./") || v.starts_with("../")) && v.contains(':');
    let time = v.len() >= 16
        && v.is_char_boundary(16)
        && v.as_bytes()[4] == b'-'
        && v.as_bytes()[7] == b'-'
        && matches!(v.as_bytes()[10], b'T' | b't' | b' ')
        && v.as_bytes()[13] == b':';
    url || drive || path || time
}

fn next_token(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> Option<SyntaxToken> {
    let mut next = tok.next_token();
    while let Some(t) = next {
        if !cx.grammar.is_trivia(t.kind()) {
            return Some(t);
        }
        next = t.next_token();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{colon_is_value, has_substitution_reference, reads_like_rule};

    #[test]
    fn a_substitution_reference_has_a_colon_and_an_equals_sign() {
        assert!(has_substitution_reference("H=$(I:%=%.o)"));
        assert!(!has_substitution_reference("H=$(I) a=b"));
        assert!(!has_substitution_reference("H=$(I:%"));
    }

    #[test]
    fn a_colon_in_the_first_word_reads_like_a_rule_unless_it_is_part_of_a_value() {
        assert!(reads_like_rule("J=a:b"));
        assert!(!reads_like_rule("J = a:b"));
        assert!(!reads_like_rule("K=http://a"));
        assert!(!reads_like_rule("K=/usr/bin:/bin"));
        assert!(!reads_like_rule("A=b"));
        assert!(colon_is_value("C:\\dir"));
        assert!(colon_is_value("2024-01-02T10:20:30"));
        assert!(!colon_is_value("a:b"));
    }
}
