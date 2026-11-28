//! Short ids for the fixes of a report, so a caller can name the fixes to apply (`sudsy fix
//! --apply ID`). An id hashes the file's path as shown, the rule, the trimmed text of the finding's
//! first line, the finding's occurrence among the findings of that rule on equal lines, and the
//! fix's index: it stays the same while that line is unchanged, and nothing is stored.
//!
//! concept: none (infrastructure: fix ids)

use crate::diagnostic::{Diagnostic, FixKind};
use crate::lines::LineIndex;
use std::collections::HashMap;

/// Characters of an id: 6 of base 32 (30 bits), so 1,000 ids in one report collide with a chance
/// of about 1 in 2,000.
pub const ID_LEN: usize = 6;

/// Lowercase letters and digits without `i`, `l`, `o`, `u`, so an id reads back unambiguously.
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Does a fix of this kind get an id? Safe fixes are applied by `fix` without asking.
pub fn has_id(kind: FixKind) -> bool {
    kind != FixKind::Safe
}

/// The ids of the fixes of `diagnostics` (the findings of the file `path` over `text`): one list
/// per diagnostic, `None` for a fix without an id.
pub fn fix_ids(
    path: &str,
    text: &str,
    lines: &LineIndex,
    diagnostics: &[Diagnostic],
) -> Vec<Vec<Option<String>>> {
    let mut seen: HashMap<(&str, &str), u32> = HashMap::new();
    diagnostics
        .iter()
        .map(|d| {
            if !d.fixes.iter().any(|f| has_id(f.kind)) {
                return vec![None; d.fixes.len()];
            }
            let (start, end) = lines.line_range(text, lines.line(d.range.start()));
            let line = text[start..end].trim();
            let occurrence = seen.entry((d.rule.as_str(), line)).or_default();
            let n = *occurrence;
            *occurrence += 1;
            d.fixes
                .iter()
                .enumerate()
                .map(|(index, f)| {
                    has_id(f.kind).then(|| id(&[path, &d.rule, line], n, index as u32))
                })
                .collect()
        })
        .collect()
}

/// FNV-1a over the parts (each ended by a byte UTF-8 never contains) and the two numbers.
fn id(parts: &[&str], occurrence: u32, index: u32) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for part in parts {
        eat(part.as_bytes());
        eat(&[0xff]);
    }
    eat(&occurrence.to_le_bytes());
    eat(&index.to_le_bytes());
    let mut folded = h ^ (h >> 32);
    let mut out = String::with_capacity(ID_LEN);
    for _ in 0..ID_LEN {
        out.push(ALPHABET[(folded & 31) as usize] as char);
        folded >>= 5;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Category, Edit, Fix, Severity};
    use core_syntax::{TextRange, TextSize};

    fn finding(rule: &str, at: u32, kinds: &[FixKind]) -> Diagnostic {
        let range = TextRange::new(TextSize::from(at), TextSize::from(at + 1));
        Diagnostic {
            rule: rule.into(),
            category: Category::Style,
            severity: Severity::Warning,
            message: String::new(),
            range,
            related: Vec::new(),
            fixes: kinds
                .iter()
                .map(|&k| Fix::new("t", k, vec![Edit::delete(range)]))
                .collect(),
        }
    }

    fn ids(text: &str, diagnostics: &[Diagnostic]) -> Vec<Vec<Option<String>>> {
        fix_ids("a.yaml", text, &LineIndex::new(text), diagnostics)
    }

    #[test]
    fn safe_fixes_have_no_id_and_the_others_differ_by_index() {
        let got = ids(
            "a: yes\n",
            &[finding(
                "r",
                3,
                &[FixKind::Safe, FixKind::Unsafe, FixKind::Suggestion],
            )],
        );
        let [safe, a, b] = &got[0][..] else { panic!() };
        assert!(safe.is_none());
        assert_eq!(a.as_ref().unwrap().len(), ID_LEN);
        assert_ne!(a, b);
    }

    #[test]
    fn an_id_follows_its_line_and_tells_equal_lines_apart() {
        let one = ids("a: yes\n", &[finding("r", 3, &[FixKind::Unsafe])]);
        // A line above and more indentation move the finding, not its id.
        let moved = ids("x: 1\n  a: yes\n", &[finding("r", 10, &[FixKind::Unsafe])]);
        assert_eq!(one, moved);
        let edited = ids("a: yes # c\n", &[finding("r", 3, &[FixKind::Unsafe])]);
        assert_ne!(one, edited);
        let twice = ids(
            "a: yes\na: yes\n",
            &[
                finding("r", 3, &[FixKind::Unsafe]),
                finding("r", 10, &[FixKind::Unsafe]),
            ],
        );
        assert_eq!(twice[0], one[0]);
        assert_ne!(twice[0], twice[1]);
    }
}
