//! Case of words as data: the `[case]` rows of a language's rules file say which token kinds are
//! written in which case, `KINDS : POLICY`. `KINDS` are names of the grammar's kinds joined by `|`;
//! `POLICY` is `upper`, `lower`, `capitalise`, `consistent` or the name of a string option that
//! holds one of them (`keyword_case`; any other value means `consistent`). The first row that
//! names a kind decides; a kind without a row keeps its case.
//!
//! `consistent` follows sqlfluff (CP01 to CP05): the first word of a row fixes what the file may
//! be (`upper`, `lower` or `capitalise`), each word that differs rules cases out, and once none is
//! left the words after it, and so the whole file, take the case the last non-empty set preferred.

use core_config::{OptionValue, ResolvedOptions};
use core_syntax::{Grammar, SyntaxKind};

const UPPER: u8 = 1;
const LOWER: u8 = 2;
const CAPITALISE: u8 = 4;
/// Preference order of the cases a file may still be in.
const ORDER: [u8; 3] = [UPPER, LOWER, CAPITALISE];

#[derive(Clone, Copy)]
enum Policy {
    Consistent,
    Fixed(u8),
}

fn policy_of(name: &str) -> Option<Policy> {
    Some(match name {
        "upper" => Policy::Fixed(UPPER),
        "lower" => Policy::Fixed(LOWER),
        "capitalise" => Policy::Fixed(CAPITALISE),
        "consistent" => Policy::Consistent,
        _ => return None,
    })
}

struct Row {
    kinds: Vec<SyntaxKind>,
    /// The policy, or the option that holds it.
    policy: Result<Policy, String>,
}

/// The `[case]` rows of one language.
#[derive(Default)]
pub struct Cases(Vec<Row>);

#[derive(Default)]
struct Memory {
    refuted: u8,
    latest: Option<u8>,
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect()
    })
}

fn convert(raw: &str, case: u8) -> String {
    match case {
        UPPER => raw.to_uppercase(),
        LOWER => raw.to_lowercase(),
        _ => capitalise(raw),
    }
}

impl Memory {
    /// Takes in `raw`; returns the case it must be written in, if it is not already.
    fn visit(&mut self, raw: &str, policy: Policy) -> Option<String> {
        let first_lower = raw
            .chars()
            .find(|c| c.to_lowercase().ne(c.to_uppercase()))
            .map(|c| c.to_uppercase().ne(std::iter::once(c)));
        if first_lower == Some(true) {
            self.refuted |= UPPER | CAPITALISE;
            if raw != raw.to_lowercase() {
                self.refuted |= LOWER;
            }
        } else {
            self.refuted |= LOWER;
            if raw != raw.to_uppercase() {
                self.refuted |= UPPER;
            }
            if raw != capitalise(raw) {
                self.refuted |= CAPITALISE;
            }
        }
        let case = match policy {
            Policy::Fixed(case) if self.refuted & case == 0 => return None,
            Policy::Fixed(case) => case,
            Policy::Consistent => {
                if let Some(c) = ORDER.into_iter().find(|c| self.refuted & c == 0) {
                    self.latest = Some(c);
                    return None;
                }
                self.latest.unwrap_or(UPPER)
            }
        };
        let fixed = convert(raw, case);
        (fixed != raw).then_some(fixed)
    }
}

impl Cases {
    pub fn parse(grammar: &Grammar, rows: impl Iterator<Item = impl AsRef<str>>) -> Cases {
        Cases(
            rows.map(|l| {
                let (kinds, policy) = l
                    .as_ref()
                    .split_once(" : ")
                    .unwrap_or_else(|| panic!("case: `{}`", l.as_ref()));
                let policy = policy.trim();
                Row {
                    kinds: kinds
                        .split('|')
                        .map(|n| {
                            grammar
                                .kind_by_name(n.trim())
                                .unwrap_or_else(|| panic!("case: unknown kind `{n}`"))
                        })
                        .collect(),
                    policy: policy_of(policy).ok_or_else(|| policy.to_string()),
                }
            })
            .collect(),
        )
    }

    /// `text` of a token of `kind` in the case a literal policy (`upper`, `lower`, `capitalise`)
    /// asks for, `None` when it is in it already or no such row names the kind.
    pub fn fixed(&self, kind: SyntaxKind, text: &str) -> Option<String> {
        let row = self.0.iter().find(|r| r.kinds.contains(&kind))?;
        match row.policy {
            Ok(Policy::Fixed(case)) => {
                let moves = |f: fn(char) -> bool| text.chars().any(f);
                let wrong = match case {
                    UPPER => moves(|c| c.to_uppercase().ne(std::iter::once(c))),
                    LOWER => moves(|c| c.to_lowercase().ne(std::iter::once(c))),
                    _ => true,
                };
                wrong.then(|| convert(text, case)).filter(|c| c != text)
            }
            _ => None,
        }
    }

    /// A pass over the words of a file in order, with the policies the options give.
    pub fn recaser(&self, options: &ResolvedOptions) -> Recaser<'_> {
        Recaser {
            policies: self
                .0
                .iter()
                .map(|r| match &r.policy {
                    Ok(p) => *p,
                    Err(name) => match options.get(name) {
                        Some(OptionValue::String(s)) => policy_of(s).unwrap_or(Policy::Consistent),
                        _ => Policy::Consistent,
                    },
                })
                .collect(),
            memory: self.0.iter().map(|_| Memory::default()).collect(),
            cases: self,
        }
    }
}

pub struct Recaser<'a> {
    cases: &'a Cases,
    policies: Vec<Policy>,
    memory: Vec<Memory>,
}

impl Recaser<'_> {
    /// The word of a token of `kind` as it must be written, `None` when it is right already.
    pub fn word(&mut self, kind: SyntaxKind, text: &str) -> Option<String> {
        let i = self.cases.0.iter().position(|r| r.kinds.contains(&kind))?;
        self.memory[i].visit(text, self.policies[i])
    }
}
