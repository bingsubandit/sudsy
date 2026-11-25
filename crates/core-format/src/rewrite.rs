//! Code rewrites as data: a formatter that also rewrites code (a short array syntax, a default
//! visibility) lists token rows, and the source is edited before it is laid out.
//!
//! A line is `PATTERN : ACTION`, the pattern as in [`crate::spacing`] and the action one of
//! `replace "x"` (the token becomes `x`), `replace through next "x"` (the token and the next one,
//! with the blanks between them, become `x`), `insert before "x"` and `insert after "x"`. The
//! first row that matches a token is the one applied. Lines starting with `#` are comments.
//!
//! concept: token-rewrite-rows
//! what: Token rewrites (replace, insert) as rows applied to the source before layout.
//! tools: PHP-CS-Fixer short array syntax and visibility_required
//! reads: rules:rewrite

use crate::spacing::{EndSet, Pattern};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

/// The option listing kind pairs: a node of the first kind directly inside one of the second.
pub const PARENTHESIZE: core_config::OptId = core_config::opt::REWRITE_PARENTHESIZE_PAIRS;

/// The kind pairs of `rewrite_parenthesize_pairs`, resolved once per file.
pub struct Parenthesize(Vec<(core_syntax::SyntaxKind, core_syntax::SyntaxKind)>);

impl Parenthesize {
    pub fn new(grammar: &Grammar, options: &core_config::ResolvedOptions) -> Self {
        let names: Vec<&str> = options.strings(PARENTHESIZE).collect();
        Self(
            names
                .chunks(2)
                .filter_map(|p| {
                    Some((
                        grammar.kind_by_name(p[0])?,
                        grammar.kind_by_name(p.get(1)?)?,
                    ))
                })
                .collect(),
        )
    }

    /// Where the parentheses go (`(start, end)` byte offsets) when `node` is the first kind of a
    /// pair and its parent the second.
    pub fn at(&self, grammar: &Grammar, node: &SyntaxNode) -> Option<(usize, usize)> {
        let own = node.kind();
        if self.0.iter().all(|p| p.0 != own) {
            return None;
        }
        let parent = node.parent()?.kind();
        self.0.contains(&(own, parent)).then_some(())?;
        let mut tokens = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !grammar.is_trivia(t.kind()));
        let first = tokens.next()?;
        let last = tokens.last().unwrap_or_else(|| first.clone());
        Some((
            usize::from(first.text_range().start()),
            usize::from(last.text_range().end()),
        ))
    }
}

enum Action {
    Replace(String),
    ReplaceThroughNext(String),
    InsertBefore(String),
    InsertAfter(String),
}

pub struct RewriteRules {
    rows: Vec<(Pattern, Action)>,
    /// By node kind: bit 0, a row looks at the tokens of a node of it (`parent=`); bit 1, a row
    /// looks at its last token (`end_of=`).
    anchors: Vec<u8>,
    /// Does every row have an anchor? Then only the tokens the anchors name are looked at,
    /// found while the caller walks the nodes anyway; else every token is.
    anchored: bool,
    /// The token kinds the `parent=` rows can match; `None`: any.
    parent_tokens: Option<Vec<core_syntax::SyntaxKind>>,
}

impl RewriteRules {
    pub fn parse(grammar: &Grammar, text: &str) -> RewriteRules {
        let rows = crate::rules::section(text, "rewrite")
            .map(|line| {
                let (pattern, action) = line
                    .split_once(" : ")
                    .unwrap_or_else(|| panic!("rewrite: `{line}`"));
                let (words, arg) = action
                    .split_once('"')
                    .unwrap_or_else(|| panic!("rewrite: `{line}`"));
                let arg = arg.trim_end_matches('"').to_string();
                let action = match words.trim() {
                    "replace" => Action::Replace(arg),
                    "replace through next" => Action::ReplaceThroughNext(arg),
                    "insert before" => Action::InsertBefore(arg),
                    "insert after" => Action::InsertAfter(arg),
                    other => panic!("rewrite: unknown action `{other}`"),
                };
                (Pattern::parse(grammar, pattern), action)
            })
            .collect();
        let rows: Vec<(Pattern, Action)> = rows;
        let mut anchors: Vec<u8> = Vec::new();
        let mut set = |kinds: Vec<core_syntax::SyntaxKind>, bit: u8| {
            for k in kinds {
                let at = k.index() as usize;
                if anchors.len() <= at {
                    anchors.resize(at + 1, 0);
                }
                anchors[at] |= bit;
            }
        };
        let mut anchored = true;
        let mut parent_tokens: Option<Vec<core_syntax::SyntaxKind>> = Some(Vec::new());
        for (p, _) in &rows {
            let (parents, ends) = (p.parent_kinds(), p.end_kinds());
            if !parents.is_empty() {
                match (p.kinds(), &mut parent_tokens) {
                    (Some(ks), Some(all)) => all.extend_from_slice(ks),
                    _ => parent_tokens = None,
                }
            }
            anchored &= !parents.is_empty() || !ends.is_empty();
            set(parents, 1);
            set(ends, 2);
        }
        RewriteRules {
            rows,
            anchors,
            anchored,
            parent_tokens,
        }
    }

    /// A scan of one tree: the caller shows it every node ([`RewriteScan::visit`], in any order)
    /// and takes the edits.
    pub fn scan(&self) -> RewriteScan<'_> {
        RewriteScan {
            rules: self,
            ends: EndSet::default(),
            tokens: Vec::new(),
        }
    }

    /// The source with the rows applied, `None` when no token matches.
    pub fn apply(&self, grammar: &Grammar, tree: &SyntaxNode, text: &str) -> Option<String> {
        let mut scan = self.scan();
        for n in tree.descendants() {
            scan.visit(&n);
        }
        scan.apply(grammar, tree, text)
    }

    /// Adds what the scan looks at to `wanted` ([`core_syntax::tree::find`]): the nodes that
    /// end something, and the tokens of the nodes a `parent=` row names ([`RewriteScan::add_token`]).
    pub fn wanted(&self, wanted: &mut core_syntax::tree::Wanted) {
        let set = |v: &mut Vec<bool>, at: usize| {
            if v.len() <= at {
                v.resize(at + 1, false);
            }
            v[at] = true;
        };
        for (i, f) in self.anchors.iter().enumerate() {
            if f & 1 != 0 {
                set(&mut wanted.parents, i);
            }
            if f & 2 != 0 {
                set(&mut wanted.nodes, i);
            }
        }
        match &self.parent_tokens {
            Some(kinds) => kinds
                .iter()
                .for_each(|k| set(&mut wanted.tokens, k.index() as usize)),
            None => wanted.any_token = true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The edits the rows make, `(from, to, text)` in document order: a language that rewrites
    /// more of its own in the same round merges them with its edits.
    pub fn edits(&self, grammar: &Grammar, tree: &SyntaxNode) -> Vec<(usize, usize, &str)> {
        let mut scan = self.scan();
        for n in tree.descendants() {
            scan.visit(&n);
        }
        scan.edits(grammar, tree)
    }
}

/// The rows' view of a tree while its nodes go by.
pub struct RewriteScan<'r> {
    rules: &'r RewriteRules,
    ends: EndSet,
    /// The tokens the anchors name (when every row has one).
    tokens: Vec<SyntaxToken>,
}

impl<'r> RewriteScan<'r> {
    /// A token `find` found for the rows ([`RewriteRules::wanted`]), instead of [`RewriteScan::visit`]
    /// of its parent.
    pub fn add_token(&mut self, token: SyntaxToken) {
        self.tokens.push(token);
    }

    pub fn visit(&mut self, node: &SyntaxNode) {
        let flags = self
            .rules
            .anchors
            .get(node.kind().index() as usize)
            .copied()
            .unwrap_or(0);
        if flags & 1 != 0 && self.rules.anchored {
            self.tokens
                .extend(node.children_with_tokens().filter_map(|e| e.into_token()));
        }
        if flags & 2 != 0 {
            self.visit_end(node);
        }
    }

    fn visit_end(&mut self, node: &SyntaxNode) {
        self.ends.add(node);
        if self.rules.anchored
            && let Some(l) = crate::spacing::last_code(node)
        {
            self.tokens.push(l);
        }
    }

    /// The source with the rows applied, `None` when no token matches.
    pub fn apply(self, grammar: &Grammar, tree: &SyntaxNode, text: &str) -> Option<String> {
        let edits = self.edits(grammar, tree);
        if edits.is_empty() {
            return None;
        }
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for (from, to, with) in edits {
            out.push_str(&text[at..from]);
            out.push_str(with);
            at = to;
        }
        out.push_str(&text[at..]);
        Some(out)
    }

    pub fn edits(mut self, grammar: &Grammar, tree: &SyntaxNode) -> Vec<(usize, usize, &'r str)> {
        let rules = self.rules;
        let mut edits: Vec<(usize, usize, &str)> = Vec::new();
        let mut one = |t: SyntaxToken| -> bool {
            if grammar.is_trivia(t.kind()) {
                return true;
            }
            let kind = t.kind();
            let Some((_, action)) = rules.rows.iter().find(|(p, _)| {
                p.kinds().is_none_or(|ks| ks.contains(&kind)) && p.matches_ends(&t, &self.ends)
            }) else {
                return true;
            };
            let (start, end) = (
                usize::from(t.text_range().start()),
                usize::from(t.text_range().end()),
            );
            edits.push(match action {
                Action::Replace(x) => (start, end, &x[..]),
                Action::InsertBefore(x) => (start, start, &x[..]),
                Action::InsertAfter(x) => (end, end, &x[..]),
                Action::ReplaceThroughNext(x) => match next_end(grammar, &t) {
                    Some(e) => (start, e, &x[..]),
                    None => return false,
                },
            });
            true
        };
        if rules.anchored {
            let mut tokens = std::mem::take(&mut self.tokens);
            tokens.sort_by_key(|t| t.text_range().start());
            tokens.dedup();
            for t in tokens {
                if !one(t) {
                    return Vec::new();
                }
            }
        } else {
            for t in tree
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
            {
                if !one(t) {
                    return Vec::new();
                }
            }
        }
        edits
    }
}

/// The end of the next token that is not whitespace or a comment.
fn next_end(grammar: &Grammar, t: &SyntaxToken) -> Option<usize> {
    let mut cur = t.next_token();
    while let Some(n) = cur {
        if !grammar.is_trivia(n.kind()) {
            return Some(usize::from(n.text_range().end()));
        }
        cur = n.next_token();
    }
    None
}

/// The `;` between and after statements, but not in a node of the kinds of `semicolon_keep_parents`:
/// one at the end of a line, in front of a `}`, an `else` or a comment is dropped, one between
/// two statements of a line becomes a line break. Edits `(from, to, text)`.
pub fn redundant_semicolons(
    grammar: &Grammar,
    tree: &SyntaxNode,
    text: &str,
    keep: &[&str],
    edits: &mut Vec<(usize, usize, String)>,
) {
    // (a scan of the text finds the candidates; a walk of the tree for all tokens costs more)
    for t in text
        .match_indices(';')
        .filter_map(|(at, _)| {
            let at = u32::try_from(at).ok()?;
            tree.token_at_offset(at.into()).right_biased()
        })
        .filter(|t| t.text() == ";" && grammar.name(t.kind()).starts_with("Semi"))
    {
        if t.parent()
            .is_none_or(|p| keep.contains(&grammar.name(p.kind())))
        {
            continue;
        }
        let (s, e) = (
            usize::from(t.text_range().start()),
            usize::from(t.text_range().end()),
        );
        let rest = text[e..].trim_start_matches([' ', '\t']);
        let ends_line = rest.is_empty()
            || rest.starts_with(['\n', '\r', '}', ')'])
            || rest.starts_with("//")
            || rest.starts_with("else");
        if ends_line {
            edits.push((s, e, String::new()));
        } else {
            edits.push((s, text.len() - rest.len(), "\n".into()));
        }
    }
}

/// The kinds and keywords of [`keyword_forms`]: the instruction node, its keyword token, the token
/// that holds a legacy `key value` pair and the tokens of a JSON array.
pub struct KeywordForms<'a> {
    pub instruction: core_syntax::SyntaxKind,
    pub keyword: core_syntax::SyntaxKind,
    pub pair: core_syntax::SyntaxKind,
    /// Instructions whose `pair` token `key value` may be written `key=value`.
    pub pair_keywords: &'a [&'a str],
    /// The tokens of a JSON array: `[`, a string, `,`, `]`.
    pub array: [core_syntax::SyntaxKind; 4],
    /// Instructions whose JSON array of plain words may be written as arguments.
    pub array_keywords: &'a [&'a str],
}

/// The value a word of an instruction has once its quotes and escapes are read (`$` stays: it is
/// read the same wherever the word stands), and whether an unquoted blank splits it into several
/// words. `None` for an unterminated quote or escape.
fn read_word(raw: &str, escape: char) -> Option<(String, bool)> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    let (mut quote, mut split) = (None::<char>, false);
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (Some('"'), _) if c == escape => match chars.next()? {
                n @ ('"' | '$') => out.push(n),
                n if n == escape => out.push(n),
                n => {
                    out.push(c);
                    out.push(n);
                }
            },
            (None, _) if c == escape => out.push(chars.next()?),
            (None, ' ' | '\t') => {
                split = true;
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    quote.is_none().then_some((out, split))
}

/// `key=value` for the legacy `key value` (the value is the rest of the line after the blanks),
/// when the two are read the same: a value without a quote goes in double quotes if it has blanks,
/// a quoted value stays. The new text is read again with [`read_word`], independently of how it
/// was made: it must be one word with the value of the old text.
fn legacy_pair(text: &str, escape: char) -> Option<String> {
    let name_end = text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
    let (name, rest) = text.split_at(name_end);
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let value = rest.strip_prefix([' ', '\t'])?.trim();
    if value.is_empty() || value.contains(['\n', '\r']) {
        return None;
    }
    let new = if value.contains([' ', '\t']) && !value.contains(['"', '\'']) {
        format!("{name}=\"{value}\"")
    } else {
        format!("{name}={value}")
    };
    let (old, _) = read_word(value, escape)?;
    let (read, split) = read_word(new.split_once('=')?.1, escape)?;
    (read == old && !split).then_some(new)
}

/// The edit that turns the JSON array that `open` starts into plain arguments, when every element
/// is a plain word and the array is on one line. The words are read again from the new text.
fn array_words(
    open: &SyntaxToken,
    [_, string, comma, close]: [core_syntax::SyntaxKind; 4],
    text: &str,
) -> Option<(usize, usize, String)> {
    let plain = |s: &str| {
        !s.starts_with('-')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_./:@%+,=~*?^-".contains(c))
    };
    let mut words = Vec::new();
    let (mut expect_string, mut end) = (true, None);
    let mut next = open.next_token();
    while let Some(t) = next {
        match t.kind() {
            k if k == string && expect_string => {
                let word = t.text().strip_prefix('"')?.strip_suffix('"')?;
                words.push(word.to_string());
                expect_string = false;
            }
            k if k == comma && !expect_string => expect_string = true,
            k if k == close && !expect_string => {
                end = Some(t.text_range().end());
                break;
            }
            // (blanks only: a comment, a line break or a continuation stops the rewrite)
            _ if t.text().trim().is_empty() && !t.text().contains('\n') => {}
            _ => return None,
        }
        next = t.next_token();
    }
    let (start, end) = (usize::from(open.text_range().start()), usize::from(end?));
    let new = words.join(" ");
    let read: Vec<&str> = new.split(' ').collect();
    (words.len() > 1
        && words.iter().all(|w| plain(w))
        && read.len() == words.len()
        && read.iter().zip(&words).all(|(a, b)| a == b)
        && !text[start..end].contains('\n'))
    .then_some((start, end, new))
}

/// The edits of the equivalent forms of an instruction's arguments (see [`KeywordForms`]): the
/// legacy `key value` pair as `key=value` where `pairs`, the JSON array as words where `arrays`.
/// `escape` is the file's escape character.
pub fn keyword_forms(
    tree: &SyntaxNode,
    text: &str,
    forms: &KeywordForms<'_>,
    (pairs, arrays, escape): (bool, bool, char),
    edits: &mut Vec<(usize, usize, String)>,
) {
    let named =
        |set: &[&str], t: &SyntaxToken| set.iter().any(|w| t.text().eq_ignore_ascii_case(w));
    for node in tree.descendants().filter(|n| n.kind() == forms.instruction) {
        let Some(keyword) = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == forms.keyword)
        else {
            continue;
        };
        let (pair, array) = (
            pairs && named(forms.pair_keywords, &keyword),
            arrays && named(forms.array_keywords, &keyword),
        );
        if !(pair || array) {
            continue;
        }
        for t in node.children_with_tokens().filter_map(|e| e.into_token()) {
            if pair && t.kind() == forms.pair {
                if let Some(new) = legacy_pair(t.text(), escape) {
                    let r = t.text_range();
                    edits.push((usize::from(r.start()), usize::from(r.end()), new));
                }
            } else if array && t.kind() == forms.array[0] {
                edits.extend(array_words(&t, forms.array, text));
            }
        }
    }
}

/// A line comment whose marker run (`#`, `##`) is followed by a letter or digit gets a blank after
/// it (`#x` as `# x`); `skip` names the comment tokens that stay as written. Edits
/// `(from, to, text)`.
pub fn comment_space(
    grammar: &Grammar,
    tree: &SyntaxNode,
    skip: impl Fn(&SyntaxToken) -> bool,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let Some(marker) = grammar.comments.line.as_deref() else {
        return;
    };
    for t in tree
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| grammar.has_tag(t.kind(), "comment"))
    {
        let mut rest = t.text();
        let mut at = usize::from(t.text_range().start());
        if !rest.starts_with(marker) {
            continue;
        }
        while let Some(r) = rest.strip_prefix(marker) {
            rest = r;
            at += marker.len();
        }
        if rest.starts_with(char::is_alphanumeric) && !skip(&t) {
            edits.push((at, at, " ".into()));
        }
    }
}

/// `fun f(): T { return x }` becomes `fun f(): T = x` (also `throw`): a function whose body is one
/// `#return` or `#throw` without a label and without comments. A `throw` body of a function with
/// no declared type gets the first of `redundant_return_types` (`fun f(): Unit = throw x`: the
/// body has type `Nothing`). Edits `(from, to, text)`.
pub fn expression_body(
    g: &Grammar,
    tree: &SyntaxNode,
    text: &str,
    options: &core_config::ResolvedOptions,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let default_type = options
        .strings(core_config::opt::REDUNDANT_RETURN_TYPES)
        .next();
    for fun in tree
        .descendants()
        .filter(|n| g.has_tag(n.kind(), "function"))
    {
        let Some(block) = fun.children().find(|n| g.has_tag(n.kind(), "block")) else {
            continue;
        };
        let mut items = block.children();
        let (Some(stmt), None) = (items.next(), items.next()) else {
            continue;
        };
        let throws = g.has_tag(stmt.kind(), "throw");
        let mut parts = stmt.children();
        let (Some(value), None) = (parts.next(), parts.next()) else {
            continue; // (a label, or nothing returned)
        };
        let has_comment = block
            .descendants_with_tokens()
            .any(|e| g.has_tag(e.kind(), "comment"));
        if !(throws || g.has_tag(stmt.kind(), "return")) || has_comment {
            continue;
        }
        let Some(end) = block
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !g.is_trivia(t.kind()))
            .last()
            .map(|t| t.text_range().end())
        else {
            continue;
        };
        let (b, v) = (block.text_range(), value.text_range());
        let untyped = text[..usize::from(b.start())].trim_end().ends_with(')');
        let prefix = match default_type {
            Some(t) if throws && untyped => format!(": {t} = throw "),
            _ if throws => "= throw ".to_string(),
            _ => "= ".to_string(),
        };
        edits.push((b.start().into(), v.start().into(), prefix));
        edits.push((v.end().into(), end.into(), String::new()));
    }
}

/// The `text` with the edits `(from, to, text)` applied: an insert first, then the larger of the
/// edits that start at the same place; an edit inside the text an earlier one replaced is dropped.
pub fn apply_edits(text: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by_key(|(s, e, _)| (*s, e > s, std::cmp::Reverse(*e)));
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (s, e, with) in edits {
        if s < at {
            continue;
        }
        out.push_str(&text[at..s]);
        out.push_str(&with);
        at = e;
    }
    out.push_str(&text[at..]);
    out
}

/// `${name}` in a string template: an entry of one of the kinds of `redundant_template_braces`
/// that holds one plain name (an `#identifier` token, no backticks) loses its braces unless the
/// text after it goes on with a name (a letter, a digit, `_`) or another `$`. Edits
/// `(from, to, text)`.
pub fn redundant_template_braces(
    grammar: &Grammar,
    tree: &SyntaxNode,
    text: &str,
    options: &core_config::ResolvedOptions,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let kinds: Vec<&str> = options
        .strings(core_config::opt::REDUNDANT_TEMPLATE_BRACES)
        .collect();
    if kinds.is_empty() {
        return;
    }
    for (at, _) in text.match_indices("${") {
        let Some(open) = u32::try_from(at)
            .ok()
            .and_then(|at| tree.token_at_offset(at.into()).right_biased())
        else {
            continue;
        };
        let Some(entry) = open.parent().filter(|p| {
            kinds.contains(&grammar.name(p.kind())) && p.first_token().as_ref() == Some(&open)
        }) else {
            continue;
        };
        let end = usize::from(entry.text_range().end());
        let code: Vec<_> = entry
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !grammar.is_trivia(t.kind()))
            .collect();
        let plain = code.len() == 3
            && grammar.has_tag(code[1].kind(), "identifier")
            && code[1]
                .text()
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_');
        let next = text[end..].chars().next();
        if plain && next.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$')) {
            edits.push((at, at + 2, "$".into()));
            edits.push((end - 1, end, String::new()));
        }
    }
}

/// The last entry of a body (`trailing_comma_last_entry`: pairs of a body kind and an entry kind)
/// with one entry per line, or with several entries over several lines, gets a trailing comma; a
/// `;` after it goes on a line of its own, or is replaced by the comma when nothing follows it.
pub fn trailing_comma_last_entry(
    grammar: &Grammar,
    tree: &SyntaxNode,
    text: &str,
    options: &core_config::ResolvedOptions,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let pairs: Vec<&str> = options
        .strings(core_config::opt::TRAILING_COMMA_LAST_ENTRY)
        .collect();
    let code = |n: &SyntaxNode| {
        n.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !grammar.is_trivia(t.kind()))
            .collect::<Vec<_>>()
    };
    for pair in pairs.chunks(2) {
        let [body_kind, entry_kind] = pair else {
            continue;
        };
        for body in tree
            .descendants()
            .filter(|n| grammar.name(n.kind()) == *body_kind)
        {
            let entries: Vec<_> = body
                .children()
                .filter(|n| grammar.name(n.kind()) == *entry_kind)
                .collect();
            let Some(entry) = entries.last() else {
                continue;
            };
            let toks = code(entry);
            let own = entry.children_with_tokens().filter_map(|e| e.into_token());
            if own.clone().any(|t| t.text() == ",") {
                continue;
            }
            let (Some(first), Some(last)) = (toks.first(), toks.last()) else {
                continue;
            };
            let start = usize::from(first.text_range().start());
            let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
            let indent = &text[line_start..start];
            let own_line = line_start > 0 && indent.trim().is_empty();
            if !own_line && (entries.len() < 2 || !body.text().to_string().trim().contains('\n')) {
                continue;
            }
            if let Some(semi) = own.clone().find(|t| t.text() == ";") {
                if own_line {
                    let (s, e) = (
                        usize::from(semi.text_range().start()),
                        usize::from(semi.text_range().end()),
                    );
                    // (nothing follows the entry in the body: the `;` is not needed)
                    let sep = if entry.next_sibling().is_none() {
                        ",".to_string()
                    } else {
                        format!(",\n{indent};")
                    };
                    edits.push((s, e, sep));
                }
            } else {
                let end = usize::from(last.text_range().end());
                edits.push((end, end, ",".into()));
            }
        }
    }
}

/// A list that spans lines in front of its ending token (`trailing_comma_before`: pairs of a node
/// kind and the token text) gets a trailing comma, and the token goes on a line of its own, at the
/// indentation of the line the node starts on. Only a list with a comma in it counts.
pub fn trailing_commas_before(
    grammar: &Grammar,
    tree: &SyntaxNode,
    text: &str,
    options: &core_config::ResolvedOptions,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let pairs: Vec<&str> = options
        .strings(core_config::opt::TRAILING_COMMA_BEFORE)
        .collect();
    for pair in pairs.chunks(2) {
        let [kind, end] = pair else { continue };
        for (at, _) in text.match_indices(*end) {
            let Some(t) = u32::try_from(at)
                .ok()
                .and_then(|at| tree.token_at_offset(at.into()).right_biased())
                .filter(|t| t.text() == *end)
            else {
                continue;
            };
            let Some(node) = t.parent().filter(|n| grammar.name(n.kind()) == *kind) else {
                continue;
            };
            let first = node
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|x| !grammar.is_trivia(x.kind()));
            let mut prev = t.prev_token();
            while let Some(p) = prev.as_ref().filter(|p| p.text().trim().is_empty()) {
                prev = p.prev_token();
            }
            let (Some(first), Some(prev)) = (first, prev) else {
                continue;
            };
            let (from, to) = (
                usize::from(prev.text_range().end()),
                usize::from(t.text_range().start()),
            );
            let start = usize::from(first.text_range().start());
            let has_comma = node
                .children_with_tokens()
                .any(|e| e.as_token().is_some_and(|c| c.text() == ","));
            if !has_comma
                || prev.text() == ","
                || grammar.is_trivia(prev.kind())
                || !text[start..to].contains('\n')
                || !text[from..to].trim().is_empty()
            {
                continue;
            }
            let line = text[..start].rfind('\n').map_or(0, |i| i + 1);
            let indent = &text[line..start];
            if !indent.trim().is_empty() {
                continue;
            }
            edits.push((from, to, format!(",\n{indent}")));
        }
    }
}

/// Does the text have a line that starts with one of `keywords`, has a body after its
/// parenthesized head on the same line that does not start a `{` block, and is followed by a
/// deeper-indented line (a statement the layout wrapped, whose braces a rewrite that depends on
/// the width decides again)?
pub fn has_wrapped_statement(text: &str, keywords: &[&str]) -> bool {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.next() {
        let t = l.trim_start();
        let Some(rest) = keywords
            .iter()
            .find_map(|k| t.strip_prefix(*k).filter(|r| r.starts_with([' ', '('])))
        else {
            continue;
        };
        let rest = rest.trim_start();
        let body = if rest.starts_with('(') {
            let mut depth = 0;
            let closed = rest.char_indices().find(|&(_, c)| {
                depth += i32::from(c == '(') - i32::from(c == ')');
                depth == 0
            });
            // (a head that goes on to the next line leaves the body to a later round)
            let Some((at, _)) = closed else { continue };
            &rest[at + 1..]
        } else {
            rest
        };
        let code = body.split_once("//").map_or(body, |(c, _)| c).trim();
        if code.is_empty()
            || code.starts_with(['{', '('])
            || code.ends_with('{')
            || code.starts_with("if ")
        {
            continue;
        }
        if lines
            .peek()
            .is_some_and(|n| !n.trim().is_empty() && indent(n) > indent(l))
        {
            return true;
        }
    }
    false
}
