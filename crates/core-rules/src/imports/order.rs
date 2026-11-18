//! The order of import statements: which group an entry sorts into and how paths compare. A tool's
//! order is a value of [`Order`] (its options), never a branch on the tool.
//!
//! concept: sorted-imports
//! what: Import statements ordered by group, name and case, with blank lines between groups.
//! tools: isort, google-java-format, ktlint import-ordering, StyleCop SA1210, PHP-CS-Fixer ordered_imports, buf format
//! reads: family:imports, grammar:imports

use super::entry::Entry;
use std::cmp::Ordering;

/// How paths compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    /// Code points.
    Ordinal,
    /// Letters ignoring case, with punctuation before digits before letters; names that differ
    /// only in case put the lower-case one first.
    Culture,
    /// As `Culture`, but names that differ only in case stay in their order.
    CultureIgnoreCase,
    /// Digit runs as numbers, without regard to case first.
    Natural,
}

/// Where the blank lines of the written list go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Separate {
    /// None: the statements follow each other.
    None,
    /// Between two statements of different categories.
    Category,
    /// Between a statement under a first root and one that is not.
    Root,
}

/// Owned words for the lists of an [`Order`].
pub fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// What an import list's order is made of.
#[derive(Clone, Debug)]
pub struct Order {
    pub compare: Compare,
    /// The groups, in order, by a word of the statement: a modifier (`static`), `alias`, or `*`
    /// for a statement without a listed word. A statement whose word is not listed keeps its
    /// place.
    pub categories: Vec<String>,
    /// Modifiers that keep a statement in place, unless the modifier is a category.
    pub fixed: Vec<String>,
    /// Statements whose path starts with one of these names (up to the first `.` or `<`) come
    /// first in their category (`System`).
    pub roots_first: Vec<String>,
    /// Plain statements whose path starts with one of these come after the others, in this
    /// order (`java.`, `javax.`, `kotlin.`).
    pub last_prefixes: Vec<String>,
    /// A path that starts with this qualifier (`global::`) comes behind the root-first statements
    /// but before the others, unless the name after it starts with one of `behind_except`.
    pub behind: Option<String>,
    pub behind_except: Vec<String>,
    /// Aliases are compared by their alias name, ignoring case, not by their path.
    pub alias_by_name: bool,
    /// The same import written twice is written once.
    pub dedup: bool,
    pub separate: Separate,
    /// Comments between the statements: kept with the statement below (the list is rewritten
    /// around them), or the list is left alone.
    pub keep_comments: bool,
    /// With `keep_comments`: the comments above the first statement move with it, instead of
    /// staying in front of the list.
    pub first_comment_moves: bool,
}

impl Order {
    /// An order with the given categories that compares paths with `compare` and has no other
    /// options; set those with struct update syntax.
    pub fn new(compare: Compare, categories: &[&str]) -> Order {
        Order {
            compare,
            categories: words(categories),
            fixed: Vec::new(),
            roots_first: Vec::new(),
            last_prefixes: Vec::new(),
            behind: None,
            behind_except: Vec::new(),
            alias_by_name: false,
            dedup: false,
            separate: Separate::None,
            keep_comments: false,
            first_comment_moves: false,
        }
    }

    fn listed(&self, word: &str) -> Option<usize> {
        self.categories.iter().position(|c| c == word)
    }

    /// The category of an entry; `None` when it keeps its place.
    pub fn category(&self, e: &Entry) -> Option<usize> {
        let fixed = e
            .modifiers
            .iter()
            .any(|m| self.fixed.contains(m) && self.listed(m).is_none());
        let found = e
            .modifiers
            .iter()
            .map(String::as_str)
            .chain(e.alias.is_some().then_some("alias"))
            .filter_map(|w| self.listed(w))
            .min();
        if fixed {
            return None;
        }
        match (found, e.modifiers.is_empty() && e.alias.is_none()) {
            (Some(c), _) => Some(c),
            (None, true) => self.listed("*"),
            (None, false) => None,
        }
    }

    /// Is the entry under a first root (`System.IO`)?
    pub fn is_root(&self, e: &Entry) -> bool {
        e.alias.is_none()
            && e.path
                .split(['.', '<'])
                .next()
                .is_some_and(|r| self.roots_first.iter().any(|x| x == r))
    }

    fn is_behind(&self, e: &Entry) -> bool {
        self.behind.as_deref().is_some_and(|q| {
            e.path.starts_with(q)
                && !self.behind_except.iter().any(|r| e.path[q.len()..].starts_with(r.as_str()))
        })
    }

    /// What the statement sorts by before its path: category, root, last prefix.
    fn group(&self, e: &Entry, category: usize) -> (usize, u8, usize) {
        let plain = self.listed("*") == Some(category);
        let root = if self.is_root(e) {
            0
        } else if plain && self.is_behind(e) {
            1
        } else {
            2
        };
        let last = if plain {
            self.last_prefixes
                .iter()
                .position(|p| e.path.starts_with(p.as_str()))
                .map_or(0, |i| i + 1)
        } else {
            0
        };
        (category, root, last)
    }

    fn cmp_entries(&self, x: &Entry, y: &Entry) -> Ordering {
        if self.alias_by_name && x.alias.is_some() {
            let a = x.alias.as_deref().unwrap_or_default().to_lowercase();
            return a.cmp(&y.alias.as_deref().unwrap_or_default().to_lowercase());
        }
        compare(&x.path, &y.path, self.compare)
    }

    /// Where each statement goes: position `i` of the sorted list holds the statement
    /// `result[i]`. Statements that keep their place stay at their positions; the others fill the
    /// remaining positions in order.
    pub fn sorted(&self, entries: &[Entry]) -> Vec<usize> {
        let cats: Vec<Option<usize>> = entries.iter().map(|e| self.category(e)).collect();
        let slots: Vec<usize> = (0..entries.len()).filter(|&i| cats[i].is_some()).collect();
        let key = |i: usize| self.group(&entries[i], cats[i].unwrap_or_default());
        let mut ordered = slots.clone();
        ordered.sort_by(|&a, &b| {
            key(a)
                .cmp(&key(b))
                .then_with(|| self.cmp_entries(&entries[a], &entries[b]))
        });
        let mut result: Vec<usize> = (0..entries.len()).collect();
        for (&slot, &item) in slots.iter().zip(&ordered) {
            result[slot] = item;
        }
        result
    }

    /// [`Order::sorted`] without the statements that repeat the one before them (with `dedup`).
    pub fn sorted_unique(&self, entries: &[Entry]) -> Vec<usize> {
        let mut out = self.sorted(entries);
        if self.dedup {
            out.dedup_by(|b, a| entries[*a].same_as(&entries[*b]));
        }
        out
    }

    /// Does a blank line go between these two neighbours of the written list?
    pub fn blank_between(&self, a: &Entry, b: &Entry) -> bool {
        match self.separate {
            Separate::None => false,
            Separate::Category => self.category(a) != self.category(b),
            Separate::Root => self.is_root(a) != self.is_root(b),
        }
    }
}

/// Primary weight of a character under the invariant culture: punctuation, digits, letters.
fn weight(c: char) -> (u8, u32) {
    const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    if let Some(i) = PUNCTUATION.find(c) {
        (0, i as u32)
    } else if c.is_alphabetic() {
        (2, c.to_lowercase().next().map_or(c as u32, |l| l as u32))
    } else if c.is_numeric() {
        (1, c as u32)
    } else {
        (0, 1000 + c as u32)
    }
}

fn culture(a: &str, b: &str, tie_break_case: bool) -> Ordering {
    let primary = a.chars().map(weight).cmp(b.chars().map(weight));
    primary.then_with(|| {
        if !tie_break_case {
            return Ordering::Equal;
        }
        let differ = a.chars().zip(b.chars()).find(|(x, y)| x != y);
        match differ {
            Some((x, _)) if x.is_lowercase() => Ordering::Less,
            Some(_) => Ordering::Greater,
            None => Ordering::Equal,
        }
    })
}

/// Compares two paths.
pub fn compare(a: &str, b: &str, how: Compare) -> Ordering {
    match how {
        Compare::Ordinal => a.cmp(b),
        Compare::Culture => culture(a, b, true),
        Compare::CultureIgnoreCase => culture(a, b, false),
        Compare::Natural => crate::sorting::natural(&a.to_lowercase(), &b.to_lowercase())
            .then_with(|| crate::sorting::natural(a, b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn culture_puts_punctuation_before_digits_before_letters_and_ignores_case() {
        let ordered = [
            "_x",
            "A_b",
            "A.B",
            "a.b",
            "A1",
            "Ab",
            "b",
            "Microsoft.A",
            "z",
            "Z",
            "Z1",
        ];
        for pair in ordered.windows(2) {
            assert_ne!(
                compare(pair[0], pair[1], Compare::CultureIgnoreCase),
                Ordering::Greater,
                "{} before {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn culture_puts_lower_case_first_only_when_asked() {
        assert_eq!(compare("aa", "Aa", Compare::Culture), Ordering::Less);
        assert_eq!(compare("aa", "Aa", Compare::CultureIgnoreCase), Ordering::Equal);
        assert_eq!(compare("Zeta", "alpha", Compare::Ordinal), Ordering::Less);
        assert_eq!(compare("System", "System.A", Compare::Culture), Ordering::Less);
    }
}
