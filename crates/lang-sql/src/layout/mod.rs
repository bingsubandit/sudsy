//! The layout of a SQL file, on the blocks and points of [`Seq`]. It follows sqlfluff's layout
//! rules (LT01 to LT15) with its defaults: the rules run in turn until the file stops changing,
//! as `sqlfluff fix` does, and the result is printed line by line.

mod blocks;
mod breaks;
mod indent;
mod seq;
mod space;
mod wrap;

use core_config::{OptionValue, ResolvedOptions, shared};
use core_format::rules::LazyRules;
use core_format::{Doc, doc};
use core_syntax::{Grammar, SyntaxNode, grammars};
use seq::Seq;

static ROWS: LazyRules = LazyRules::new(|| grammars::SQL.get(), include_str!("../../format.rules"));

/// One level of indentation.
fn unit(options: &ResolvedOptions) -> String {
    let int = |name, fallback: usize| match options.get(name) {
        Some(OptionValue::Int(n)) => *n as usize,
        _ => fallback,
    };
    match options.get(shared::INDENT_STYLE) {
        Some(OptionValue::String(s)) if s == "tab" => "\t".into(),
        _ => " ".repeat(int(shared::INDENT_SIZE, 4)),
    }
}

/// The signature of the layout, to see whether a round changed it.
fn signature(seq: &Seq) -> Vec<(u32, String)> {
    seq.points
        .iter()
        .map(|p| (p.newlines, p.ws.to_string()))
        .collect()
}

pub fn format(root: &SyntaxNode, grammar: &Grammar, options: &ResolvedOptions) -> Doc {
    let mut seq = Seq::build(root, grammar);
    match options.get("dialect") {
        Some(OptionValue::String(d)) if d == "mysql" => blocks::add_indents(&mut seq),
        Some(OptionValue::String(d)) if d == "tsql" => blocks::add_tsql_indents(&mut seq),
        _ => {}
    }
    let mut recase = ROWS.get().cases().recaser(options);
    for block in &mut seq.blocks {
        if let Some(fixed) = recase.word(block.kind, &block.text) {
            block.text = fixed;
        }
    }
    let unit = unit(options);
    let tab = unit.chars().count().max(1);
    let width = match options.get(shared::LINE_WIDTH) {
        Some(OptionValue::Int(n)) => *n as usize,
        _ => 80,
    };
    let comma = match options.get("comma_position") {
        Some(OptionValue::String(s)) if s == "leading" => breaks::Pos::Leading,
        _ => breaks::Pos::Trailing,
    };
    for _ in 0..40 {
        let before = signature(&seq);
        space::respace(&mut seq);
        indent::reindent(&mut seq, &unit);
        breaks::rebreak(&mut seq, comma);
        let wrapped = wrap::long_lines(&mut seq, width, tab, &unit, comma);
        breaks::ctes(&mut seq);
        breaks::select_targets(&mut seq, width);
        breaks::keywords(&mut seq);
        breaks::blank_lines(&mut seq);
        if !wrapped && signature(&seq) == before {
            break;
        }
    }
    print(&seq)
}

fn print(seq: &Seq) -> Doc {
    let mut parts = Vec::with_capacity(seq.blocks.len() * 2);
    for (point, block) in seq.points.iter().zip(&seq.blocks) {
        if block.kind == crate::generated::kind::EOF {
            break;
        }
        if point.newlines > 0 {
            parts.extend((0..point.newlines).map(|_| doc::hardline()));
        }
        parts.push(doc::text(point.ws.to_string()));
        parts.push(core_format::context::verbatim_text(&block.text));
    }
    doc::concat(parts)
}
