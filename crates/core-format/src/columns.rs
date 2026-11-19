//! Columns of consecutive lines: the lines of a run pad their cell so that the next one starts one
//! past the widest (`terraform fmt` lines up the `=` of consecutive attributes and their trailing
//! comments, PSScriptAnalyzer the `=` of the entries of a hashtable).
//!
//! concept: column-alignment
//! what: Cells of consecutive lines padded to start in one column.
//! tools: terraform fmt alignment, PSScriptAnalyzer PSAlignAssignmentStatement

/// Calls `each` with every run of consecutive items for which `member` holds.
pub fn runs<T>(items: &mut [T], member: impl Fn(&T) -> bool, mut each: impl FnMut(&mut [T])) {
    let mut start = None;
    for i in 0..=items.len() {
        let inside = items.get(i).is_some_and(&member);
        match (start, inside) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                each(&mut items[s..i]);
                start = None;
            }
            _ => {}
        }
    }
}

/// Calls `set` with the blanks each item needs to reach the widest of the run.
pub fn pad<T>(run: &mut [T], width: impl Fn(&T) -> usize, mut set: impl FnMut(&mut T, usize)) {
    let widest = run.iter().map(&width).max().unwrap_or(0);
    for item in run {
        let pad = widest - width(item);
        set(item, pad);
    }
}
