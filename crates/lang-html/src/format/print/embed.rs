//! Code of other languages inside the markup: the content of `<script>` and `<style>` and the
//! values of `style`, `srcset` and event-handler attributes. The engine formats the code
//! (`Opts::embed`); this module lays the lines out. Code that does not parse stays as written.
//!
//! concept: embedded-code

use super::{
    Embed, P, TEMPLATE, dedent, embed_text, s, trim_preserve_indentation, unescape_quotes,
};
use crate::format::model::{Dialect, N};
use core_format::EmbedMode;
use core_format::context::verbatim_text;
use core_format::doc::{self, Doc};

fn attr<'a>(n: &'a N, name: &str) -> Option<&'a str> {
    (n.attrs.iter())
        .find(|a| a.name.eq_ignore_ascii_case(name))
        .and_then(|a| a.value)
}

/// The language of the content of a `<script>` (Prettier's `inferScriptParser`).
fn script_language(n: &N) -> Option<&'static str> {
    if let Some(lang) = attr(n, "lang") {
        return match lang {
            "ts" => Some("ts"),
            "tsx" => Some("tsx"),
            "" | "js" | "jsx" => Some("js"),
            _ => None,
        };
    }
    match attr(n, "type").map(str::to_ascii_lowercase).as_deref() {
        None
        | Some(
            ""
            | "module"
            | "text/javascript"
            | "text/babel"
            | "text/jsx"
            | "application/javascript",
        ) => Some("js"),
        Some("application/x-typescript") => Some("ts"),
        Some("text/markdown") => Some("markdown"),
        Some("text/html") => Some("html"),
        Some(t) if t.ends_with("json") || t.ends_with("importmap") || t == "speculationrules" => {
            Some("json")
        }
        _ => None,
    }
}

fn style_language(n: &N) -> Option<&'static str> {
    match attr(n, "lang") {
        None | Some("css" | "postcss") => Some("css"),
        Some("scss") => Some("scss"),
        Some("less") => Some("less"),
        _ => None,
    }
}

pub(super) fn lines(code: &str) -> Doc {
    embed_text::lines(code, TEMPLATE)
}

/// An event handler: an expression when it parses as one (`f();` is the expression `f()` too), else
/// statements; statements that format to one expression (`fn => x` gets its parentheses) are one,
/// so that formatting again gives the same text.
fn handler(embed: Embed, language: &str, value: &str, width: usize) -> Option<(String, bool)> {
    let expr = |code: &str| embed(language, EmbedMode::Expression, code, width, true);
    if let Some(out) = expr(value.trim_end().trim_end_matches(';')) {
        return Some((out, false));
    }
    let out = embed(language, EmbedMode::Program, value, width, true)?;
    Some(match expr(out.trim_end().trim_end_matches(';')) {
        Some(one) => (one, false),
        None => (out, true),
    })
}

/// `name="` body `"`: the body stays on the line when it fits, else it is indented on lines of
/// its own.
fn expand(name: &str, body: Doc) -> Doc {
    expand_with(&format!("{name}=\""), "\"", body)
}

/// Does the code hold a `// comment` (outside strings)? It ends the line, so the expression breaks.
fn has_line_comment(code: &str) -> bool {
    let (mut quote, mut escaped, mut prev) = (None::<char>, false, ' ');
    for c in code.chars() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
        } else if matches!(c, '\'' | '"' | '`') {
            quote = Some(c);
        } else if c == '/' && prev == '/' {
            return true;
        }
        prev = c;
    }
    false
}

/// `{ a: 1, b: 2 }` as an object that is broken: a property per line, each with its comma.
fn explode_object(flat: &str, tab: usize) -> Option<String> {
    let inner = flat.strip_prefix("{ ")?.strip_suffix(" }")?;
    let (mut depth, mut start, mut quote, mut escaped) = (0i32, 0, None::<char>, false);
    let mut props = Vec::new();
    for (i, c) in inner.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                props.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    props.push(&inner[start..]);
    let pad = " ".repeat(tab);
    let lines: Vec<String> = props
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| format!("{pad}{p},"))
        .collect();
    (depth == 0 && !lines.is_empty()).then(|| format!("{{\n{}\n}}", lines.join("\n")))
}

fn expand_with(open: &str, close: &str, body: Doc) -> Doc {
    doc::concat(vec![
        s(open),
        doc::group(doc::concat(vec![
            doc::indent(doc::concat(vec![doc::softline(), body])),
            doc::softline(),
        ])),
        s(close),
    ])
}

impl P<'_> {
    /// The formatted content of a `<script>` or `<style>`, when its language is known.
    pub(super) fn embedded_content(&self, id: usize) -> Option<Doc> {
        let (n, embed) = (self.n(id), self.o.embed?);
        let p = self.n(n.parent);
        let language = match p.name.as_ref() {
            "script" => script_language(p)?,
            "style" => style_language(p)?,
            _ => return None,
        };
        let levels = self.depth(n.parent) + usize::from(!self.unindented(n.parent));
        let width = (self.o.width).saturating_sub(levels * self.o.tab);
        let code = dedent(trim_preserve_indentation(&n.value));
        let out = embed(language, EmbedMode::Program, &code, width, false)?;
        (!out.is_empty()).then(|| lines(&out))
    }

    /// The attribute `name="value"` of element `id` with its value formatted, when it has a
    /// language.
    pub(super) fn embedded_attribute(&self, id: usize, name: &str, value: &str) -> Option<Doc> {
        let lower = name.to_ascii_lowercase();
        let value = unescape_quotes(value);
        if lower == "style" && value.trim().is_empty() {
            return Some(s(format!("{name}=\"\"")));
        }
        let iframe_allow = lower == "allow" && self.n(id).name == "iframe";
        if iframe_allow
            && value
                .trim_matches(|c: char| c.is_whitespace() || c == ';')
                .is_empty()
        {
            return Some(s(format!("{name}=\"\"")));
        }
        if value.trim().is_empty()
            || value.contains("{{")
            || (self.o.dialect == Dialect::Svelte && value.contains('{'))
        {
            return None;
        }
        if iframe_allow {
            let parts = value
                .split(';')
                .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "));
            let parts: Vec<String> = parts.filter(|p| !p.is_empty()).collect();
            let last = parts.len().checked_sub(1)?;
            let body = parts.iter().enumerate().map(|(i, p)| {
                if i == last {
                    doc::concat(vec![s(p), doc::if_break(s(";"), doc::nil())])
                } else {
                    s(format!("{p};"))
                }
            });
            return Some(expand(name, doc::join(&doc::line(), body)));
        }
        if lower == "srcset" && matches!(self.n(id).name.as_ref(), "img" | "source") {
            return srcset(name, &value);
        }
        if self.o.dialect == Dialect::Vue
            && let Some(d) = self.vue_attribute(id, name, &lower, &value)
        {
            return Some(d);
        }
        let embed = self.o.embed?;
        let indent = (self.depth(id) + 1) * self.o.tab;
        let width = self.o.width.saturating_sub(indent + self.o.tab);
        let (mode, language) = match lower.as_str() {
            "style" => (EmbedMode::Declarations, "css"),
            l if l.len() > 2
                && l.starts_with("on")
                && l.bytes().all(|b| b.is_ascii_lowercase()) =>
            {
                (EmbedMode::Program, "js")
            }
            _ => return None,
        };
        let (out, mode) = if mode == EmbedMode::Program {
            let (out, statements) = handler(embed, language, &value, width)?;
            (
                out,
                if statements {
                    mode
                } else {
                    EmbedMode::Expression
                },
            )
        } else {
            (embed(language, mode, &value, width, true)?, mode)
        };
        if out.is_empty() {
            return None;
        }
        let out = out.replace('"', "&quot;");
        let rows: Vec<&str> = out.split('\n').collect();
        if mode == EmbedMode::Declarations
            && rows.iter().all(|r| r.ends_with(';') && !r.starts_with(' '))
        {
            let last = rows.len() - 1;
            let decls = rows.iter().enumerate().map(|(i, r)| {
                if i == last {
                    doc::concat(vec![
                        s(&r[..r.len() - 1]),
                        doc::if_break(s(";"), doc::nil()),
                    ])
                } else {
                    s(*r)
                }
            });
            return Some(expand(name, doc::join(&doc::line(), decls)));
        }
        Some(expand(name, lines(&out)))
    }
}

impl P<'_> {
    /// The language of the expressions of a Vue file: TypeScript when its script is.
    fn expression_language(&self) -> &'static str {
        if self.o.dialect == Dialect::Angular {
            return "ts";
        }
        let ts = self.t.n[0].kids.iter().any(|&c| {
            let n = self.n(c);
            n.name == "script" && matches!(attr(n, "lang"), Some("ts" | "tsx"))
        });
        if ts { "ts" } else { "js" }
    }

    /// `{{ expression }}` on one line, when the expression formats to one line.
    pub(super) fn interpolation_text(&self, id: usize) -> Option<String> {
        let out = self.formatted_interpolation(id)?;
        (!out.contains('\n')).then(|| format!("{{{{ {out} }}}}"))
    }

    fn formatted_interpolation(&self, id: usize) -> Option<String> {
        let n = self.n(id);
        let code = n.value.trim();
        let width = self
            .o
            .width
            .saturating_sub((self.depth(id) + 1) * self.o.tab);
        if self.o.dialect == Dialect::Angular {
            self.ng_expression(code, width, false, super::angular::Ng::Nested)
        } else {
            self.o.embed.and_then(|e| {
                e(
                    self.expression_language(),
                    EmbedMode::Expression,
                    code,
                    width,
                    false,
                )
            })
        }
    }

    /// `{{ expression }}`: the expression on the line when it fits, else on lines of its own.
    pub(super) fn interpolation(&self, id: usize) -> Doc {
        let n = self.n(id);
        let code = n.value.trim();
        if code.is_empty() {
            return doc::concat(vec![s(self.open_start(id)), s(self.close_end(id))]);
        }
        let out = self.formatted_interpolation(id);
        // An expression that is not formatted keeps its lines, with the indentation they share
        // removed so that formatting again does not indent them further.
        let kept = || match code.split_once('\n') {
            Some((first, rest)) => format!("{first}\n{}", dedent(rest)),
            None => code.to_string(),
        };
        let borrowed = n.next.is_some_and(|x| self.borrow_prev_close_end(x));
        let broken = out.as_deref().is_some_and(has_line_comment);
        let doc = doc::concat(vec![
            if broken {
                doc::break_parent()
            } else {
                doc::nil()
            },
            s(self.open_start(id)),
            doc::indent(doc::concat(vec![
                doc::line(),
                lines(&out.unwrap_or_else(kept)),
            ])),
            if borrowed { s(" ") } else { doc::line() },
            s(self.close_end(id)),
        ]);
        // An interpolation that fits on its own line stays on it when only the text after it does
        // not: the line after it breaks instead. (Prettier breaks the interpolation too, and its
        // output then changes on a second format.)
        if borrowed { doc } else { doc::group(doc) }
    }

    /// A Vue directive (`:a`, `@a`, `#a`, `v-*`) whose value is an expression.
    fn vue_attribute(&self, id: usize, name: &str, lower: &str, value: &str) -> Option<Doc> {
        let embed = self.o.embed?;
        let key = lower.split([':', '.', '[']).next().unwrap_or("");
        let language = self.expression_language();
        if !(lower.starts_with([':', '@', '#', '.'])
            || key.starts_with("v-")
            || key == "slot-scope")
        {
            return None;
        }
        // The formatted value for a width, and whether it is statements.
        let run = |width: usize| -> Option<(String, bool)> {
            let expr = |code: &str| embed(language, EmbedMode::Expression, code, width, true);
            if key == "v-for" {
                return v_for(value, expr).map(|o| (o, false));
            }
            if lower.starts_with('@') || key == "v-on" {
                return handler(embed, language, value, width);
            }
            expr(value).map(|o| (o, false))
        };
        self.attr_expression(
            id,
            &format!("{name}=\""),
            "\"",
            &|o| o.replace('"', "&quot;"),
            &run,
        )
    }

    /// `open` the formatted expression `close`: a string or template literal stays on the line, an
    /// object, array or function hugs the delimiters, anything else goes on lines of its own when
    /// it does not fit. `run` formats for a width and says whether the result is statements.
    pub(super) fn attr_expression(
        &self,
        id: usize,
        open: &str,
        close: &str,
        fix: &dyn Fn(String) -> String,
        run: &dyn Fn(usize) -> Option<(String, bool)>,
    ) -> Option<Doc> {
        let indent = (self.depth(id) + 1) * self.o.tab;
        let (flat, statements) = run(self
            .o
            .width
            .saturating_sub(indent + open.len() + close.len()))?;
        let hugged = |flat: String| doc::concat(vec![s(open), lines(&fix(flat)), s(close)]);
        let quote = flat
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"' | '`'));
        let literal = quote.is_some_and(|q| {
            flat.len() > 1 && flat.ends_with(q) && !flat[1..flat.len() - 1].contains(q)
        });
        if literal {
            return Some(hugged(flat));
        }
        if !flat.contains('\n') {
            if has_line_comment(&flat) {
                return Some(doc::concat(vec![
                    s(open),
                    doc::indent(doc::concat(vec![doc::hardline(), s(fix(flat))])),
                    doc::hardline(),
                    s(close),
                ]));
            }
            return Some(expand_with(open, close, s(fix(flat))));
        }
        let first = flat.lines().next().unwrap_or("");
        let hug = !statements
            && (flat.starts_with(['{', '['])
                || flat.starts_with("function")
                || (first.contains("=>") && first.ends_with('{')));
        if hug {
            // Only the first and last line sit beside the quotes: the lines between have the
            // attribute's indentation to themselves.
            // (Angular only: the rerun costs another format of the expression)
            let wide = (self.o.dialect == Dialect::Angular)
                .then(|| run(self.o.width.saturating_sub(indent)))
                .flatten();
            if let Some((wide, _)) = wide {
                if wide.contains('\n') {
                    return Some(hugged(wide));
                }
                // The object is too wide for the line it starts on, so it breaks: a property per
                // line, each as it fits the wide width.
                if let Some(exploded) = explode_object(&wide, self.o.tab) {
                    return Some(hugged(exploded));
                }
            }
            return Some(hugged(flat));
        }
        let (broken, _) = run(self.o.width.saturating_sub(indent + self.o.tab))?;
        Some(expand_with(open, close, lines(&fix(broken))))
    }
}

impl P<'_> {
    /// The formatted expression `code` of node `id`, when it parses.
    fn format_expression(&self, id: usize, code: &str) -> Option<String> {
        let width = self
            .o
            .width
            .saturating_sub((self.depth(id) + 1) * self.o.tab);
        let embed = self.o.embed?;
        embed(
            self.expression_language(),
            EmbedMode::Expression,
            code,
            width,
            false,
        )
    }

    /// Svelte's `{expression}`, `{@html expression}`.
    pub(super) fn mustache(&self, id: usize) -> Doc {
        let inner = self.n(id).value.trim();
        let (keyword, code) = match inner.strip_prefix('@') {
            Some(rest) => {
                let (k, c) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                (format!("@{k} "), c.trim())
            }
            None => (String::new(), inner),
        };
        let out = self.format_expression(id, code);
        if !keyword.is_empty() {
            // `{@const a = b}` hugs the braces.
            return doc::concat(vec![
                s(self.open_start(id) + &keyword),
                lines(out.as_deref().unwrap_or(code)),
                s(self.close_end(id)),
            ]);
        }
        doc::concat(vec![
            s(self.open_start(id) + &keyword),
            doc::group(doc::concat(vec![
                doc::indent(doc::concat(vec![
                    doc::softline(),
                    lines(out.as_deref().unwrap_or(code)),
                ])),
                doc::softline(),
            ])),
            s(self.close_end(id)),
        ])
    }

    /// `{#if x}`, `{:else}`...: the condition of `if`, `else if` and `key` is formatted.
    fn branch_tag(&self, id: usize, tag: &str) -> Doc {
        // One brace each side: a destructuring pattern ends in `}}`.
        let inner = tag.strip_prefix('{').unwrap_or(tag);
        let inner = inner.strip_suffix('}').unwrap_or(inner).trim();
        let formatted = ["#if ", ":else if ", "#key "].iter().find_map(|p| {
            let out = self.format_expression(id, inner.strip_prefix(p)?.trim())?;
            Some(doc::concat(vec![s(format!("{{{p}")), lines(&out), s("}")]))
        });
        formatted.unwrap_or_else(|| {
            s(format!(
                "{{{}}}",
                inner.split_whitespace().collect::<Vec<_>>().join(" ")
            ))
        })
    }

    /// `{#if}` ... `{/if}`: each branch's tag with its content indented below.
    pub(super) fn block_doc(&self, id: usize) -> Doc {
        let n = self.n(id);
        let mut parts = Vec::new();
        for (i, &b) in n.kids.iter().enumerate() {
            if i > 0 {
                parts.push(doc::hardline());
            }
            let branch = self.n(b);
            parts.push(self.branch_tag(id, &branch.value));
            if !branch.kids.is_empty() {
                parts.push(doc::indent(doc::concat(vec![
                    doc::hardline(),
                    self.children(b),
                ])));
            }
        }
        if !n.value.is_empty() {
            parts.push(doc::hardline());
            parts.push(s(n.value.trim()));
        }
        doc::concat(parts)
    }

    /// `name={expression}` (Svelte).
    pub(super) fn braced_attribute(&self, id: usize, name: &str, value: &str) -> Option<Doc> {
        let embed = self.o.embed?;
        let code = value.strip_prefix('{')?.strip_suffix('}')?.trim();
        let language = self.expression_language();
        let run = |width: usize| {
            embed(language, EmbedMode::Expression, code, width, false).map(|o| (o, false))
        };
        self.attr_expression(id, &format!("{name}={{"), "}", &|o| o, &run)
    }
}

impl P<'_> {
    /// A root block of a Vue file that is not markup: formatted by its `lang` or `type`, else kept.
    pub(super) fn custom_block_formatted(&self, id: usize) -> Option<Doc> {
        let n = self.n(id);
        let language = match (attr(n, "lang"), attr(n, "type")) {
            (Some("json"), _) | (None, Some("application/json")) => "json",
            (Some("yaml" | "yml"), _) => "yaml",
            (Some("graphql" | "gql"), _) => "graphql",
            (Some("md" | "markdown"), _) => "markdown",
            (Some("js" | "javascript"), _) => "js",
            (Some("ts" | "typescript"), _) => "ts",
            _ => return None,
        };
        let (Some(embed), Some(close), true) = (self.o.embed, n.close_start, n.name != "template")
        else {
            return None;
        };
        let code = dedent(trim_preserve_indentation(&self.t.src[n.tag_end..close]));
        match embed(language, EmbedMode::Program, &code, self.o.width, false) {
            Some(out) if !out.is_empty() => Some(doc::concat(vec![
                verbatim_text(&self.t.src[n.start..n.tag_end]),
                doc::hardline(),
                lines(&out),
                doc::hardline(),
                s(format!("</{}>", n.name)),
            ])),
            _ => None,
        }
    }
}

impl P<'_> {
    /// Front matter: YAML between `---`, TOML between `+++`; formatted, else kept.
    pub(super) fn front_matter(&self, id: usize) -> Doc {
        let whole = &*self.n(id).value;
        let raw = verbatim_text(whole);
        let (first, rest) = whole.split_once('\n').unwrap_or((whole, ""));
        let delim = &first[..3];
        let language = match (delim, first[3..].trim()) {
            ("---", "") => "yaml",
            ("+++", "") => "toml",
            _ => return raw,
        };
        let body = &rest[..rest.rfind('\n').map_or(0, |i| i + 1)];
        let formatted = match (self.o.embed, body.trim().is_empty()) {
            (_, true) => Some(String::new()),
            (Some(embed), false) => embed(language, EmbedMode::Program, body, self.o.width, false),
            _ => None,
        };
        match formatted {
            Some(out) if out.is_empty() => s(format!("{delim}\n{delim}")),
            Some(out) => doc::concat(vec![
                s(delim),
                doc::hardline(),
                lines(&out),
                doc::hardline(),
                s(delim),
            ]),
            None => raw,
        }
    }
}

/// `item in items`, `(item, index) of list`: the loop variables as written (parenthesized when
/// there are several) and the collection formatted by `expr`.
fn v_for(value: &str, expr: impl Fn(&str) -> Option<String>) -> Option<String> {
    let (at, kw) = [" in ", " of "]
        .iter()
        .filter_map(|k| value.find(k).map(|i| (i, *k)))
        .min()?;
    let left = value[..at].trim();
    let inner = left
        .strip_prefix('(')
        .and_then(|l| l.strip_suffix(')'))
        .unwrap_or(left);
    let left = if inner.contains(['{', '[']) {
        left.to_string()
    } else {
        let names: Vec<&str> = inner.split(',').map(str::trim).collect();
        if left.starts_with('(') || names.len() > 1 {
            format!("({})", names.join(", "))
        } else {
            names[0].to_string()
        }
    };
    let right = value[at + kw.len()..].trim();
    Some(format!("{left}{kw}{}", expr(right)?))
}

/// `a.png 1x, b.png 2x`: the candidates one per line, descriptors aligned, when too wide.
fn srcset(name: &str, value: &str) -> Option<Doc> {
    if value.contains("data:") {
        return None;
    }
    // The candidates as the HTML spec reads them: a URL runs to a blank, so it may hold commas.
    let mut items = Vec::new();
    let mut rest = value.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    while !rest.is_empty() {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (url, after) = rest.split_at(end);
        let desc = if url.ends_with(',') {
            rest = after;
            ""
        } else {
            let (d, more) = after.split_once(',').unwrap_or((after, ""));
            rest = more;
            d.trim()
        };
        let unit = desc.chars().last();
        let number = &desc[..desc.len().saturating_sub(1)];
        if !desc.is_empty()
            && !(matches!(unit, Some('w' | 'x' | 'h'))
                && number.parse::<f64>().is_ok_and(|n| n >= 0.0))
        {
            return None;
        }
        items.push((url.trim_end_matches(','), desc));
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    }
    let left = |d: &str| d.find('.').unwrap_or(d.len().saturating_sub(1));
    let max_url = items.iter().map(|(u, _)| u.len()).max()?;
    let max_left = items.iter().map(|(_, d)| left(d)).max()?;
    let parts = items.iter().map(|&(url, desc)| {
        if desc.is_empty() {
            return s(url);
        }
        let pad = max_url - url.len() + 1 + max_left - left(desc);
        doc::concat(vec![
            s(url),
            doc::if_break(s(" ".repeat(pad)), s(" ")),
            s(desc),
        ])
    });
    let sep = doc::concat(vec![s(","), doc::line()]);
    Some(expand(name, doc::join(&sep, parts)))
}
