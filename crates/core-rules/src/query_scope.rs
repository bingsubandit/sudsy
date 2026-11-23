//! Query analysis over the node kinds a language names in the params of its rules: the sources of
//! a select (its `FROM` elements, with alias, table name and a nested query), its column
//! references and the aliases of its select elements. Three families read it:
//! `unknown-columns` (a wildcard of the outermost query that resolves to a table whose columns
//! can change), `reference-qualification` (a select over several sources has an unqualified
//! reference) and `reference-consistency` (a select over one source mixes qualified and
//! unqualified references).
//!
//! Params of all three: `select` (the select node), `queries` (the kinds of a query: a select,
//! a set operation, a `WITH` statement), `element` (a select element), `source` (a `FROM` item),
//! `alias`, `reference`, `table` (the node whose child reference names a table), `cte`, `from`
//! (the `FROM` clause) and `wildcard` (the token text of a wildcard). Messages have `{name}`.
//!
//! concept: query-scope
//! what: The sources, references and select-element aliases of a select, for rules that need to know which table a column belongs to.
//! tools: sqlfluff AM04, AL05, RF02, RF03
//! reads: rule:unknown-columns, rule:reference-qualification, rule:reference-consistency, rule:unused-alias

use crate::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

pub const UNKNOWN_COLUMNS: Family = Family {
    name: "unknown-columns",
    build: |meta, p| build(meta, p, Check::UnknownColumns),
};

pub const REFERENCE_QUALIFICATION: Family = Family {
    name: "reference-qualification",
    build: |meta, p| build(meta, p, Check::Qualification),
};

pub const UNIQUE_ALIASES: Family = Family {
    name: "unique-aliases",
    build: |meta, p| build(meta, p, Check::UniqueAliases),
};

pub const UNUSED_ALIAS: Family = Family {
    name: "unused-alias",
    build: |meta, p| build(meta, p, Check::UnusedAlias),
};

pub const REFERENCE_CONSISTENCY: Family = Family {
    name: "reference-consistency",
    build: |meta, p| build(meta, p, Check::Consistency),
};

#[derive(Clone, Copy)]
enum Check {
    UnknownColumns,
    Qualification,
    Consistency,
    UniqueAliases,
    UnusedAlias,
}

struct Names {
    select: String,
    queries: Vec<String>,
    element: String,
    source: String,
    alias: String,
    reference: String,
    table: String,
    cte: String,
    from: String,
    wildcard: String,
}

struct QueryRule {
    meta: &'static RuleMeta,
    message: String,
    names: Names,
    check: Check,
}

fn build(meta: &'static RuleMeta, mut p: Params, check: Check) -> Result<Box<dyn Rule>, String> {
    let names = Names {
        select: p.text("select")?,
        queries: p.text("queries")?.split(',').map(str::to_string).collect(),
        element: p.text("element")?,
        source: p.text("source")?,
        alias: p.text("alias")?,
        reference: p.text("reference")?,
        table: p.text("table")?,
        cte: p.text("cte")?,
        from: p.text("from")?,
        wildcard: p.text("wildcard")?,
    };
    let rule = QueryRule {
        meta,
        message: p.text("message")?,
        names,
        check,
    };
    p.done()?;
    Ok(Box::new(rule))
}

/// The kinds of the params, resolved against the file's grammar.
struct Kinds {
    select: SyntaxKind,
    queries: Vec<SyntaxKind>,
    element: SyntaxKind,
    source: SyntaxKind,
    alias: SyntaxKind,
    reference: SyntaxKind,
    table: SyntaxKind,
    cte: SyntaxKind,
    from: SyntaxKind,
    wildcard: String,
}

impl Names {
    fn resolve(&self, g: &Grammar) -> Option<Kinds> {
        let k = |n: &str| g.kind_by_name(n);
        Some(Kinds {
            select: k(&self.select)?,
            queries: self.queries.iter().filter_map(|n| k(n)).collect(),
            element: k(&self.element)?,
            source: k(&self.source)?,
            alias: k(&self.alias)?,
            reference: k(&self.reference)?,
            table: k(&self.table)?,
            cte: k(&self.cte)?,
            from: k(&self.from)?,
            wildcard: self.wildcard.clone(),
        })
    }
}

/// A `FROM` item of a select.
struct Source {
    /// A function or a list of values: no table, so it neither counts as a source nor
    /// qualifies a column; its alias names columns.
    standalone: bool,
    /// The name a column reference qualifies it with: its alias, else its table name.
    name: Option<String>,
    /// The alias written for it, with its range.
    alias: Option<(String, TextRange)>,
    /// The query inside it (a subquery), or the common table it names.
    query: Option<SyntaxNode>,
}

fn is_name(t: &SyntaxToken, k: &Kinds, g: &Grammar) -> bool {
    let first = t.text().chars().next().unwrap_or(' ');
    !g.is_trivia(t.kind())
        && t.text() != k.wildcard
        && (first.is_alphanumeric() || matches!(first, '_' | '"' | '`' | '[' | '@'))
}

/// The nodes under `root`, not entering a node of `stop` kinds (the root itself may be one).
fn own(root: &SyntaxNode, stop: &[SyntaxKind]) -> Vec<SyntaxNode> {
    let mut out = Vec::new();
    let mut walk = root.preorder();
    while let Some(event) = walk.next() {
        if let rowan::WalkEvent::Enter(n) = event {
            if n != *root && stop.contains(&n.kind()) {
                walk.skip_subtree();
                continue;
            }
            out.push(n);
        }
    }
    out
}

/// The name tokens of a reference or alias node: its direct identifier tokens, in order.
fn parts(n: &SyntaxNode, k: &Kinds, g: &Grammar) -> Vec<String> {
    (n.children_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| is_name(t, k, g) && !matches!(t.text().to_ascii_lowercase().as_str(), "as"))
        .map(|t| t.text().trim_matches(['"', '`']).to_ascii_lowercase())
        .collect()
}

/// The last name of a node, lower case and without quotes, with its range.
fn last_part(n: &SyntaxNode, k: &Kinds, g: &Grammar) -> Option<(String, TextRange)> {
    let t = (n.children_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| is_name(t, k, g) && !t.text().eq_ignore_ascii_case("as"))
        .last()?;
    Some((
        t.text().trim_matches(['"', '`', '\'']).to_ascii_lowercase(),
        t.text_range(),
    ))
}

fn text_has(n: &SyntaxNode, text: &str) -> bool {
    n.children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| t.text() == text)
}

fn is_qualified(n: &SyntaxNode) -> bool {
    text_has(n, ".")
}

impl Kinds {
    fn is_query(&self, kind: SyntaxKind) -> bool {
        self.queries.contains(&kind)
    }

    /// The common table named `name` in a statement around `node`.
    fn cte(&self, node: &SyntaxNode, name: &str, g: &Grammar) -> Option<SyntaxNode> {
        for up in node.ancestors() {
            for c in up.children().filter(|c| c.kind() == self.cte) {
                if parts(&c, self, g).first().map(String::as_str) == Some(name) {
                    return self.inner_query(&c);
                }
            }
        }
        None
    }

    /// The first query under `node` (a subquery or a common table's body).
    fn inner_query(&self, node: &SyntaxNode) -> Option<SyntaxNode> {
        node.descendants().skip(1).find(|n| self.is_query(n.kind()))
    }

    fn sources(&self, select: &SyntaxNode, g: &Grammar) -> Vec<Source> {
        let Some(from) = select.children().find(|c| c.kind() == self.from) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for s in own(&from, &self.queries)
            .into_iter()
            .filter(|n| n.kind() == self.source)
        {
            let alias = s.children().find(|c| c.kind() == self.alias);
            let table = s
                .children()
                .find(|c| c.kind() == self.table)
                .and_then(|t| t.children().find(|c| c.kind() == self.reference));
            let table_name = table.as_ref().and_then(|t| parts(t, self, g).pop());
            let alias_name = alias.as_ref().and_then(|a| parts(a, self, g).pop());
            let query = match (&table, &table_name) {
                (Some(_), Some(name)) => self.cte(select, name, g),
                _ => None,
            }
            .or_else(|| {
                // A subquery: the query inside it, when the item has no table of its own.
                if table.is_none() {
                    self.inner_query(&s)
                } else {
                    None
                }
            });
            out.push(Source {
                standalone: table.is_none()
                    && query.is_none()
                    && alias.as_ref().is_none_or(|a| a.children().next().is_some()),
                alias: alias.as_ref().and_then(|a| last_part(a, self, g)),
                name: alias_name.or(table_name),
                query,
            });
        }
        out
    }

    /// The sources a select counts for RF02: its own tables, and those of the select around it
    /// when it sits in a condition rather than in a `FROM` clause.
    fn parent_sources(&self, select: &SyntaxNode, g: &Grammar, own: &[Source]) -> Vec<String> {
        let mut out: Vec<String> = (own.iter().filter(|s| !s.standalone))
            .map(|s| s.name.clone().unwrap_or_default())
            .collect();
        let mut in_from = false;
        let mut parent = None;
        for a in select.ancestors().skip(1) {
            if a.kind() == self.from {
                in_from = true;
            }
            if a.kind() == self.select {
                parent = Some(a);
                break;
            }
        }
        if let (Some(p), false) = (parent, in_from) {
            out.extend(
                (self.sources(&p, g).into_iter().filter(|s| !s.standalone))
                    .map(|s| s.name.unwrap_or_default()),
            );
        }
        out
    }

    /// The column references of a select: its own, not those of a nested query, and not the
    /// table names of its `FROM` items.
    fn references(&self, select: &SyntaxNode) -> Vec<SyntaxNode> {
        own(select, &self.queries)
            .into_iter()
            .filter(|n| n.kind() == self.reference)
            .filter(|n| n.parent().is_none_or(|p| p.kind() != self.table))
            .collect()
    }

    /// The names of the aliases of the select's elements, and per element the node.
    fn aliases(&self, select: &SyntaxNode, g: &Grammar) -> Vec<(SyntaxNode, String)> {
        let mut out = Vec::new();
        for e in own(select, &self.queries)
            .into_iter()
            .filter(|n| n.kind() == self.element)
        {
            if let Some(a) = e.children().find(|c| c.kind() == self.alias)
                && let Some(name) = parts(&a, self, g).pop()
            {
                out.push((e.clone(), name));
            }
        }
        out
    }

    /// The wildcard of a reference: its token.
    fn star(&self, r: &SyntaxNode) -> Option<TextRange> {
        r.children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.text() == self.wildcard)
            .map(|t| t.text_range())
    }

    /// The selects of a query: itself, or the selects of a set operation or statement (not
    /// the ones nested in a common table or a subquery).
    fn selects(&self, query: &SyntaxNode) -> Vec<SyntaxNode> {
        if query.kind() == self.select {
            return vec![query.clone()];
        }
        let mut out = Vec::new();
        let mut walk = query.preorder();
        while let Some(event) = walk.next() {
            if let rowan::WalkEvent::Enter(n) = event {
                if n == *query {
                    continue;
                }
                if n.kind() == self.select {
                    out.push(n);
                    walk.skip_subtree();
                } else if n.kind() == self.cte {
                    walk.skip_subtree();
                }
            }
        }
        out
    }

    /// Does a wildcard of this query resolve to a table whose columns can change? `Err` holds
    /// the wildcard it started from.
    fn analyze(
        &self,
        query: &SyntaxNode,
        anchor: Option<TextRange>,
        g: &Grammar,
        depth: u32,
    ) -> Result<(), TextRange> {
        if depth > 24 {
            return Ok(());
        }
        for select in self.selects(query) {
            let sources: Vec<Source> = self
                .sources(&select, g)
                .into_iter()
                .filter(|s| !s.standalone)
                .collect();
            for element in own(&select, &self.queries)
                .into_iter()
                .filter(|n| n.kind() == self.element)
            {
                for r in element.children().filter(|c| c.kind() == self.reference) {
                    let Some(star) = self.star(&r) else { continue };
                    let anchor = anchor.unwrap_or(star);
                    if is_qualified(&r) {
                        let name = parts(&r, self, g).pop().unwrap_or_default();
                        match sources.iter().find(|s| s.name.as_deref() == Some(&name)) {
                            Some(s) => match &s.query {
                                Some(q) => self.analyze(q, Some(anchor), g, depth + 1)?,
                                None => return Err(anchor),
                            },
                            None => match self.cte(&select, &name, g) {
                                Some(q) => self.analyze(&q, Some(anchor), g, depth + 1)?,
                                None => return Err(anchor),
                            },
                        }
                    } else {
                        match sources.iter().find_map(|s| s.query.as_ref()) {
                            Some(q) => {
                                self.analyze(q, Some(anchor), g, depth + 1)?;
                                return Ok(());
                            }
                            None => return Err(anchor),
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Rule for QueryRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
    }

    fn check_file(&self, file: &crate::FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let g = file.grammar;
        let Some(k) = self.names.resolve(g) else {
            return;
        };
        match self.check {
            Check::UnknownColumns => {
                for q in ctx.nodes(&k.queries) {
                    if q.ancestors().skip(1).any(|a| k.is_query(a.kind())) {
                        continue;
                    }
                    if let Err(at) = k.analyze(&q, None, g, 0) {
                        ctx.report(at, self.message.clone());
                    }
                }
            }
            Check::Qualification => {
                for select in ctx.nodes(&[k.select]) {
                    let own = k.sources(&select, g);
                    if !own.iter().any(|s| !s.standalone)
                        || k.parent_sources(&select, g, &own).len() < 2
                    {
                        continue;
                    }
                    let aliases = k.aliases(&select, g);
                    let mut using = using_columns(&select, &k, g);
                    using.extend(
                        own.iter()
                            .filter(|s| s.standalone)
                            .filter_map(|s| s.name.clone()),
                    );
                    for r in k.references(&select) {
                        if is_qualified(&r) {
                            continue;
                        }
                        let name = parts(&r, &k, g).pop().unwrap_or_default();
                        let own_element = r.ancestors().find(|a| a.kind() == k.element);
                        let alias_elsewhere = aliases
                            .iter()
                            .any(|(e, n)| *n == name && Some(e) != own_element.as_ref());
                        if alias_elsewhere || using.contains(&name) {
                            continue;
                        }
                        let text = file.text[crate::tree::significant_range(&r, g)].to_string();
                        ctx.report(
                            crate::tree::significant_range(&r, g),
                            self.message.replace("{name}", &text),
                        );
                    }
                }
            }
            Check::UniqueAliases => {
                let mut seen: std::collections::HashSet<(u32, String)> = Default::default();
                for e in ctx.nodes(&[k.element]) {
                    let Some(clause) = e.parent() else { continue };
                    let named = match e.children().find(|c| c.kind() == k.alias) {
                        Some(a) => last_part(&a, &k, g),
                        None => (e.children().find(|c| c.kind() == k.reference))
                            .and_then(|r| last_part(&r, &k, g)),
                    };
                    let Some((name, range)) = named else { continue };
                    let key = (u32::from(clause.text_range().start()), name);
                    if !seen.insert(key) {
                        let text = file.text[range].to_string();
                        ctx.report(range, self.message.replace("{name}", &text));
                    }
                }
            }
            Check::UnusedAlias => {
                for select in ctx.nodes(&[k.select]) {
                    let used: Vec<String> = (select.descendants())
                        .filter(|n| n.kind() == k.reference)
                        .filter(|n| n.parent().is_none_or(|p| p.kind() != k.table))
                        .filter(is_qualified)
                        .filter_map(|n| parts(&n, &k, g).into_iter().next())
                        .collect();
                    for s in k.sources(&select, g) {
                        let Some((name, range)) = s.alias else {
                            continue;
                        };
                        if s.standalone || used.contains(&name) {
                            continue;
                        }
                        let text = file.text[range].to_string();
                        ctx.report(range, self.message.replace("{name}", &text));
                    }
                }
            }
            Check::Consistency => {
                for select in ctx.nodes(&[k.select]) {
                    let all = k.sources(&select, g);
                    let real: Vec<&Source> = all.iter().filter(|s| !s.standalone).collect();
                    let [only] = real.as_slice() else { continue };
                    let Some(table) = &only.name else { continue };
                    let mut aliases: Vec<String> =
                        k.aliases(&select, g).into_iter().map(|(_, n)| n).collect();
                    aliases.extend(
                        all.iter()
                            .filter(|s| s.standalone)
                            .filter_map(|s| s.name.clone()),
                    );
                    let mut seen = [false, false];
                    let mut flagged: Vec<TextRange> = Vec::new();
                    let mut unqualified: Vec<TextRange> = Vec::new();
                    let mut first = true;
                    for r in k.references(&select) {
                        let qualified = is_qualified(&r);
                        if !qualified && k.star(&r).is_some() {
                            continue;
                        }
                        let name = parts(&r, &k, g).pop().unwrap_or_default();
                        if !qualified && (&name == table || aliases.contains(&name)) {
                            continue;
                        }
                        let range = crate::tree::significant_range(&r, g);
                        if !qualified {
                            unqualified.push(range);
                        }
                        let kind = qualified as usize;
                        if (seen[0] || seen[1]) && !seen[kind] {
                            // The first mix also reports every unqualified reference: they
                            // must be qualified to agree with the rest.
                            if first {
                                first = false;
                                flagged.extend(unqualified.iter().copied());
                            }
                            if qualified {
                                flagged.push(range);
                            }
                        } else if !first && !qualified {
                            flagged.push(range);
                        }
                        seen[kind] = true;
                    }
                    flagged.sort_by_key(|r| r.start());
                    flagged.dedup();
                    for range in flagged {
                        let text = file.text[range].trim().to_string();
                        ctx.report(range, self.message.replace("{name}", &text));
                    }
                }
            }
        }
    }
}

/// The columns a `USING (a, b)` of the select's joins names: the identifiers of the brackets
/// that follow the word.
fn using_columns(select: &SyntaxNode, k: &Kinds, g: &Grammar) -> Vec<String> {
    let mut out = Vec::new();
    for n in own(select, &k.queries) {
        let mut after_using = false;
        for e in n.children_with_tokens() {
            match e {
                rowan::NodeOrToken::Token(t) if g.is_trivia(t.kind()) => {}
                rowan::NodeOrToken::Token(t) => {
                    after_using = t.text().eq_ignore_ascii_case("using")
                }
                rowan::NodeOrToken::Node(b) if after_using => {
                    after_using = false;
                    for t in b.children_with_tokens().filter_map(|e| e.into_token()) {
                        if is_name(&t, k, g) {
                            out.push(t.text().trim_matches(['"', '`']).to_ascii_lowercase());
                        }
                    }
                }
                rowan::NodeOrToken::Node(_) => after_using = false,
            }
        }
    }
    out
}
