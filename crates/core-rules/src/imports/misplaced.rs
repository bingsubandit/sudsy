//! The statements of an import list that are out of place, found the way StyleCop's SA1208 to
//! SA1217 look for them: a statement under a first root after another one, an alias before a plain
//! statement, neighbours that compare the wrong way round, a `static` or alias statement that is
//! not where it belongs. Lists restart at a statement that has a preprocessor directive in front
//! of it.
//!
//! concept: sorted-imports
//! what: Import statements ordered by group, name and case, with blank lines between groups.
//! tools: isort, google-java-format, ktlint import-ordering, StyleCop SA1210, PHP-CS-Fixer ordered_imports, buf format
//! reads: family:imports, grammar:imports

use super::entry::Entry;
use super::order::{Order, compare};
use std::cmp::Ordering;

/// A statement that is out of place: which, and why.
pub struct Misplaced {
    pub at: usize,
    pub message: String,
}

fn misplaced(at: usize, message: String) -> Misplaced {
    Misplaced { at, message }
}

struct Cx<'a> {
    entries: &'a [Entry],
    o: &'a Order,
    /// The word that opens a statement (`using`).
    kw: &'a str,
    out: Vec<Misplaced>,
}

impl Cx<'_> {
    fn alias(&self, i: usize) -> bool {
        self.entries[i].alias.is_some()
    }

    fn statik(&self, i: usize) -> bool {
        self.entries[i].has("static")
    }

    fn root(&self, i: usize) -> bool {
        self.o.is_root(&self.entries[i])
    }

    fn after(&self, a: usize, b: usize) -> bool {
        compare(&self.entries[a].path, &self.entries[b].path, self.o.compare) == Ordering::Greater
    }
}

/// The statements that are out of place under `o`.
pub fn misplaced_entries(entries: &[Entry], o: &Order, kw: &str) -> Vec<Misplaced> {
    let mut cx = Cx {
        entries,
        o,
        kw,
        out: Vec::new(),
    };
    let listed = |w: &str| o.categories.iter().any(|c| c == w);
    if !o.roots_first.is_empty() {
        cx.roots_before_others();
    }
    if listed("alias") {
        cx.aliases_after_others();
    }
    cx.names_in_order();
    if listed("alias") {
        cx.aliases_in_order();
    }
    if listed("static") {
        cx.statics_placed();
        cx.statics_in_order();
    }
    cx.out.sort_by_key(|m| m.at);
    cx.out
}

impl Cx<'_> {
    fn roots_before_others(&mut self) {
        let mut before: Option<String> = None;
        for i in 1..self.entries.len() {
            let e = &self.entries[i];
            if self.alias(i) || self.statik(i) || e.directive_before || !self.root(i) {
                continue;
            }
            if before.is_none() {
                if self.root(i - 1) && !self.statik(i - 1) {
                    continue;
                }
                before = Some(self.entries[i - 1].path.clone());
            }
            let name = before.as_deref().unwrap_or_default();
            let (root, kw) = (&self.o.roots_first[0], self.kw);
            self.out.push(misplaced(
                i,
                format!(
                    "`{}` should come before `{name}`: {root} {kw}s go first",
                    e.path
                ),
            ));
        }
    }

    fn aliases_after_others(&mut self) {
        for i in 0..self.entries.len().saturating_sub(1) {
            let next = &self.entries[i + 1];
            if self.alias(i) && !self.alias(i + 1) && !self.statik(i + 1) && !next.directive_before {
                self.out.push(misplaced(
                    i,
                    format!("alias directives belong after the other {}s", self.kw),
                ));
            }
        }
    }

    fn names_in_order(&mut self) {
        let (mut others, mut roots) = (Vec::new(), Vec::new());
        for i in 0..self.entries.len() {
            if self.entries[i].directive_before {
                self.flush(&mut others);
                self.flush(&mut roots);
            }
            if self.alias(i) || self.statik(i) {
                continue;
            }
            if self.root(i) {
                roots.push(i);
            } else {
                others.push(i);
            }
        }
        self.flush(&mut others);
        self.flush(&mut roots);
    }

    fn flush(&mut self, list: &mut Vec<usize>) {
        for pair in list.windows(2) {
            if self.after(pair[0], pair[1]) {
                self.out.push(misplaced(
                    pair[0],
                    format!(
                        "`{}` should come after `{}`",
                        self.entries[pair[0]].path, self.entries[pair[1]].path
                    ),
                ));
            }
        }
        list.clear();
    }

    fn aliases_in_order(&mut self) {
        let mut previous: Option<usize> = None;
        for i in 0..self.entries.len() {
            if self.entries[i].directive_before {
                previous = None;
            }
            if !self.alias(i) {
                continue;
            }
            let name = |i: usize| self.entries[i].alias.clone().unwrap_or_default();
            if let Some(p) = previous
                && name(p).to_lowercase().cmp(&name(i).to_lowercase()) == Ordering::Greater
            {
                self.out.push(misplaced(
                    i,
                    format!("alias `{}` should come before `{}`", name(i), name(p)),
                ));
                return;
            }
            previous = Some(i);
        }
    }

    fn statics_placed(&mut self) {
        let (mut last_static, mut last_alias) = (None, None);
        for i in 0..self.entries.len() {
            if self.entries[i].directive_before {
                (last_static, last_alias) = (None, None);
            }
            if self.statik(i) {
                if last_alias.is_some() {
                    let kw = self.kw;
                    self.out
                        .push(misplaced(i, format!("`{kw} static` belongs before the aliases")));
                    return;
                }
                last_static = Some(i);
            } else if self.alias(i) {
                last_alias = Some(i);
            } else if let Some(s) = last_static {
                let kw = self.kw;
                self.out.push(misplaced(
                    s,
                    format!("`{kw} static` belongs after the other {kw}s"),
                ));
                return;
            }
        }
    }

    fn statics_in_order(&mut self) {
        let roots = !self.o.roots_first.is_empty();
        let (mut last_static, mut last_root, mut first_other): (
            Option<usize>,
            Option<usize>,
            Option<usize>,
        ) = (None, None, None);
        for i in 0..self.entries.len() {
            if self.entries[i].directive_before {
                (last_static, last_root, first_other) = (None, None, None);
            }
            if !self.statik(i) {
                continue;
            }
            let path = |i: usize| self.entries[i].path.clone();
            let kw = self.kw;
            if roots && self.root(i) {
                if let Some(f) = first_other {
                    self.out.push(misplaced(
                        f,
                        format!(
                            "`{kw} static {}` should come after the {} statics",
                            path(f),
                            self.o.roots_first[0]
                        ),
                    ));
                    return;
                }
                if let Some(l) = last_root.filter(|&l| self.after(l, i)) {
                    self.out.push(misplaced(
                        l,
                        format!("`{kw} static {}` should come after `{}`", path(l), path(i)),
                    ));
                    return;
                }
                last_root = Some(i);
            } else {
                if let Some(l) = last_static.filter(|&l| self.after(l, i)) {
                    self.out.push(misplaced(
                        l,
                        format!("`{kw} static {}` should come after `{}`", path(l), path(i)),
                    ));
                    return;
                }
                last_static = Some(i);
                first_other = first_other.or(Some(i));
            }
        }
    }
}
