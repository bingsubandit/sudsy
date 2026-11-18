//! The layout of a SQL file, on the blocks and points of [`Seq`]. It follows sqlfluff's layout
//! rules (LT01 to LT15) with its defaults: the rules run in turn until the file stops changing,
//! as `sqlfluff fix` does, and the result is printed line by line.
//!
//! concept: new line-layout-engine
//! what: A line-breaking, indentation and wrapping engine on its own block and point model.
//! tools: sqlfluff layout rules

mod blocks;
mod breaks;
mod indent;
mod seq;
mod space;
#[cfg(test)]
mod space_old;
mod wrap;

use core_config::{ResolvedOptions, shared};
use core_format::rules::LazyRules;
use core_format::{Doc, doc};
use core_syntax::{Grammar, SyntaxNode, grammars};
use seq::Seq;

static ROWS: LazyRules = LazyRules::new(|| grammars::SQL.get(), include_str!("../../format.rules"));

/// One level of indentation.
fn unit(options: &ResolvedOptions) -> String {
    if options.is(shared::INDENT_STYLE, "tab") {
        "\t".into()
    } else {
        " ".repeat(options.uint(shared::INDENT_SIZE, 4))
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
    let mut seq = Seq::build(root, grammar, ROWS.get().space());
    match options.text("dialect") {
        "mysql" => blocks::add_indents(&mut seq),
        "tsql" => blocks::add_tsql_indents(&mut seq),
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
    let width = options.uint(shared::LINE_WIDTH, 80);
    let comma = if options.is("comma_position", "leading") {
        breaks::Pos::Leading
    } else {
        breaks::Pos::Trailing
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
    if std::env::var_os("SQLDBG").is_some() {
        let mut t = String::new();
        for (p, b) in seq.points.iter().zip(&seq.blocks) {
            t.push_str(&"\n".repeat(p.newlines as usize));
            t.push_str(&p.ws);
            t.push_str(&b.text);
        }
        eprintln!("SQLDBG<<{t}>>");
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
