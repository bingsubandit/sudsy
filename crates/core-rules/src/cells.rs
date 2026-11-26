//! The cells of a notebook as parts of one program (`FileContext::cells`): which cell an offset is
//! in, and the rule option `except_cell_tail`, which leaves a finding on the last statement of a
//! cell alone because the notebook shows that statement's value (a bare expression, a `;` that
//! hides it).
//!
//! concept: notebook-cells
//! what: The cells of a notebook are one program whose rules start afresh at a cell and whose last statement of a cell shows its value.
//! tools: ruff notebook mode
//! reads: rule:no-useless-expression, rule:useless-semicolon

use crate::engine::ActiveRule;
use crate::tree::code_start;
use crate::{Diagnostic, FileContext};
use core_syntax::{TextRange, TextSize};

/// The index of the cell that holds `at`.
pub fn cell_of(cells: &[TextRange], at: TextSize) -> Option<usize> {
    let i = cells.partition_point(|c| c.start() <= at);
    i.checked_sub(1).filter(|i| at < cells[*i].end())
}

/// Are the two offsets in one cell (always, in a file that has none)?
pub fn same_cell(cells: &[TextRange], a: TextSize, b: TextSize) -> bool {
    cells.is_empty() || cell_of(cells, a) == cell_of(cells, b)
}

/// Drops the findings of the rules with `except_cell_tail` that sit on the last top-level
/// statement of a cell: the statement itself or the tokens it ends with.
pub fn except_cell_tail(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if file.cells.is_empty() {
        return;
    }
    let exempt: Vec<&str> = (active.iter())
        .filter(|a| a.options.bool("except_cell_tail"))
        .map(|a| a.rule.meta().id)
        .collect();
    diagnostics.retain(|d| !exempt.contains(&d.rule.as_str()) || !on_tail(file, d.range));
}

fn on_tail(file: &FileContext<'_>, range: TextRange) -> bool {
    let Some(cell) = cell_of(file.cells, range.start()) else {
        return false;
    };
    let cell = file.cells[cell];
    let Some(last) = (file.root.children())
        .filter(|n| !file.grammar.is_trivia(n.kind()) && cell.contains(code_start(n, file.grammar)))
        .last()
    else {
        return false;
    };
    // The statement's own start, what follows it in the cell (its `;`), or the last token it ends with.
    range.start() == code_start(&last, file.grammar)
        || range.start() >= last.text_range().end()
        || last
            .last_token()
            .is_some_and(|t| t.text_range() == range && !file.grammar.is_trivia(t.kind()))
}
